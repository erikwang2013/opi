// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.clip

import android.app.DownloadManager
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.os.Environment
import android.provider.DocumentsContract
import androidx.core.content.FileProvider
import java.io.File

private const val EXTERNAL_STORAGE_AUTHORITY = "com.android.externalstorage.documents"

/**
 * 公共 Download 下的相对路径 → ExternalStorageProvider 文档 ID。
 * 只认 Download 根及其子目录；`Downloaded/...` 这类前缀相似目录不算。
 */
internal fun downloadDocumentId(dir: String, publicRoot: String): String? {
    val root = publicRoot.trimEnd('/')
    val relative = when {
        dir == root -> ""
        dir.startsWith("$root/") -> dir.removePrefix("$root/")
        dir == "Download" -> ""
        dir.startsWith("Download/") -> dir.removePrefix("Download/")
        else -> return null
    }.trim('/')
    return "primary:Download" + if (relative.isEmpty()) "" else "/$relative"
}

/**
 * 打开接收目录（2026-11-04 提取为共享：键盘传输面板与应用设置页共用）。
 * 公共 Download 用 ExternalStorageProvider 直接打开具体子目录（不再只落到下载首页）；
 * 暂存目录走 FileProvider。都失败时返回给用户看的路径，不静默失败。
 */
internal fun openSavedFolder(context: Context, dir: String): String {
    val public = Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS).absolutePath
    val documentId = downloadDocumentId(dir, public)
    if (documentId != null) {
        val uri = DocumentsContract.buildDocumentUri(EXTERNAL_STORAGE_AUTHORITY, documentId)
        if (openDirectory(context, uri)) return ""
        if (openDownloads(context)) return ""
        return "已保存到：$dir"
    }
    val file = File(dir)
    if (!file.isDirectory) return "目录不存在（可能已被系统清理）：$dir"
    val uri = runCatching {
        FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
    }.getOrNull() ?: return "已保存到：$dir"
    if (openDirectory(context, uri)) return ""
    if (openDownloads(context)) return ""
    return "已保存到：$dir"
}

private fun openDirectory(context: Context, uri: android.net.Uri): Boolean = try {
    context.startActivity(
        Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(uri, DocumentsContract.Document.MIME_TYPE_DIR)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
        },
    )
    true
} catch (_: ActivityNotFoundException) {
    false
} catch (_: SecurityException) {
    false
} catch (_: IllegalArgumentException) {
    false
}

/** 最后兜底：能打开系统下载页也比只显示一行路径强。 */
private fun openDownloads(context: Context): Boolean = try {
    context.startActivity(
        Intent(DownloadManager.ACTION_VIEW_DOWNLOADS)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
    )
    true
} catch (_: ActivityNotFoundException) {
    false
}
