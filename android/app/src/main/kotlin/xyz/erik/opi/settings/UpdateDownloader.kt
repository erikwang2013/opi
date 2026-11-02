// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Android 应用内更新下载（2026-10-31 用户裁决：不再跳浏览器，后台下载 + 关于里显示
//! 进度；全量与热更新都走这一条）。用系统 **DownloadManager**：退出 App/键盘后仍继续，
//! 完成由系统通知；本类只负责入队、轮询进度、完成后校验 SHA-256。
//!
//! 落点是系统 Downloads 下的 `opi/`（`setDestinationInExternalPublicDir`，由系统
//! DownloadProvider 写入，API 21 起免存储权限）。Android 10+ 不能直接读公共目录路径，
//! 校验走 `openDownloadedFile` 的 fd，安装/打开用 `getUriForDownloadedFile` 的内容 URI
//! 授权给系统安装器/查看器。**只下载 + 校验，不自动安装**；用户点「安装/打开」后仍由
//! 系统二次确认。
//!
//! 状态放在进程级单例 + SharedPreferences 记录下载 ID：进程被杀再回来，DownloadManager
//! 还在下，`resume()` 按 ID 重新挂上轮询。

package xyz.erik.opi.settings

import android.app.DownloadManager
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Environment
import android.os.ParcelFileDescriptor
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import java.security.MessageDigest

internal enum class UpdatePackageKind(val id: String, val label: String) {
    FULL("full", "全量安装包"),
    HOT("hot", "热更新包"),
}

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
        val fileName: String,
        val uri: Uri,
        /** null = 清单没给校验值（如实显示，不冒充校验通过）。 */
        val shaOk: Boolean?,
    ) : DownloadState

    data class Failed(val kind: UpdatePackageKind, val version: String, val reason: String) : DownloadState
}

/** 人类可读大小（与桌面端同一口径）；null/负值 = 未知。 */
internal fun formatDownloadSize(n: Long?): String {
    if (n == null || n < 0) return "未知大小"
    if (n < 1024) return "$n B"
    val kb = 1024.0
    return when {
        n < 1024L * 1024 -> String.format(java.util.Locale.ROOT, "%.1f KB", n / kb)
        n < 1024L * 1024 * 1024 -> String.format(java.util.Locale.ROOT, "%.1f MB", n / (kb * kb))
        else -> String.format(java.util.Locale.ROOT, "%.2f GB", n / (kb * kb * kb))
    }
}

/** URL → 安全文件名（保留扩展名；版本号里的点不会被误当扩展名）。 */
internal fun downloadFileName(url: String, version: String, kind: UpdatePackageKind): String {
    // 用 java.net.URI 而不是 android.net.Uri：JVM 单测里的 android.jar 是空壳，Uri 会抛。
    val raw = runCatching { java.net.URI(url).path.substringAfterLast('/') }.getOrNull().orEmpty()
    if (raw.isBlank()) {
        return "opi-$version-${kind.id}." + if (kind == UpdatePackageKind.FULL) "apk" else "zip"
    }
    val safe = raw.replace(Regex("[^A-Za-z0-9._-]"), "_").take(160)
    val dot = safe.lastIndexOf('.')
    val stem = if (dot > 0) safe.substring(0, dot) else safe
    val ext = if (dot > 0) safe.substring(dot).take(12).ifEmpty { ".bin" } else ".bin"
    return stem.take(140) + ext
}

internal object UpdateDownloader {
    private const val PREFS = "opi_update_download"

    /** 系统 Downloads 下的子目录（与桌面端 `Downloads/opi` 对齐）。 */
    private const val PUBLIC_SUBDIR = "opi"
    private const val KEY_ID = "download_id"
    private const val KEY_KIND = "kind"
    private const val KEY_VERSION = "version"
    private const val KEY_SHA = "sha256"
    private const val KEY_SIZE = "size"
    private const val KEY_FILE = "file_name"
    private const val KEY_URL = "url"

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val _state = MutableStateFlow<DownloadState>(DownloadState.Idle)
    val state: StateFlow<DownloadState> = _state.asStateFlow()
    private var pollJob: Job? = null

    /** 关于页每次打开时调；有未完成任务就重新挂上轮询（进程重启后也能续上）。 */
    fun resume(context: Context) {
        val ctx = context.applicationContext
        if (pollJob?.isActive == true) return
        val p = ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val id = p.getLong(KEY_ID, -1L)
        if (id <= 0) return
        val kind = UpdatePackageKind.entries.firstOrNull { it.id == p.getString(KEY_KIND, null) } ?: return
        val version = p.getString(KEY_VERSION, null) ?: return
        val url = p.getString(KEY_URL, null) ?: return
        val fileName = p.getString(KEY_FILE, null) ?: downloadFileName(url, version, kind)
        val sha = p.getString(KEY_SHA, null)
        val size = p.getLong(KEY_SIZE, -1L).takeIf { it > 0 }
        poll(ctx, id, kind, version, fileName, sha, size)
    }

    fun start(context: Context, kind: UpdatePackageKind, pkg: UpdatePackage, version: String) {
        val ctx = context.applicationContext
        if (!pkg.url.startsWith("https://")) {
            _state.value = DownloadState.Failed(kind, version, "下载地址不是 HTTPS，已拒绝")
            return
        }
        pollJob?.cancel()
        val fileName = downloadFileName(pkg.url, version, kind)
        pollJob = scope.launch {
            _state.value = DownloadState.Verifying(kind, version)
            val dm = downloadManager(ctx)
            // 构造 Request 也可能抛（如外部存储不可用），和 enqueue 一起兜住。
            val id = runCatching {
                val request = DownloadManager.Request(Uri.parse(pkg.url))
                    .setTitle("小欧输入法 v$version")
                    .setDescription(kind.label)
                    .setNotificationVisibility(DownloadManager.Request.VISIBILITY_VISIBLE_NOTIFY_COMPLETED)
                    // 用户可见的系统 Downloads/opi；写入由 DownloadProvider 完成，不需要存储权限。
                    .setDestinationInExternalPublicDir(Environment.DIRECTORY_DOWNLOADS, "$PUBLIC_SUBDIR/$fileName")
                    .setAllowedOverRoaming(false)
                    .addRequestHeader("User-Agent", "opi/${xyz.erik.opi.BuildConfig.VERSION_NAME}")
                dm.enqueue(request)
            }.getOrElse { e ->
                _state.value = DownloadState.Failed(kind, version, "无法开始下载：${e.message ?: e.javaClass.simpleName}")
                return@launch
            }
            ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit()
                .putLong(KEY_ID, id)
                .putString(KEY_KIND, kind.id)
                .putString(KEY_VERSION, version)
                .putString(KEY_SHA, pkg.sha256)
                .putLong(KEY_SIZE, pkg.size ?: -1L)
                .putString(KEY_FILE, fileName)
                .putString(KEY_URL, pkg.url)
                .apply()
            _state.value = DownloadState.Running(kind, version, 0, pkg.size)
            // 本协程即将结束；把句柄交给 poll 的独立协程，别再让它取消自己。
            pollJob = null
            poll(ctx, id, kind, version, fileName, pkg.sha256, pkg.size)
        }
    }

    fun cancel(context: Context) {
        val ctx = context.applicationContext
        val p = ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val id = p.getLong(KEY_ID, -1L)
        if (id > 0) runCatching { downloadManager(ctx).remove(id) }
        clearPrefs(ctx)
        pollJob?.cancel()
        _state.value = DownloadState.Idle
    }

    /** 点「安装/打开」：APK 交给系统安装器，zip 等交给能接的应用。返回 null = 已拉起。 */
    fun open(context: Context, done: DownloadState.Done): String? {
        val intent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(done.uri, mimeOf(done.fileName))
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        return try {
            context.startActivity(intent)
            null
        } catch (_: ActivityNotFoundException) {
            "没有能打开 ${done.fileName} 的应用"
        }
    }

    private fun poll(
        ctx: Context,
        id: Long,
        kind: UpdatePackageKind,
        version: String,
        fileName: String,
        sha: String?,
        size: Long?,
    ) {
        pollJob?.cancel()
        pollJob = scope.launch {
            val dm = downloadManager(ctx)
            while (isActive) {
                val q = DownloadManager.Query().setFilterById(id)
                dm.query(q).use { c ->
                    if (c == null || !c.moveToFirst()) {
                        fail(ctx, kind, version, "下载任务丢失")
                        return@launch
                    }
                    val status = c.getInt(c.getColumnIndexOrThrow(DownloadManager.COLUMN_STATUS))
                    val downloaded = c.getLong(c.getColumnIndexOrThrow(DownloadManager.COLUMN_BYTES_DOWNLOADED_SO_FAR))
                    val total = c.getLong(c.getColumnIndexOrThrow(DownloadManager.COLUMN_TOTAL_SIZE_BYTES))
                        .takeIf { it > 0 } ?: size
                    when (status) {
                        DownloadManager.STATUS_RUNNING,
                        DownloadManager.STATUS_PENDING,
                        DownloadManager.STATUS_PAUSED,
                        -> _state.value = DownloadState.Running(kind, version, downloaded, total)

                        DownloadManager.STATUS_SUCCESSFUL -> {
                            _state.value = DownloadState.Verifying(kind, version)
                            verify(ctx, kind, version, id, fileName, sha, size)
                            return@launch
                        }
                        DownloadManager.STATUS_FAILED -> {
                            val reason = c.getInt(c.getColumnIndexOrThrow(DownloadManager.COLUMN_REASON))
                            fail(ctx, kind, version, "系统下载失败（$reason）")
                            return@launch
                        }
                    }
                }
                delay(500)
            }
        }
    }

    private fun verify(
        ctx: Context,
        kind: UpdatePackageKind,
        version: String,
        id: Long,
        fileName: String,
        sha: String?,
        size: Long?,
    ) {
        val dm = downloadManager(ctx)
        val pfd = runCatching { dm.openDownloadedFile(id) }.getOrNull()
        if (pfd == null) {
            fail(ctx, kind, version, "下载完成但无法读取文件")
            return
        }
        // AutoCloseInputStream 关闭流时一并关掉 PFD；别再单独 close（同一 fd 关两次
        // 在多线程进程里可能误关被复用的 fd）。
        val measured = runCatching {
            ParcelFileDescriptor.AutoCloseInputStream(pfd).use { input ->
                readSizeAndSha(input, sha != null)
            }
        }.getOrElse { e ->
            runCatching { pfd.close() }
            fail(ctx, kind, version, "读取下载文件失败：${e.message ?: e.javaClass.simpleName}")
            return
        }
        val (actualSize, actualSha) = measured
        if (size != null && actualSize != size) {
            runCatching { dm.remove(id) }
            fail(ctx, kind, version, "大小校验失败：期望 ${formatDownloadSize(size)}，实际 ${formatDownloadSize(actualSize)}")
            return
        }
        val shaOk = sha?.let { it.equals(actualSha, ignoreCase = true) }
        if (shaOk == false) {
            runCatching { dm.remove(id) }
            fail(ctx, kind, version, "SHA-256 校验失败，已删除下载文件")
            return
        }
        val uri = runCatching { dm.getUriForDownloadedFile(id) }.getOrNull()
        if (uri == null) {
            fail(ctx, kind, version, "无法获取下载文件（任务 $id）")
            return
        }
        clearPrefs(ctx)
        _state.value = DownloadState.Done(kind, version, fileName, uri, shaOk)
    }

    /** 读下载流：返回 (字节数, SHA-256 或 null)；关闭由调用方的 `use` 负责。 */
    private fun readSizeAndSha(input: java.io.InputStream, wantSha: Boolean): Pair<Long, String?> {
        val md = if (wantSha) MessageDigest.getInstance("SHA-256") else null
        var total = 0L
        val buf = ByteArray(64 * 1024)
        while (true) {
            val n = input.read(buf)
            if (n < 0) break
            total += n
            md?.update(buf, 0, n)
        }
        return total to md?.digest()?.joinToString("") { "%02x".format(it.toInt() and 0xFF) }
    }

    private fun fail(ctx: Context, kind: UpdatePackageKind, version: String, reason: String) {
        clearPrefs(ctx)
        _state.value = DownloadState.Failed(kind, version, reason)
    }

    private fun clearPrefs(ctx: Context) {
        ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().clear().apply()
    }

    private fun downloadManager(ctx: Context): DownloadManager =
        ctx.getSystemService(Context.DOWNLOAD_SERVICE) as DownloadManager

    private fun mimeOf(name: String): String = when {
        name.endsWith(".apk", ignoreCase = true) -> "application/vnd.android.package-archive"
        name.endsWith(".zip", ignoreCase = true) -> "application/zip"
        else -> "*/*"
    }
}
