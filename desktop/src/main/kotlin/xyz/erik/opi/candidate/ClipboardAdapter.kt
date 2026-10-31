// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 本机剪贴板适配。两条实现：
//! · Windows：`GetClipboardSequenceNumber` 判变 + CF_UNICODETEXT 读写（自定义 JNA 接口 ——
//!   内置 `User32` 没有剪贴板函数）。
//! · Linux/macOS：AWT 轮询（X11 可用）。**Wayland 明确降级**：不轮询，只保留写入；
//!   全局监听交给「设置页手动发送」。
//!
//! 自己写入的内容必须不回声：Windows 靠写后重取序号，AWT 靠 `lastSeen`/`suppressed`。
//! 二者都在内存里保留最近一次文本（只为比对，不落盘、不打日志）。

package xyz.erik.opi.candidate

import com.sun.jna.Library
import com.sun.jna.Native
import com.sun.jna.Pointer
import java.awt.Toolkit
import java.awt.datatransfer.Clipboard
import java.awt.datatransfer.DataFlavor
import java.awt.datatransfer.StringSelection

internal interface ClipboardAdapter {
    val mode: ClipMode

    /** 有新文本 → 返回它；无变化/空/读不到 → null。MANUAL 模式恒为 null。 */
    fun poll(): String?

    /** 写本机剪贴板；false = 失败。 */
    fun write(text: String): Boolean

    /** 告知「这份文本是本程序刚写的」，下一次 poll 不回声。 */
    fun noteWritten(text: String)

    fun close() {}
}

// ---------- AWT（Linux X11 / macOS） ----------

internal class AwtClipAdapter(
    override val mode: ClipMode,
    private val readText: () -> String?,
    private val writeText: (String) -> Unit,
) : ClipboardAdapter {
    private var lastSeen: String? = null
    private var suppressed: String? = null

    override fun poll(): String? {
        if (mode == ClipMode.MANUAL) return null // Wayland：不假装能全局监听
        val text = try {
            readText().orEmpty()
        } catch (_: Exception) {
            return null // 选区所有者的应用没响应等：下一拍再试
        }
        if (text == lastSeen) return null
        lastSeen = text
        if (text.isEmpty()) return null
        if (text == suppressed) {
            suppressed = null
            return null
        }
        suppressed = null // 中间出现过别的文本 ⇒ 自己的回声已经过去，别再误吞后来的同名复制
        return text
    }

    override fun write(text: String): Boolean = try {
        writeText(text)
        noteWritten(text)
        true
    } catch (_: Exception) {
        false
    }

    override fun noteWritten(text: String) {
        suppressed = text
        lastSeen = text
    }
}

/** 真实 AWT 适配器；无桌面/无剪贴板（headless）→ null。 */
internal fun awtClipAdapter(mode: ClipMode): ClipboardAdapter? = try {
    val clipboard: Clipboard = Toolkit.getDefaultToolkit().systemClipboard
    AwtClipAdapter(
        mode = mode,
        readText = {
            try {
                clipboard.getData(DataFlavor.stringFlavor) as? String
            } catch (_: Exception) {
                null
            }
        },
        writeText = { clipboard.setContents(StringSelection(it), null) },
    )
} catch (_: Throwable) {
    null
}

/** Wayland 手动发送用：设置窗（有焦点）读本机文本；读不到 → null。 */
internal fun readLocalClipboardText(): String? = try {
    val clipboard = Toolkit.getDefaultToolkit().systemClipboard
    clipboard.getData(DataFlavor.stringFlavor) as? String
} catch (_: Throwable) {
    null
}

// ---------- Windows（JNA；Win32 BOOL 是 4 字节 int，别照 Rust bool 写 Byte） ----------

internal const val CF_UNICODETEXT = 13

public interface User32Clip : Library {
    fun OpenClipboard(hWndNewOwner: Pointer?): Int
    fun CloseClipboard(): Int
    fun EmptyClipboard(): Int
    fun GetClipboardSequenceNumber(): Int
    fun GetClipboardData(uFormat: Int): Pointer?
    fun SetClipboardData(uFormat: Int, hMem: Pointer?): Pointer?
}

public interface Kernel32Clip : Library {
    fun GlobalAlloc(uFlags: Int, dwBytes: Long): Pointer?
    fun GlobalFree(hMem: Pointer?): Pointer?

    /** 返回 NULL 也可能是合法句柄 0；调用方按「拿不到内存」处理即可（只读路径）。 */
    fun GlobalLock(hMem: Pointer?): Pointer?
    fun GlobalUnlock(hMem: Pointer?): Int
}

private const val GMEM_MOVEABLE = 0x0002

internal class WindowsClipAdapter : ClipboardAdapter {
    override val mode = ClipMode.POLL
    private val user32 = Native.load("user32", User32Clip::class.java)
    private val kernel32 = Native.load("kernel32", Kernel32Clip::class.java)
    private var lastSeq = -1
    private var lastSeen: String? = null
    private var suppressed: String? = null

    private fun open(): Boolean {
        repeat(10) {
            if (user32.OpenClipboard(null) != 0) return true
            Thread.sleep(5) // 别的应用正开着：常见的是剪贴板管理器，稍后重试
        }
        return false
    }

    /** null = 这一拍没读到（不要更新序号，下一拍重试）；"" = 确实没有文本。 */
    private fun read(): String? {
        if (!open()) return null
        return try {
            val handle = user32.GetClipboardData(CF_UNICODETEXT) ?: return ""
            val p = kernel32.GlobalLock(handle) ?: return ""
            try {
                p.getWideString(0) // Windows 的 wchar_t = 16 位
            } finally {
                kernel32.GlobalUnlock(handle)
            }
        } catch (_: Exception) {
            null
        } finally {
            user32.CloseClipboard()
        }
    }

    override fun poll(): String? {
        val seq = user32.GetClipboardSequenceNumber()
        if (seq == lastSeq) return null
        val text = read() ?: return null
        lastSeq = seq
        if (text.isEmpty() || text == lastSeen) return null
        lastSeen = text
        if (text == suppressed) {
            suppressed = null
            return null
        }
        suppressed = null // 中间出现过别的文本 ⇒ 自己的回声已经过去，别再误吞后来的同名复制
        return text
    }

    override fun write(text: String): Boolean {
        val chars = text.toCharArray()
        val bytes = (chars.size.toLong() + 1L) * 2L
        val h = kernel32.GlobalAlloc(GMEM_MOVEABLE, bytes) ?: return false
        var ownedBySystem = false
        try {
            val p = kernel32.GlobalLock(h) ?: return false
            try {
                for (i in chars.indices) p.setShort(i * 2L, chars[i].code.toShort())
                p.setShort(chars.size * 2L, 0)
            } finally {
                kernel32.GlobalUnlock(h)
            }
            if (!open()) return false
            try {
                user32.EmptyClipboard()
                val ok = user32.SetClipboardData(CF_UNICODETEXT, h)
                if (ok != null) {
                    ownedBySystem = true
                    lastSeq = user32.GetClipboardSequenceNumber()
                    noteWritten(text)
                }
            } finally {
                user32.CloseClipboard()
            }
            return ownedBySystem
        } finally {
            if (!ownedBySystem) kernel32.GlobalFree(h) // 成功后内存归系统，不能再 free
        }
    }

    override fun noteWritten(text: String) {
        suppressed = text
        lastSeen = text
    }
}

/**
 * 适配器工厂。null = 当前环境没有可用的剪贴板（无桌面 / 库缺失）。
 * `windows` 是显式参数：测试在 Linux 上也只能走 AWT 分支。
 */
internal fun createClipboardAdapter(
    windows: Boolean,
    mode: ClipMode,
): ClipboardAdapter? = try {
    if (windows) WindowsClipAdapter() else awtClipAdapter(mode)
} catch (_: Throwable) {
    null
}
