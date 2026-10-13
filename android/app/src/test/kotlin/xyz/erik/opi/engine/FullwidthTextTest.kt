// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 全角宽档的门禁（用户裁决 2026-10-12「字母、数字也要」+「Android 补入口」）：
//! - `widenAscii` 的映射逐例（与 engine-core 的 Rust 用例同一批期望：段两端点、
//!   字母大小写各留各的、数字、标点、空格与非 ASCII 恒等）；
//! - `EngineController.fullwidthize` 受开关把关（关 ⇒ 恒等）；
//! - **锁同步**：现读 `crates/engine-core/src/punctuation.rs`（语义源）—— 常数与范围
//!   判据还在不在。改一边不改另一边时这条会红。
//!   （测试工作目录 = `android/app`，路径相对模块根，同 SymbolCategoriesTest。）

package xyz.erik.opi.engine

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class FullwidthTextTest {

    @Test
    fun widenAsciiMapsEveryAsciiGraphicAndNothingElse() {
        // 段的两端（0x21 `!` / 0x7E `~`）逐点 —— 手写范围最容易在这两头错一位。
        assertEquals('！', widenAscii('!'))
        assertEquals('～', widenAscii('~'))
        assertEquals('ａ', widenAscii('a'))
        assertEquals('Ａ', widenAscii('A'))
        assertEquals('１', widenAscii('1'))
        assertEquals('．', widenAscii('.'))
        // 边界：空格（0x20）不在段内；非 ASCII/控制符原样。
        assertEquals(' ', widenAscii(' '))
        assertEquals('汉', widenAscii('汉'))
        assertEquals('\n', widenAscii('\n'))
    }

    @Test
    fun controllerFullwidthizeRespectsTheSwitch() {
        val fake = FakeEngine()
        val ctrl = EngineController(fake)
        assertEquals("开关关 ⇒ 恒等", "a1, ", ctrl.fullwidthize("a1, "))
        ctrl.toggleFullwidth()
        assertTrue("切换后状态=开", ctrl.fullwidth)
        assertEquals("字母/数字/标点都宽，空格不动", "ａ１， ", ctrl.fullwidthize("a1, "))
        assertEquals("非 ASCII 恒等", "你好", ctrl.fullwidthize("你好"))
    }

    @Test
    fun toggleReadsBackThroughRefreshAndModeSwitch() {
        val fake = FakeEngine().apply { fullwidthValue = true }
        val ctrl = EngineController(fake)
        assertTrue("refresh 读回引擎真值", ctrl.fullwidth)
        ctrl.toggleFullwidth()
        assertFalse("再切一次回半角", ctrl.fullwidth)
        // 切模式：引擎侧会重置为模式默认 —— 这里由 fake 改值 + refresh 模拟，
        // 钉的是「refresh 必须重读」（漏了它，controller.fullwidth 会停在旧值；
        // 2026-10-13 月相按钮移除后已无 UI 消费者，本档守控制器 ↔ 引擎的镜像）。
        fake.fullwidthValue = true
        ctrl.switchMode(EngineMode.PINYIN)
        assertTrue("切模式后 refresh 重读", ctrl.fullwidth)
    }

    @Test
    fun rustSourceStillCarriesTheSameRule() {
        // 语义源 = engine-core；本侧的 widenAscii 只是镜像（英文/数字的字母数字各端直传、
        // 不进引擎，规则搬不过来 —— 见 EngineController.fullwidthize 头注）。
        // 这条钉「两边还在说同一件事」：函数在、范围判据是 is_ascii_graphic、常数是 0xFEE0。
        val f = File("../../crates/engine-core/src/punctuation.rs")
        assertTrue("语义源不在了（搬家了？）：${f.absolutePath}", f.isFile)
        val src = f.readText()
        assertTrue(
            "宽档的范围判据变了（应为 is_ascii_graphic —— 段 0x21..0x7E）",
            Regex("""fn ascii_fullwidth_wide[\s\S]{0,200}is_ascii_graphic""").containsMatchIn(src),
        )
        assertTrue("宽档的平移常数变了（应为 0xFEE0）", src.contains("0xFEE0"))
    }
}
