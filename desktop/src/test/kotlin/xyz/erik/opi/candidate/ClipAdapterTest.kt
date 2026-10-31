// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 剪贴板适配器的去重/回声/降级门禁（注入读写 lambda，不碰真剪贴板）。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeFalse
import org.junit.Test

class ClipAdapterTest {

    @Test
    fun `poller emits changes once and skips own writes`() {
        var clip = "hello"
        val written = ArrayList<String>()
        val a = AwtClipAdapter(
            mode = ClipMode.POLL,
            readText = { clip },
            writeText = {
                written += it
                clip = it
            },
        )
        assertEquals("hello", a.poll())
        assertNull("相同内容不重复发", a.poll())
        clip = ""
        assertNull("空剪贴板不发", a.poll())
        clip = "next"
        assertEquals("next", a.poll())

        assertTrue(a.write("own"))
        assertEquals(listOf("own"), written)
        assertNull("自己写的不能回声", a.poll())
        clip = "other"
        assertEquals("other", a.poll())
        clip = "own"
        assertEquals("中间出现过别的文本后，外部再复制同名文本不能误吞", "own", a.poll())
    }

    @Test
    fun `manual mode never reads the clipboard`() {
        var reads = 0
        val a = AwtClipAdapter(mode = ClipMode.MANUAL, readText = { reads++; "x" }, writeText = {})
        assertNull(a.poll())
        assertEquals("Wayland 手动模式不得轮询", 0, reads)
    }

    @Test
    fun `read failure is swallowed and retried`() {
        var fail = true
        val a = AwtClipAdapter(
            mode = ClipMode.POLL,
            readText = { if (fail) error("clipboard busy") else "ok" },
            writeText = {},
        )
        assertNull(a.poll())
        fail = false
        assertEquals("ok", a.poll())
    }

    @Test
    fun `write failure returns false`() {
        val a = AwtClipAdapter(
            mode = ClipMode.POLL,
            readText = { null },
            writeText = { error("denied") },
        )
        assertFalse(a.write("secret"))
    }

    @Test
    fun `windows adapter is unavailable off windows`() {
        assumeFalse(isWindows())
        // 工厂必须把 JNA/User32 的加载失败折成 null，不能把异常抛给设置页。
        assertNull(createClipboardAdapter(windows = true, mode = ClipMode.POLL))
    }
}
