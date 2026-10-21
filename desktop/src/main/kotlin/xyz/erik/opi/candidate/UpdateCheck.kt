// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 版本更新检测（2026-10-22 用户裁决：「增加新版本更新提示和检测更新」，下载地址
//! = 用户给定的 https://opi.erik.xyz/download）。
//!
//! 数据源两级，都是**纯静态**（站点没有服务端）：
//! 1. `https://opi.erik.xyz/version.json` —— 首选。发版时与 `site/` 一起更新：
//!    `{"version":"1.20.3","download":"https://opi.erik.xyz/download"}`
//! 2. `https://opi.erik.xyz/index.html` 的 JSON-LD `"softwareVersion"` —— 兜底。
//!    站点还没部署 version.json 时（当前线上就是）靠它工作；再兜底找下载文件名里的
//!    `opi-<版本>-windows`。**解析是纯函数**，夹具用线上形状，主机可单测。
//!
//! 网络失败**不打扰**用户：自动检测（打开设置时）失败只记在「关于」页；只有手动点
//! 「检测更新」才把失败原因显示出来。检查不阻塞 UI（调用方放 IO 线程，超时 4s）。

package xyz.erik.opi.candidate

import java.net.HttpURLConnection
import java.net.URI

/**
 * 下载落点（用户 2026-10-22 裁决）：`https://opi.erik.xyz/download`。
 * 该路径现在由 `site/download/index.html` 转场到主页下载区；具体安装包仍在
 * `/download/opi-…` 下。
 */
internal const val UPDATE_DOWNLOAD_URL = "https://opi.erik.xyz/download"

/** 机器可读的最新版本（随站点部署）。 */
internal const val UPDATE_VERSION_URL = "https://opi.erik.xyz/version.json"

/** 兜底：站点首页的 JSON-LD 里有 `softwareVersion`。 */
internal const val UPDATE_INDEX_URL = "https://opi.erik.xyz/index.html"

/** 响应体上限：版本信息是几百字节；超过这个数只可能是错误页/坏端点。 */
private const val UPDATE_MAX_BYTES = 256 * 1024

/** 网络超时（连接/读取各一份）。打开设置时的自动检测不该拖慢窗口。 */
private const val UPDATE_TIMEOUT_MS = 4000

/** 版本端点解析结果（`download` 缺省 = 用 [UPDATE_DOWNLOAD_URL]）。 */
internal data class VersionPoint(val version: String, val download: String?)

/** 一次检测的结论。 */
internal sealed interface UpdateCheck {
    /** 当前版本不低于站点版本。 */
    data object UpToDate : UpdateCheck

    /** 站点有更新的版本；[download] 是「前往下载」的地址。 */
    data class Available(val latest: String, val download: String) : UpdateCheck

    /** 检测失败（给用户看的一句原因）。 */
    data class Failed(val reason: String) : UpdateCheck
}

/** 版本号只认数字与点（`1.20.2`，可带一个 `v`）。 */
private val VERSION_RE = Regex("""^[0-9]+(?:\.[0-9]+)*$""")

/**
 * `version.json` → [VersionPoint]。坏 JSON / 缺 version / 版本号不是数字点号 → null
 * （调用方回落到首页解析，不把坏端点当「已是最新」）。
 */
internal fun parseVersionPoint(text: String): VersionPoint? {
    val obj = parseLine(text) ?: return null
    val raw = ((obj["version"] ?: obj["latest"]) as? JVal.JStr)?.v?.trim() ?: return null
    val version = raw.removePrefix("v").trim()
    if (!VERSION_RE.matches(version)) return null
    val download = (obj["download"] as? JVal.JStr)?.v
        ?.takeIf { it.startsWith("https://") }
    return VersionPoint(version, download)
}

/**
 * 站点首页 → 最新版本。优先 JSON-LD 的 `softwareVersion`（站点自己维护的权威值），
 * 再兜底下载文件名（`opi-1.20.2-windows-x64-setup.exe`）。
 */
internal fun parseSiteVersion(html: String): String? {
    Regex(""""softwareVersion"\s*:\s*"v?([0-9]+(?:\.[0-9]+)*)"""")
        .find(html)?.groupValues?.get(1)?.let { return it }
    Regex("""opi-([0-9]+(?:\.[0-9]+)*)-windows""")
        .find(html)?.groupValues?.get(1)?.let { return it }
    return null
}

/**
 * 语义化版本比较（只比数字段；缺的段按 0，`1.20` == `1.20.0`）。
 * 返回 <0 / 0 / >0，与 [String.compareTo] 同号语义。
 */
internal fun compareVersions(a: String, b: String): Int {
    fun parts(v: String) = v.removePrefix("v").split('.').map { it.toIntOrNull() ?: 0 }
    val pa = parts(a)
    val pb = parts(b)
    for (i in 0 until maxOf(pa.size, pb.size)) {
        val x = pa.getOrElse(i) { 0 }
        val y = pb.getOrElse(i) { 0 }
        if (x != y) return x.compareTo(y)
    }
    return 0
}

/**
 * 检测更新。[current] 缺省 = 构建时生成的 [OPI_VERSION]；[fetch] 可注入（门禁不碰网络）。
 *
 * 顺序：version.json → 首页 JSON-LD/下载文件名。两级都拿不到才是 [UpdateCheck.Failed]
 * —— 网络不通与「站点没版本信息」都归失败，**绝不静默当已是最新**。
 */
internal fun checkForUpdate(
    current: String = OPI_VERSION,
    fetch: (String) -> String? = ::httpGet,
): UpdateCheck {
    val point = fetch(UPDATE_VERSION_URL)?.let(::parseVersionPoint)
    if (point != null) {
        return if (compareVersions(point.version, current) > 0) {
            UpdateCheck.Available(point.version, point.download ?: UPDATE_DOWNLOAD_URL)
        } else {
            UpdateCheck.UpToDate
        }
    }
    val html = fetch(UPDATE_INDEX_URL)
        ?: return UpdateCheck.Failed("连不上更新服务器（网络不可用？）")
    val latest = parseSiteVersion(html)
        ?: return UpdateCheck.Failed("站点上没有可识别的版本信息")
    return if (compareVersions(latest, current) > 0) {
        UpdateCheck.Available(latest, UPDATE_DOWNLOAD_URL)
    } else {
        UpdateCheck.UpToDate
    }
}

/** 「关于」页那一行的文字（纯函数，门禁直接钉）。 */
internal fun updateLabel(state: UpdateCheck?, checking: Boolean): String = when {
    checking -> "正在检测更新…"
    state is UpdateCheck.UpToDate -> "已是最新版本（v$OPI_VERSION）"
    state is UpdateCheck.Available -> "发现新版本 v${state.latest}（当前 v$OPI_VERSION）"
    state is UpdateCheck.Failed -> "检测失败：${state.reason}"
    else -> "尚未检测"
}

/**
 * 一次 HTTPS GET，返回 UTF-8 文本；失败（超时/非 2xx/断网/证书）→ null。
 * 只用于版本端点，**不携带任何用户数据**；`User-Agent` 里的版本号用于站点统计。
 */
internal fun httpGet(url: String, timeoutMs: Int = UPDATE_TIMEOUT_MS): String? {
    val conn = try {
        // URI.toURL()：JDK 20+ 把 URL(String) 构造器标了 deprecated（会走到不安全解析）。
        URI(url).toURL().openConnection() as? HttpURLConnection ?: return null
    } catch (_: Exception) {
        return null
    }
    return try {
        conn.connectTimeout = timeoutMs
        conn.readTimeout = timeoutMs
        conn.instanceFollowRedirects = true
        conn.setRequestProperty("User-Agent", "opi/$OPI_VERSION")
        conn.setRequestProperty("Accept", "application/json, text/html;q=0.9, */*;q=0.8")
        if (conn.responseCode !in 200..299) return null
        val bytes = conn.inputStream.use { it.readNBytes(UPDATE_MAX_BYTES) }
        String(bytes, Charsets.UTF_8)
    } catch (_: Exception) {
        null
    } finally {
        conn.disconnect()
    }
}
