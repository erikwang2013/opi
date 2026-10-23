// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 自定义短语页的文件选择器与有界读（从 `CustomPhrasesPage.kt` 拆出：那边加中英
//! 双套后接近 500 行门禁）。
//!
//! 文件选择器沿用词库页的 AWT `JFileChooser` 先例（无新依赖；对话框在 EDT 上 show，
//! 调用方从 IO 线程包进来）。

package xyz.erik.opi.candidate

import java.awt.KeyboardFocusManager
import java.nio.file.Files
import java.nio.file.Path
import javax.swing.JFileChooser
import javax.swing.SwingUtilities
import javax.swing.filechooser.FileNameExtensionFilter

/** 导入文本上限：短语表是手写的，1 MiB 远超正常规模；超限挡在读入之前（同 Android）。 */
private const val MAX_IMPORT_BYTES = 1024 * 1024

/**
 * 弹 AWT 的 JFileChooser 选一个短语文件。Swing 的规矩 = 对话框在 EDT 上 show，
 * 调用方从 IO 线程 `withContext(Dispatchers.IO)` 包进来（与词库页 [pickTsvFile] 同款）。
 * 取消 / headless（测试机）⇒ null。
 */
internal fun pickImportFile(): Path? {
    var picked: java.io.File? = null
    return try {
        SwingUtilities.invokeAndWait {
            val parent = KeyboardFocusManager.getCurrentKeyboardFocusManager().activeWindow
            val fc = JFileChooser()
            fc.dialogTitle = "选择短语文件"
            fc.isAcceptAllFileFilterUsed = true
            fc.fileFilter = FileNameExtensionFilter(
                "短语文件 (*.json, *.tsv, *.csv, *.txt)",
                "json", "tsv", "csv", "txt",
            )
            if (fc.showOpenDialog(parent) == JFileChooser.APPROVE_OPTION) picked = fc.selectedFile
        }
        picked?.toPath()
    } catch (_: Exception) {
        null
    }
}

/**
 * 弹保存对话框。用户没写扩展名时补上（Linux 的 JFileChooser 不自动加）——
 * 补完的路径仍可能被用户改成别的名字，导出语义不受影响。
 */
internal fun pickSaveFile(title: String, defaultName: String, ext: String): Path? {
    var picked: java.io.File? = null
    return try {
        SwingUtilities.invokeAndWait {
            val parent = KeyboardFocusManager.getCurrentKeyboardFocusManager().activeWindow
            val fc = JFileChooser()
            fc.dialogTitle = title
            fc.selectedFile = java.io.File(defaultName)
            fc.fileFilter = FileNameExtensionFilter("短语文件 (*.$ext)", ext)
            if (fc.showSaveDialog(parent) == JFileChooser.APPROVE_OPTION) picked = fc.selectedFile
        }
        picked?.toPath()?.let { p ->
            if (p.fileName.toString().endsWith(".$ext", ignoreCase = true)) {
                p
            } else {
                p.resolveSibling(p.fileName.toString() + ".$ext")
            }
        }
    } catch (_: Exception) {
        null
    }
}

/** 有界读（UTF-8，坏字节替换）：超限抛 IllegalArgumentException，由调用方如实显示。 */
internal fun readImportText(p: Path): String {
    require(Files.size(p) <= MAX_IMPORT_BYTES) { "文件超过 ${MAX_IMPORT_BYTES / 1024} KB" }
    return Files.newBufferedReader(p).use { it.readText() }
}
