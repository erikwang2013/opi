// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.clip

import android.Manifest
import android.content.ContentValues
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import java.io.File

/**
 * 入站传输的落点导入（2026-11-04 用户裁决「所有端保存到 Download/opi」）。
 *
 * 传输内核只会写普通文件路径，不能写 `content://`；Android 10+ 又不允许应用直接
 * 往公共 Download 写普通路径。所以内核先写 `ClipRuntime.incomingSaveDir` 给出的
 * 应用专属暂存目录，完成后由本文件导入公共 Download/opi：
 * · Android 10+：MediaStore.Downloads（RELATIVE_PATH="Download/opi"，子目录保留在 opi 下）；
 * · Android 9 及以下：直接写公共目录（需要存储权限，未授权则保留暂存目录）。
 *
 * 返回给 UI 的落点：全部成功 = 公共 Download/opi 绝对路径；部分失败/无权限 = 暂存路径，
 * 文件不丢。调用方必须在后台线程运行（大文件复制耗时）。
 */
internal fun importIncomingToPublicDownloads(context: Context, source: File): String? {
    if (!source.isDirectory) return null
    val files = source.walkTopDown().filter { it.isFile }.toList()
    if (files.isEmpty()) {
        source.deleteRecursively()
        return publicTransferPath()
    }
    return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
        importViaMediaStore(context, source, files)
    } else {
        importViaLegacyStorage(context, source, files)
    }
}

private fun publicTransferPath(): String =
    File(Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS), "opi").absolutePath

/** Android 10+：MediaStore.Downloads，RELATIVE_PATH 取 "Download/opi"。 */
private fun importViaMediaStore(context: Context, source: File, files: List<File>): String {
    val resolver = context.contentResolver
    var failed = 0
    for (file in files) {
        val parent = file.parentFile?.let { it.relativeTo(source).path }?.replace('\\', '/') ?: ""
        val relative = if (parent.isEmpty() || parent == ".") "Download/opi" else "Download/opi/$parent"
        val values = ContentValues().apply {
            put(MediaStore.Downloads.DISPLAY_NAME, file.name)
            put(MediaStore.Downloads.MIME_TYPE, mimeForName(file.name))
            put(MediaStore.Downloads.RELATIVE_PATH, relative)
            put(MediaStore.Downloads.IS_PENDING, 1)
        }
        val uri = runCatching {
            resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
        }.getOrNull()
        if (uri == null) {
            failed++
            continue
        }
        val copied = runCatching {
            resolver.openOutputStream(uri)?.use { out ->
                file.inputStream().use { input -> input.copyTo(out) }
            } ?: error("no output stream")
        }.isSuccess
        if (copied) {
            val done = ContentValues().apply { put(MediaStore.Downloads.IS_PENDING, 0) }
            runCatching { resolver.update(uri, done, null, null) }
        } else {
            runCatching { resolver.delete(uri, null, null) }
            failed++
        }
    }
    return if (failed == 0) {
        source.deleteRecursively()
        publicTransferPath()
    } else {
        source.absolutePath
    }
}

/** Android 9 及以下：直接写公共 Download/opi（需要存储权限；没授权就保留暂存目录）。 */
private fun importViaLegacyStorage(context: Context, source: File, files: List<File>): String {
    val granted = Build.VERSION.SDK_INT < Build.VERSION_CODES.M ||
        context.checkSelfPermission(Manifest.permission.WRITE_EXTERNAL_STORAGE) ==
        PackageManager.PERMISSION_GRANTED
    if (!granted) return source.absolutePath
    val public = File(publicTransferPath())
    var failed = 0
    for (file in files) {
        val dst = File(public, file.relativeTo(source).path)
        val ok = runCatching {
            dst.parentFile?.mkdirs()
            file.copyTo(dst, overwrite = true)
        }.isSuccess
        if (!ok) failed++
    }
    return if (failed == 0) {
        source.deleteRecursively()
        public.absolutePath
    } else {
        source.absolutePath
    }
}
