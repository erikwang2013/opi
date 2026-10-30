// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.net.HttpURLConnection
import java.net.URL

/**
 * 新版本检测（2026-10-22）：数据源是官网的 `version.json`，失败再兜底解析首页的
 * JSON-LD `softwareVersion`。**只访问这两个官网端点，不上传任何用户数据。**
 *
 * 不用 `org.json`：JVM 单测里它只有 android.jar 的空壳（方法一调就抛「not mocked」），
 * 而这两个端点的载荷是固定的单字段 JSON —— 正则足够，且解析函数因此可单测。
 */
internal const val OPI_VERSION_URL = "https://opi.erik.xyz/version.json"
internal const val OPI_HOME_URL = "https://opi.erik.xyz/"
internal const val OPI_DOWNLOAD_URL = "https://opi.erik.xyz/download"

/** 整次检测的时间预算（毫秒）。UI 侧在 IO 线程跑，不阻塞输入。 */
internal const val VERSION_CHECK_TIMEOUT_MS = 4_000

private val JSON_VERSION = Regex(""""version"\s*:\s*"([^"]+)"""")
private val JSON_LD_VERSION = Regex(""""softwareVersion"\s*:\s*"([^"]+)"""")
private val SHA256_RE = Regex("""^[0-9a-fA-F]{64}$""")

/** 一个可直下的更新包；[sha256] 缺省 = 站点没给校验值。 */
internal data class UpdatePackage(val url: String, val sha256: String?, val size: Long?)

/** `version.json` 解析结果（Android 取 `packages.android`，桌面取各自平台键）。 */
internal data class RemoteRelease(
    val version: String,
    val download: String = OPI_DOWNLOAD_URL,
    val full: UpdatePackage? = null,
    val hot: UpdatePackage? = null,
)

/** `version.json` 的 `{"version":"x.y.z","download":"..."}`（老形状，单测/兜底用）。 */
internal fun parseVersionJson(text: String): String? =
    JSON_VERSION.find(text)?.groupValues?.get(1)?.trim()?.takeIf { it.isNotEmpty() }

/** 找到 `"key": { ... }` 的平衡花括号块（字符串/转义内的括号不算）。 */
internal fun jsonObjectAt(text: String, key: String): String? {
    val m = Regex(""""$key"\s*:""").find(text) ?: return null
    var i = m.range.last + 1
    while (i < text.length && text[i].isWhitespace()) i++
    if (i >= text.length || text[i] != '{') return null
    val start = i
    var depth = 0
    var inStr = false
    var esc = false
    while (i < text.length) {
        val c = text[i]
        if (inStr) {
            when {
                esc -> esc = false
                c == '\\' -> esc = true
                c == '"' -> inStr = false
            }
        } else {
            when (c) {
                '"' -> inStr = true
                '{' -> depth++
                '}' -> {
                    depth--
                    if (depth == 0) return text.substring(start, i + 1)
                }
            }
        }
        i++
    }
    return null
}

private fun jsonString(obj: String, key: String): String? =
    Regex(""""$key"\s*:\s*"([^"]*)"""").find(obj)?.groupValues?.get(1)

private fun jsonLong(obj: String, key: String): Long? =
    Regex(""""$key"\s*:\s*(-?\d+)""").find(obj)?.groupValues?.get(1)?.toLongOrNull()

private fun parsePackage(obj: String): UpdatePackage? {
    val url = jsonString(obj, "url")?.trim()?.takeIf { it.startsWith("https://") } ?: return null
    val sha = jsonString(obj, "sha256")?.trim()?.takeIf { SHA256_RE.matches(it) }?.lowercase()
    val size = jsonLong(obj, "size")?.takeIf { it > 0 }
    return UpdatePackage(url, sha, size)
}

/**
 * 解析 `version.json` 的包清单（Android 只认 `packages.android`，兼容
 * `android-universal`）。任何字段坏了都退成 null，不猜 URL。
 */
internal fun parseReleaseJson(text: String, platform: String = "android"): RemoteRelease? {
    val version = jsonString(text, "version") ?: jsonString(text, "latest")
    val v = version?.trim()?.removePrefix("v")?.takeIf { it.isNotEmpty() } ?: return null
    val download = jsonString(text, "download")?.takeIf { it.startsWith("https://") }
        ?: OPI_DOWNLOAD_URL
    val packages = jsonObjectAt(text, "packages")
    val platformObj = packages?.let { jsonObjectAt(it, platform) }
        ?: packages?.let { jsonObjectAt(it, "android-universal") }
    val full = platformObj?.let { jsonObjectAt(it, "full") }?.let(::parsePackage)
    val hot = platformObj?.let { jsonObjectAt(it, "hot") }?.let(::parsePackage)
    return RemoteRelease(v, download, full, hot)
}

/** 兜底：首页 JSON-LD 里的 `"softwareVersion":"x.y.z"`。 */
internal fun parseJsonLdVersion(html: String): String? =
    JSON_LD_VERSION.find(html)?.groupValues?.get(1)?.trim()?.takeIf { it.isNotEmpty() }

/**
 * 语义化数字比较：逐段按整数比（`1.21.10 > 1.21.2`）；缺段按 0（`1.22 == 1.22.0`）；
 * 允许前导 `v`。任一侧不是纯数字点分串就返回 null（**不猜**，宁可当成「无更新」）。
 */
internal fun compareVersions(remote: String, current: String): Int? {
    fun parts(s: String): List<Int>? {
        val raw = s.trim().removePrefix("v").removePrefix("V")
        if (raw.isEmpty()) return null
        return raw.split('.').map { it.trim().toIntOrNull() ?: return null }
    }
    val a = parts(remote) ?: return null
    val b = parts(current) ?: return null
    for (i in 0 until maxOf(a.size, b.size)) {
        val x = a.getOrElse(i) { 0 }
        val y = b.getOrElse(i) { 0 }
        if (x != y) return if (x > y) 1 else -1
    }
    return 0
}

internal fun isNewerVersion(remote: String, current: String): Boolean =
    (compareVersions(remote, current) ?: 0) > 0

/**
 * 拉最新版本：先 `version.json`，任何失败（网络/HTTP/格式）再打首页 JSON-LD。
 * 总预算 [VERSION_CHECK_TIMEOUT_MS]，两次请求共用。
 *
 * 「前往下载」的目标固定为 [OPI_DOWNLOAD_URL]，**不读**端点里的 `download` 字段 ——
 * 不让网络数据决定要跳去哪。
 */
internal fun fetchLatestRelease(): Result<RemoteRelease> = runCatching {
    val deadline = System.currentTimeMillis() + VERSION_CHECK_TIMEOUT_MS
    fun budget(): Int {
        val left = deadline - System.currentTimeMillis()
        if (left <= 0) error("检测超时")
        return left.toInt()
    }
    runCatching { parseReleaseJson(httpGet(OPI_VERSION_URL, budget())) }.getOrNull()
        ?: parseJsonLdVersion(httpGet(OPI_HOME_URL, budget()))?.let { RemoteRelease(it) }
        ?: error("版本端点没有可用版本号")
}

private fun httpGet(url: String, timeoutMs: Int): String {
    val conn = (URL(url).openConnection() as HttpURLConnection).apply {
        connectTimeout = timeoutMs
        readTimeout = timeoutMs
        requestMethod = "GET"
    }
    try {
        if (conn.responseCode !in 200..299) error("HTTP ${conn.responseCode}")
        return conn.inputStream.bufferedReader().use { it.readText() }
    } finally {
        conn.disconnect()
    }
}

/** 更新检测的 UI 状态。自动检测失败静默回 [Idle]；只有手动检测才展示失败原因。 */
internal sealed interface UpdateState {
    object Idle : UpdateState
    object Checking : UpdateState

    /** 新版本；[full]/[hot] = 站点点名的可直下包（没有就只留下载页兜底）。 */
    data class Newer(
        val version: String,
        val full: UpdatePackage? = null,
        val hot: UpdatePackage? = null,
        val download: String = OPI_DOWNLOAD_URL,
    ) : UpdateState

    object Latest : UpdateState
    data class Failed(val reason: String) : UpdateState
}

/**
 * 检测一次并折算成 UI 状态（IO 线程，两处关于面板共用）。
 * [manual] = false 时失败不打扰：回 [UpdateState.Idle]，界面仍只显示当前版本。
 */
internal suspend fun checkForUpdate(currentVersion: String, manual: Boolean): UpdateState =
    withContext(Dispatchers.IO) {
        fetchLatestRelease().fold(
            onSuccess = { release ->
                if (isNewerVersion(release.version, currentVersion)) {
                    UpdateState.Newer(
                        version = release.version,
                        full = release.full,
                        hot = release.hot,
                        download = release.download,
                    )
                } else {
                    UpdateState.Latest
                }
            },
            onFailure = { e ->
                if (manual) UpdateState.Failed(e.message ?: "网络不可用") else UpdateState.Idle
            },
        )
    }

/** 「更新」一行的状态文案（设置页、键盘关于/设置面板共用同一口径）。 */
internal fun updateStatusText(state: UpdateState, currentVersion: String): String = when (state) {
    UpdateState.Idle -> "当前 v$currentVersion"
    UpdateState.Checking -> "正在检测更新…"
    is UpdateState.Newer -> "发现新版本 v${state.version}（当前 v$currentVersion）"
    UpdateState.Latest -> "已是最新版本 v$currentVersion"
    is UpdateState.Failed -> "检测更新失败：${state.reason}"
}
