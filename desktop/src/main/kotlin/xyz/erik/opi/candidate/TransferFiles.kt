// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 文件传输的宿主侧：选择器、清单 JSON、发送前的大小/条目预检。
//!
//! 预检只为「别把必然失败的传输送进内核」：真正的信任、路径遍历与并发限制仍在
//! `opi-clip`（`transfer::validate_send`）。目录在预检里也递归一遍，所以命中上限时
//! 能在点击发送后立刻给出原因，而不是等 `send_files` 返回 last_error。

package xyz.erik.opi.candidate

import java.awt.Desktop
import java.awt.KeyboardFocusManager
import java.io.File
import java.nio.file.FileVisitResult
import java.nio.file.Files
import java.nio.file.LinkOption
import java.nio.file.Path
import java.nio.file.Paths
import java.nio.file.SimpleFileVisitor
import java.nio.file.attribute.BasicFileAttributes
import javax.swing.JFileChooser
import javax.swing.SwingUtilities
import javax.swing.filechooser.FileNameExtensionFilter

/** 与 `opi-clip` 的 `MAX_TRANSFER_*` 同值；超限在点击发送时前置拒绝。 */
internal const val TRANSFER_MAX_ENTRIES = 128
internal const val TRANSFER_MAX_FILE_BYTES = 2L * 1024 * 1024 * 1024
internal const val TRANSFER_MAX_TOTAL_BYTES = 8L * 1024 * 1024 * 1024

/** 控制面一行最多 1 MiB；文件清单远小于它，留足 JSON 转义余量。 */
internal const val TRANSFER_CONTROL_MAX_CHARS = 512 * 1024

/** `[{"path":abs,"name":可选,"mime":可选}]`；目录只给 path（内核递归展开）。 */
internal data class TransferPick(val path: String, val name: String? = null, val mime: String? = null)

internal sealed interface TransferPrep {
    data class Ready(val json: String, val entries: Int, val bytes: Long) : TransferPrep
    data class Rejected(val reason: String) : TransferPrep
}

internal fun encodeTransferFiles(picks: List<TransferPick>): String = buildString {
    append('[')
    picks.forEachIndexed { i, pick ->
        if (i > 0) append(',')
        append("{\"path\":").append(jsonString(pick.path))
        pick.name?.let { append(",\"name\":").append(jsonString(it)) }
        pick.mime?.let { append(",\"mime\":").append(jsonString(it)) }
        append('}')
    }
    append(']')
}

/**
 * 控制面入口的清单校验（不可信输入）：只接受绝对路径、上限内条目。
 * `name`/`mime` 原样保留，真正合法性由 FFI 的 `parse_files` 再判。
 */
internal fun parseTransferFiles(json: String): List<TransferPick>? {
    if (json.length > TRANSFER_CONTROL_MAX_CHARS) return null
    val root = parseLine("{\"files\":$json}") ?: return null
    val arr = root["files"] as? JVal.JArr ?: return null
    if (arr.v.isEmpty() || arr.v.size > TRANSFER_MAX_ENTRIES) return null
    val out = ArrayList<TransferPick>(arr.v.size)
    for (value in arr.v) {
        val m = (value as? JVal.JObj)?.v ?: return null
        val path = (m["path"] as? JVal.JStr)?.v ?: return null
        if (path.isEmpty() || path.contains('\u0000')) return null
        val absolute = try {
            Paths.get(path).isAbsolute
        } catch (_: Exception) {
            false
        }
        if (!absolute) return null
        val name = (m["name"] as? JVal.JStr)?.v
        val mime = (m["mime"] as? JVal.JStr)?.v
        if (name != null && (name.isEmpty() || name.length > 256)) return null
        if (mime != null && mime.length > 128) return null
        out += TransferPick(path, name, mime)
    }
    return out
}

/** 预检：路径合法 + 递归扫描 + 上限判断，最后生成 FFI 要的清单 JSON。 */
internal fun prepareTransferPaths(paths: List<String>): TransferPrep {
    if (paths.isEmpty()) return TransferPrep.Rejected("未选择任何文件")
    if (paths.size > TRANSFER_MAX_ENTRIES) {
        return TransferPrep.Rejected("一次最多 ${TRANSFER_MAX_ENTRIES} 项，请减少选择")
    }
    val roots = ArrayList<Path>(paths.size)
    for (raw in paths) {
        if (raw.isEmpty() || raw.contains('\u0000')) return TransferPrep.Rejected("路径不合法")
        val path = try {
            Paths.get(raw)
        } catch (_: Exception) {
            return TransferPrep.Rejected("路径不合法：$raw")
        }
        if (!path.isAbsolute) return TransferPrep.Rejected("只能发送绝对路径：$raw")
        roots.add(path.normalize())
    }
    if (roots.distinct().size != roots.size) return TransferPrep.Rejected("选择了重复路径")
    val scan = scanTransfer(roots)
    scan.error?.let { return TransferPrep.Rejected(it) }
    val json = encodeTransferFiles(roots.map { TransferPick(it.toString()) })
    if (json.length > TRANSFER_CONTROL_MAX_CHARS) {
        return TransferPrep.Rejected("路径清单过大，请减少选择")
    }
    return TransferPrep.Ready(json, scan.entries, scan.bytes)
}

internal data class TransferScan(val entries: Int, val bytes: Long, val error: String?)

/** 纯判定（单测用）：任一条超限即返回中文原因，否则 null。 */
internal fun transferLimitError(entries: Int, totalBytes: Long, largestFile: Long): String? = when {
    entries > TRANSFER_MAX_ENTRIES -> "超过 $TRANSFER_MAX_ENTRIES 项上限（文件夹会递归计数），未发送"
    largestFile > TRANSFER_MAX_FILE_BYTES -> "单个文件超过 2 GiB 上限，未发送"
    totalBytes > TRANSFER_MAX_TOTAL_BYTES -> "总大小超过 8 GiB 上限，未发送"
    else -> null
}

/**
 * 递归扫描（不跟随符号链接，与内核 `collect_files` 同口径）：目录本身也计一项。
 * 命中上限立即终止，避免为一个注定被拒的选择遍历整棵大树。
 */
internal fun scanTransfer(roots: List<Path>): TransferScan {
    var entries = 0
    var bytes = 0L
    var largest = 0L
    var error: String? = null

    fun add(size: Long) {
        if (error != null) return
        entries++
        if (size > largest) largest = size
        if (size > 0) bytes += size
        error = transferLimitError(entries, bytes, largest)
    }

    for (root in roots) {
        if (error != null) break
        val attrs = try {
            Files.readAttributes(root, BasicFileAttributes::class.java, LinkOption.NOFOLLOW_LINKS)
        } catch (t: Throwable) {
            error = "无法读取：${root.fileName ?: root}（${t.javaClass.simpleName}）"
            break
        }
        if (attrs.isSymbolicLink) {
            error = "不支持符号链接：${root.fileName ?: root}"
            break
        }
        if (!attrs.isDirectory) {
            add(attrs.size())
            continue
        }
        add(0L) // 目录条目本身也进 offer
        if (error != null) break
        try {
            Files.walkFileTree(
                root,
                object : SimpleFileVisitor<Path>() {
                    override fun preVisitDirectory(
                        dir: Path,
                        attrs: BasicFileAttributes,
                    ): FileVisitResult {
                        if (dir != root) add(0L)
                        return if (error == null) FileVisitResult.CONTINUE else FileVisitResult.TERMINATE
                    }

                    override fun visitFile(
                        file: Path,
                        attrs: BasicFileAttributes,
                    ): FileVisitResult {
                        if (!attrs.isSymbolicLink) add(attrs.size())
                        return if (error == null) FileVisitResult.CONTINUE else FileVisitResult.TERMINATE
                    }

                    override fun visitFileFailed(file: Path, exc: java.io.IOException): FileVisitResult {
                        if (error == null) {
                            error = "无法读取：${file.fileName ?: file}（${exc.javaClass.simpleName}）"
                        }
                        return FileVisitResult.TERMINATE
                    }
                },
            )
        } catch (t: Throwable) {
            if (error == null) {
                error = "无法读取文件夹：${root.fileName ?: root}（${t.javaClass.simpleName}）"
            }
        }
    }
    return TransferScan(entries, bytes, error)
}

/** 默认落盘目录：Linux `~/Downloads/opi`，Windows `%USERPROFILE%\Downloads\opi`。纯函数可单测。 */
internal fun defaultTransferSaveDir(
    windows: Boolean = isWindows(),
    userHome: String? = System.getProperty("user.home"),
    userProfile: String? = System.getenv("USERPROFILE"),
): Path {
    val home = if (windows) {
        userProfile?.takeIf { it.isNotBlank() } ?: userHome
    } else {
        userHome
    }?.takeIf { it.isNotBlank() } ?: "."
    return Paths.get(home, "Downloads", "opi")
}

// ---------- 文件选择器 / 打开目录（调用方从 Dispatchers.IO 包进来） ----------

internal fun pickTransferFiles(): List<Path> =
    chooseTransferPaths("选择要发送的文件", JFileChooser.FILES_ONLY, multi = true, filter = null)

internal fun pickTransferMedia(): List<Path> = chooseTransferPaths(
    "选择图片/音频/视频",
    JFileChooser.FILES_ONLY,
    multi = true,
    filter = FileNameExtensionFilter("图片 / 音频 / 视频", *TRANSFER_MEDIA_EXTENSIONS),
)

internal fun pickTransferFolder(): Path? =
    chooseTransferPaths("选择要发送的文件夹", JFileChooser.DIRECTORIES_ONLY, multi = false, filter = null)
        .firstOrNull()

internal fun pickTransferSaveDir(): Path? =
    chooseTransferPaths("选择接收文件的保存位置", JFileChooser.DIRECTORIES_ONLY, multi = false, filter = null)
        .firstOrNull()

internal val TRANSFER_MEDIA_EXTENSIONS = arrayOf(
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "heic", "svg",
    "mp3", "wav", "flac", "aac", "ogg", "m4a", "opus", "wma", "ape",
    "mp4", "mkv", "mov", "avi", "webm", "m4v", "wmv", "flv", "ts",
)

private fun chooseTransferPaths(
    title: String,
    mode: Int,
    multi: Boolean,
    filter: FileNameExtensionFilter?,
): List<Path> {
    var picked: Array<File>? = null
    return try {
        SwingUtilities.invokeAndWait {
            val parent = KeyboardFocusManager.getCurrentKeyboardFocusManager().activeWindow
            val fc = JFileChooser()
            fc.dialogTitle = title
            fc.fileSelectionMode = mode
            fc.isMultiSelectionEnabled = multi
            if (filter != null) {
                fc.isAcceptAllFileFilterUsed = false
                fc.fileFilter = filter
            }
            if (fc.showOpenDialog(parent) == JFileChooser.APPROVE_OPTION) {
                picked = fc.selectedFiles
            }
        }
        picked?.map { it.toPath() }.orEmpty()
    } catch (_: Throwable) {
        emptyList() // 取消 / headless / 桌面不支持 Swing 对话框
    }
}

/** 完成后「打开」按钮：系统文件管理器；失败返回 false 由 UI 如实提示。 */
internal fun openTransferFolder(path: String): Boolean = try {
    val dir = File(path)
    if (!dir.isDirectory) {
        false
    } else if (Desktop.isDesktopSupported() &&
        Desktop.getDesktop().isSupported(Desktop.Action.OPEN)
    ) {
        Desktop.getDesktop().open(dir)
        true
    } else {
        false
    }
} catch (_: Throwable) {
    false
}
