// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.clip

import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager

/**
 * 回环抑制（纯 JVM 可测）：远端文本写进系统剪贴板后，系统会再触发一次
 * `OnPrimaryClipChangedListener` —— 那一次不能再发布回去。
 *
 * 只抑制「仍然是上次远端写入的那段文本」：连续重复的同内容回调都抑制（部分 ROM 会
 * 连发多次），本地换成别的内容即解除。Rust 内核还有一层「最近应用过的内容哈希」
 * 抑制，两层互不依赖。
 */
internal class ClipEchoGuard {
    private var applied: String? = null

    fun rememberRemote(text: String) {
        applied = text
    }

    fun shouldSuppress(text: String): Boolean {
        val last = applied ?: return false
        if (last == text) return true
        applied = null
        return false
    }

    fun reset() {
        applied = null
    }
}

/**
 * 系统剪贴板 → 内核 / 内核 → 系统剪贴板。**只处理纯文本**：
 * 图片、文件、URI 等 `item.text` 为 null 的剪贴板内容直接忽略；MIME 声明里没有
 * `text/plain` 的也忽略。
 *
 * 读剪贴板在 Android 10+ 只允许前台应用或默认输入法 —— 本类由 [ClipRuntime] 在
 * IME 进程里注册，因此输入法是默认输入法时能持续读到本地复制。后台写（远端 → 本地）
 * 不受此限制。读失败（SecurityException）静默跳过，绝不让监听器崩掉进程。
 */
internal class ClipboardAdapter(
    private val clipboard: ClipboardManager,
    private val onLocalText: (String) -> Unit,
) {
    private val echo = ClipEchoGuard()
    private val listener = ClipboardManager.OnPrimaryClipChangedListener { onChanged() }
    private var started = false

    fun start() {
        if (started) return
        started = true
        clipboard.addPrimaryClipChangedListener(listener)
    }

    fun stop() {
        if (!started) return
        started = false
        clipboard.removePrimaryClipChangedListener(listener)
        echo.reset()
    }

    /** 只写剪贴板，绝不自动粘贴到输入框（调用方也不许）。 */
    fun applyRemote(text: String) {
        echo.rememberRemote(text)
        clipboard.setPrimaryClip(ClipData.newPlainText(LABEL, text))
    }

    private fun onChanged() {
        val clip = try {
            clipboard.primaryClip
        } catch (_: SecurityException) {
            return // 后台读剪贴板被系统拒绝：不是错误，下一次在前台/IME 再读
        } ?: return
        if (clip.itemCount <= 0) return
        if (!declaresPlainText(clip.description)) return
        val text = clip.getItemAt(0)?.text?.toString() ?: return
        // shortcut: 只按字符数挡超大剪贴板（内核按 UTF-8 字节限 256 KiB），
        // 真需要精确字节数再在这里转码。
        if (text.length > MAX_TEXT_CHARS) return
        if (echo.shouldSuppress(text)) return
        onLocalText(text)
    }

    private fun declaresPlainText(description: ClipDescription?): Boolean {
        if (description == null || description.mimeTypeCount == 0) return true
        for (i in 0 until description.mimeTypeCount) {
            if (description.getMimeType(i) == ClipDescription.MIMETYPE_TEXT_PLAIN) return true
        }
        return false
    }

    private companion object {
        const val LABEL = "opi-clip"
        const val MAX_TEXT_CHARS = 256 * 1024
    }
}
