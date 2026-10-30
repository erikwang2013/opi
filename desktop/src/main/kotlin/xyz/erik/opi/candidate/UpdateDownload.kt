// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 应用内更新下载（2026-10-31 用户裁决：更新不再跳浏览器，后台下载并在「关于」显示
//! 进度；全量安装包与热更新包都走这一条）。**只下载 + 校验，不自动提权/替换**：
//! 下载完成后由用户点「打开文件」或「打开文件夹」。
//!
//! 安全边界：
//! * 只接受 HTTPS（清单解析与下载器各挡一次；测试可显式放开）；
//! * 有 SHA-256 就边下边算，完成后比对；不符删除并报错；清单没给就显示「未提供」；
//! * 有 size 就完成后复检字节数；坏包不会留在 updates 目录；
//! * 先写 `.part` 再原子改名；取消/失败都删临时文件。
//!
//! 后台语义：下载在进程级 IO scope 上，**关掉设置窗仍会继续**（进程还活着时）；
//! 整个应用退出则停止。Android 端用系统 DownloadManager，退出后仍继续。

package xyz.erik.opi.candidate

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.isActive
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.awt.Desktop
import java.io.File
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URI
import java.nio.file.Files
import java.nio.file.StandardCopyOption
import java.security.MessageDigest
import java.util.Locale
import kotlin.coroutines.coroutineContext

/**
 * 进程级下载器：设置窗关掉再开仍能看到同一个下载的进度（scope 也是进程级的）。
 * 整个进程退出才停止；Android 端由系统 DownloadManager 跨进程续传。
 */
internal val SharedUpdateDownloader: UpdateDownloader by lazy { UpdateDownloader() }

/** 两种包型；[id] 用于文件名/状态，[label] 用于界面。 */
internal enum class UpdatePackageKind(val id: String, val label: String) {
    FULL("full", "全量安装包"),
    HOT("hot", "热更新包"),
}

/** 下载器状态（Compose 可观察；从 IO 线程写 Snapshot 状态是项目既有做法）。 */
internal sealed interface DownloadState {
    data object Idle : DownloadState
    data class Running(
        val kind: UpdatePackageKind,
        val version: String,
        val downloaded: Long,
        val total: Long?,
    ) : DownloadState {
        val fraction: Float?
            get() = total?.takeIf { it > 0 }?.let { (downloaded.toFloat() / it).coerceIn(0f, 1f) }
    }

    data class Verifying(val kind: UpdatePackageKind, val version: String) : DownloadState
    data class Done(
        val kind: UpdatePackageKind,
        val version: String,
        val file: File,
        /** null = 清单没给校验值（如实显示，不冒充校验通过）。 */
        val shaOk: Boolean?,
    ) : DownloadState

    data class Failed(val kind: UpdatePackageKind, val version: String, val reason: String) : DownloadState
}

/** 下载上限：2 GiB，防坏清单/坏端点把磁盘写满。 */
private const val MAX_DOWNLOAD_BYTES = 2L * 1024 * 1024 * 1024

/** 更新包落点：Linux `$XDG_DATA_HOME/opi/updates`，Windows `%LOCALAPPDATA%\opi\updates`。 */
internal fun updatesDir(): File {
    val base = if (isWindows()) {
        System.getenv("LOCALAPPDATA")?.takeIf { it.isNotEmpty() }
            ?: System.getProperty("user.home")
    } else {
        System.getenv("XDG_DATA_HOME")?.takeIf { it.isNotEmpty() }
            ?: System.getenv("HOME")?.takeIf { it.isNotEmpty() }?.let { "$it/.local/share" }
            ?: System.getProperty("user.home")
    }
    return File(base, "opi/updates")
}

/** 从 URL 取一个安全的文件名（去掉 query、路径分隔符；过长时保留扩展名）。 */
internal fun downloadFileName(url: String, version: String, kind: UpdatePackageKind): String {
    val raw = runCatching { URI(url).path.substringAfterLast('/') }.getOrNull().orEmpty()
    // 没有文件名（比如 .../download/）时版本号里的点不能被当成扩展名，按包型给扩展名。
    if (raw.isBlank()) {
        return "opi-$version-${kind.id}." + if (kind == UpdatePackageKind.FULL) "bin" else "zip"
    }
    val safe = raw.replace(Regex("[^A-Za-z0-9._-]"), "_").take(160)
    val dot = safe.lastIndexOf('.')
    val stem = if (dot > 0) safe.substring(0, dot) else safe
    val ext = if (dot > 0) safe.substring(dot).take(12).ifEmpty { ".bin" } else ".bin"
    return stem.take(140) + ext
}

/** 人类可读大小；null/负值 = 未知。纯函数，单测钉住。 */
internal fun formatDownloadSize(n: Long?): String {
    if (n == null || n < 0) return "未知大小"
    if (n < 1024) return "$n B"
    val kb = 1024.0
    return when {
        n < 1024L * 1024 -> String.format(Locale.ROOT, "%.1f KB", n / kb)
        n < 1024L * 1024 * 1024 -> String.format(Locale.ROOT, "%.1f MB", n / (kb * kb))
        else -> String.format(Locale.ROOT, "%.2f GB", n / (kb * kb * kb))
    }
}

/**
 * 更新下载器。[requireHttps] 只给 JVM 单测的本地 HTTP 服务器放开；生产永远 true。
 * 自带进程级 scope：设置窗关了，只要状态栏进程还活着就继续下。
 */
internal class UpdateDownloader(
    private val scope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.IO),
    private val requireHttps: Boolean = true,
    private val targetDir: File = updatesDir(),
) {
    var state by mutableStateOf<DownloadState>(DownloadState.Idle)
        private set

    private var job: Job? = null

    @Volatile
    private var conn: HttpURLConnection? = null

    fun start(kind: UpdatePackageKind, pkg: UpdatePackage, version: String) {
        job?.cancel()
        conn?.disconnect()
        if (requireHttps && !pkg.url.startsWith("https://")) {
            state = DownloadState.Failed(kind, version, "下载地址不是 HTTPS，已拒绝")
            return
        }
        state = DownloadState.Running(kind, version, 0, pkg.size)
        job = scope.launch {
            try {
                download(kind, pkg, version)
            } catch (e: CancellationException) {
                throw e
            } catch (t: Throwable) {
                // 取消后 conn.disconnect() 可能先抛 IO 异常；协程已取消时不要再盖成失败。
                if (coroutineContext.isActive) {
                    state = DownloadState.Failed(kind, version, t.message ?: t.javaClass.simpleName)
                }
            }
        }
    }

    fun cancel() {
        job?.cancel()
        conn?.disconnect()
        state = DownloadState.Idle
    }

    /** 打开下载好的文件（.deb/.exe/.zip 交给系统默认程序）；返回 null = 已交给系统。 */
    fun openFile(file: File): String? = runCatching {
        if (!Desktop.isDesktopSupported()) return "系统不支持自动打开文件"
        Desktop.getDesktop().open(file)
        null
    }.getOrElse { "打不开文件：${it.message ?: it.javaClass.simpleName}" }

    /** 打开下载目录。 */
    fun openFolder(file: File): String? = runCatching {
        if (!Desktop.isDesktopSupported()) return "系统不支持自动打开文件夹"
        Desktop.getDesktop().open(file.parentFile ?: updatesDir())
        null
    }.getOrElse { "打不开文件夹：${it.message ?: it.javaClass.simpleName}" }

    private suspend fun download(kind: UpdatePackageKind, pkg: UpdatePackage, version: String) =
        withContext(Dispatchers.IO) {
            val dir = targetDir.apply { mkdirs() }
            val target = File(dir, downloadFileName(pkg.url, version, kind))
            val part = File(dir, "${target.name}.part")
            part.delete()
            var ok = false
            try {
                val c = (URI(pkg.url).toURL().openConnection() as HttpURLConnection).apply {
                    connectTimeout = 15_000
                    readTimeout = 30_000
                    instanceFollowRedirects = true
                    setRequestProperty("User-Agent", "opi/$OPI_VERSION")
                    setRequestProperty("Accept", "application/octet-stream, */*")
                }
                conn = c
                try {
                    if (c.responseCode !in 200..299) error("服务器返回 HTTP ${c.responseCode}")
                    // 跟随重定向后最终 URL 也必须是 HTTPS（防 https → http 降级）。
                    if (requireHttps && !c.url.protocol.equals("https", ignoreCase = true)) {
                        error("下载被重定向到非 HTTPS，已拒绝")
                    }
                    val total = pkg.size ?: c.contentLengthLong.takeIf { it > 0 }
                    if (total != null && total > MAX_DOWNLOAD_BYTES) error("安装包超过 2 GiB")
                    val digest = MessageDigest.getInstance("SHA-256")
                    var downloaded = 0L
                    var lastUi = 0L
                    c.inputStream.use { input ->
                        FileOutputStream(part).use { output ->
                            val buf = ByteArray(64 * 1024)
                            while (true) {
                                coroutineContext.ensureActive()
                                val n = input.read(buf)
                                if (n < 0) break
                                output.write(buf, 0, n)
                                digest.update(buf, 0, n)
                                downloaded += n
                                if (downloaded > MAX_DOWNLOAD_BYTES) error("安装包超过 2 GiB")
                                val now = System.nanoTime()
                                if (now - lastUi > 100_000_000L || downloaded == total) {
                                    state = DownloadState.Running(kind, version, downloaded, total)
                                    lastUi = now
                                }
                            }
                        }
                    }
                    if (pkg.size != null && part.length() != pkg.size) {
                        error("大小校验失败：期望 ${formatDownloadSize(pkg.size)}，实际 ${formatDownloadSize(part.length())}")
                    }
                    state = DownloadState.Verifying(kind, version)
                    val actual = digest.digest().joinToString("") { "%02x".format(it.toInt() and 0xFF) }
                    val shaOk = pkg.sha256?.let { it.equals(actual, ignoreCase = true) }
                    if (shaOk == false) error("SHA-256 校验失败")
                    Files.move(part.toPath(), target.toPath(), StandardCopyOption.REPLACE_EXISTING)
                    ok = true
                    state = DownloadState.Done(kind, version, target, shaOk)
                } finally {
                    conn = null
                    c.disconnect()
                }
            } finally {
                if (!ok) part.delete()
            }
        }
}
