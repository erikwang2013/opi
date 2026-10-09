// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 虚拟键盘的**数据与映射**门禁 + 一条真 X11 探针。
//!
//! 注入本身（`XTestFakeKeyEvent` / `SendInput`）没法自动测：那是往用户桌面真的按一下
//! 键。能自动测且值得测的是它**上游**的两层 —— 键表（两个平台的键标识）与
//! 「键 → 事件序列」的纯映射；键表里写错一个键名/键码，两端的表现都是「点了没反应」，
//! 没有任何东西会报错，所以这里逐键钉住，并在本机真问一次 Xlib 的键码解析（探针，
//! 不注入 —— 见下面 `every key resolves` 那条）。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test

class VirtualKeyboardTest {

    private val allKeys = VK_ROWS.flatten() + VK_BOTTOM.map { it.first }

    @Test
    fun `layout is the decided key set`() {
        // 用户裁决的键盘面：qwerty + 数字行 + 空格/回车/退格，本版不加别的键。
        assertEquals("1234567890", VK_ROWS[0].map { it.label }.joinToString(""))
        assertEquals("qwertyuiop", VK_ROWS[1].map { it.label }.joinToString(""))
        assertEquals("asdfghjkl", VK_ROWS[2].map { it.label }.joinToString(""))
        assertEquals("zxcvbnm", VK_ROWS[3].map { it.label }.joinToString(""))
        assertEquals(listOf("空格", "回车", "退格"), VK_BOTTOM.map { it.first.label })
        // 底行格宽（空格 6 / 回车 2 / 退格 2）：与字母行的 10 格对齐
        assertEquals(10, VK_BOTTOM.sumOf { it.second })
    }

    @Test
    fun `special keys pin both platform identifiers`() {
        // 手写的字面量（构造函数推不出来的那部分）：X11 的 keysym 名与 Windows 的 VK 码。
        // 错一个的表现是「这个键点了没反应/出别的字符」，两端都不报错。
        assertEquals("space", VK_SPACE.keysym)
        assertEquals(0x20, VK_SPACE.vk)
        assertEquals("Return", VK_ENTER.keysym)
        assertEquals(0x0D, VK_ENTER.vk)
        assertEquals("BackSpace", VK_BACKSPACE.keysym)
        assertEquals(0x08, VK_BACKSPACE.vk)
    }

    @Test
    fun `letters and digits map to the ascii shapes both platforms use`() {
        for (k in allKeys) {
            if (k.label.length != 1) continue
            val c = k.label[0]
            if (c.isLetter()) {
                // X11：小写 keysym 名（"a"）；Windows：VK 是大写字母码（'A' = 0x41）
                assertEquals(c.toString(), k.keysym)
                assertEquals(c.uppercaseChar().code, k.vk)
            } else {
                assertEquals(c.code, k.vk)
            }
        }
    }

    @Test
    fun `a click injects down then up`() {
        // 顺序是承重的（先抬后按 = 反向的按键），且「序列」这个形状是组合键的扩展点。
        val seq = vkEventSequence(VK_SPACE)
        assertEquals(listOf(true, false), seq.map { it.down })
        assertEquals(listOf(VK_SPACE, VK_SPACE), seq.map { it.key })
        assertEquals(listOf(true, false), vkEventSequence(VK_ROWS[1][0]).map { it.down })
    }

    /**
     * **真 X11 探针**：逐个键问一次 Xlib 能不能解出物理键码（`XStringToKeysym` →
     * `XKeysymToKeycode`，**不注入**，见 KeyInject.x11KeycodeFor）。
     *
     * 这是键表在 Linux 侧唯一的行为级证据：`XStringToKeysym` 认不得的键名会**静默**
     * 返回 NoSymbol（0），只有真问一次才知道表里有没有错字 —— 而错的后果就是
     * 「那个键点了没反应」。没有 X 显示时（CI）**跳过**：跳过是「没测」，不是「通过」。
     */
    @Test
    fun `every key resolves against a real X11 session`() {
        assumeTrue(
            "没有 X 显示（CI/无头），跳过：这条在真机上才算数",
            !isWindows() && !System.getenv("DISPLAY").isNullOrEmpty(),
        )
        for (k in allKeys) {
            assertNotNull(
                "「${k.label}」（keysym=${k.keysym}）在本机 X11 上解不出键码",
                x11KeycodeFor(k),
            )
        }
        assertTrue("表里得有键", allKeys.size >= 30)
    }
}
