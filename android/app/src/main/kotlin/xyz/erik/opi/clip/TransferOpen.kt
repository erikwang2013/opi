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

/**
 * 打开接收目录（2026-11-04 提取为共享：键盘传输面板与应用设置页共用）。
 * 公共 Download 走系统下载页；暂存目录走 FileProvider。失败时返回给用户看的路径，
 * 不静默失败。
 */
internal fun openSavedFolder(context: Context, dir: String): String {
    val public = Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS).absolutePath
    if (dir == public || dir.startsWith("$public/") || dir == "Download" || dir.startsWith("Download/")) {
        return try {
            context.startActivity(
                Intent(DownloadManager.ACTION_VIEW_DOWNLOADS)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            )
            ""
        } catch (_: ActivityNotFoundException) {
            "已保存到：$dir"
        }
    }
    val file = File(dir)
    if (!file.isDirectory) return "目录不存在（可能已被系统清理）：$dir"
    return try {
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
        context.startActivity(
            Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, DocumentsContract.Document.MIME_TYPE_DIR)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
            },
        )
        ""
    } catch (_: ActivityNotFoundException) {
        "已保存到：$dir"
    } catch (_: IllegalArgumentException) {
        "已保存到：$dir"
    }
}
