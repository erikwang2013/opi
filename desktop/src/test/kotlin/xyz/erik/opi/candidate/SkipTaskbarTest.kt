// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「不进任务栏」的文字面门禁（无 X、无 JNA）：EWMH 原子名字面量 + `_NET_WM_STATE` 的
//! 读-改-写合并 + 认窗判据。三者写错都不报任何错 —— 表现分别是「属性静默没贴上」
//! 「条不再置顶」「把同进程另一条窗认成目标」，全是肉眼才能发现的错。
//!
//! **属性真的贴没贴上**不在这里测：那要活的 X（本机 xprop 手验，见 SkipTaskbar.kt 头
//! 注释的取证状态）；CI 没有 X，钉不住的东西不装钉。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SkipTaskbarTest {

    @Test
    fun `atom names are the frozen EWMH literals`() {
        // 线上契约字面量（EWMH 规范里的名字）：拼错一个字符 = InternAtom 造出一个没人
        // 认识的原子，属性照写、WM 无视、什么都不报。跨轨常量，钉死。
        assertEquals("_NET_WM_WINDOW_TYPE", ATOM_WM_WINDOW_TYPE)
        assertEquals("_NET_WM_WINDOW_TYPE_UTILITY", ATOM_WM_WINDOW_TYPE_UTILITY)
        assertEquals("_NET_WM_STATE", ATOM_WM_STATE)
        assertEquals("_NET_WM_STATE_SKIP_TASKBAR", ATOM_WM_STATE_SKIP_TASKBAR)
        assertEquals("_NET_WM_STATE_SKIP_PAGER", ATOM_WM_STATE_SKIP_PAGER)
        assertEquals("_NET_WM_PID", ATOM_WM_PID)
        assertEquals("_NET_WM_NAME", ATOM_WM_NAME)
    }

    @Test
    fun `state merge only appends and never drops what the wm wrote`() {
        // AWT 每次 map 都写 [ABOVE, STAYS_ON_TOP]；我们只往后加两个 SKIP —— 若改成整条
        // 重写，表现是「条不再置顶」（而且不报错，只有肉眼看得出）。
        assertEquals(listOf(11L, 22L, 33L, 44L), appendMissing(listOf(11L, 22L), listOf(33L, 44L)))
        // 已有一项就不重复加
        assertEquals(listOf(11L, 33L, 44L), appendMissing(listOf(11L, 33L), listOf(33L, 44L)))
        // 两项都在 ⇒ 原样（幂等的判据：want == state 时一个字节都不写）
        val already = listOf(11L, 33L, 44L)
        assertEquals(already, appendMissing(already, listOf(33L, 44L)))
        // 空表（新窗还没写过 _NET_WM_STATE）⇒ 就两项
        assertEquals(listOf(33L, 44L), appendMissing(emptyList(), listOf(33L, 44L)))
        // 0 = InternAtom 没拿到（属性名不存在等）—— 绝不能塞进属性
        assertEquals(listOf(11L), appendMissing(listOf(11L), listOf(0L, 0L)))
    }

    @Test
    fun `window matching requires the title and our pid when present`() {
        assertTrue(windowMatches("opi输入法 状态栏", 4242L, 4242L, "opi输入法 状态栏"))
        // pid 属性还没写上 ⇒ 只认标题（找不到的代价是「条目还在」，不是错杀）
        assertTrue(windowMatches("opi输入法 状态栏", null, 4242L, "opi输入法 状态栏"))
        // 别的进程的同名窗不能认（认错 = 把别人窗口从任务栏里摘了）
        assertFalse(windowMatches("opi输入法 状态栏", 111L, 4242L, "opi输入法 状态栏"))
        // 同进程的另一条窗（虚拟键盘 vs 状态栏）不能认错
        assertFalse(windowMatches("opi输入法 虚拟键盘", 4242L, 4242L, "opi输入法 状态栏"))
        assertFalse(windowMatches(null, 4242L, 4242L, "opi输入法 状态栏"))
    }
}
