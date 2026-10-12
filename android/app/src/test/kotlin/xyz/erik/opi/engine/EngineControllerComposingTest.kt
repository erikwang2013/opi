// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 组合串（preedit）一族的用例（500 行门禁从 EngineControllerTest.kt 搬出，本仓同惯例）。
// 覆盖的是「推给字段的文本」这条出口：9 键不落数字、待编键补「·」、上屏文本不含「·」。

package xyz.erik.opi.engine

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

class EngineControllerComposingTest {

    @Test
    fun composingTextNeverLeaksNineKeyDigits() {
        // preedit 取值（2026-10-11 修「9 键没匹配的时候会变成数字输入」）：读数优先；
        // 读数空时 26 键回退缓冲（旧行为不动）、9 键**不落数字** —— 待编键改用「·」
        // 标记（2026-10-12 晚：无匹时字段一字不动，用户判为「键盘死了」）。
        val fake = FakeEngine().apply { buf = "94642"; displayBuf = "" }
        val ctrl = EngineController(fake)
        assertEquals("26 键：读数空回退缓冲（旧行为）", "94642", ctrl.composingText())
        fake.schemeValue = InputScheme.NINE.value
        ctrl.refresh()
        assertEquals(
            "9 键：读数空不落数字，但每个待编键要有个记号（5 键 = 5 个 ·）",
            "·····",
            ctrl.composingText(),
        )
        assertFalse("· 标记里不许出现数字", ctrl.composingText().any { it.isDigit() })
        fake.displayBuf = "xiang'gang"
        ctrl.refresh()
        assertEquals(
            "有读数时以读数为准；读数覆盖 9 个字母键（' 不占键位）够盖住 5 键缓冲 ⇒ 无补记",
            "xiang'gang",
            ctrl.composingText(),
        )
        // 上屏文本与显示文本同源但不含「·」（见 pendingCommitText）
        assertEquals("xiang'gang", ctrl.pendingCommitText())
    }

    @Test
    fun composingTextMarksKeysNotCoveredByTheReading() {
        // 9 键点候选确认后残留的尾巴（用户报障的核心形态）：读数只覆盖「最长可匹配
        // 前缀」，没被覆盖的键以前完全不显示 ⇒ 再按新键时读数与候选一字不变。
        // 实测：连点 6 次 6 → 点候选确认 → 缓冲剩 `66`（读数 mo'mo 覆盖 4 键）⇒ `mo'mo··`。
        val fake = FakeEngine().apply {
            schemeValue = InputScheme.NINE.value
            buf = "666666"
            displayBuf = "mo'mo"
        }
        val ctrl = EngineController(fake)
        assertEquals("mo'mo··", ctrl.composingText())
        // 再按一键（缓冲 7 位）：标记跟着长一位 —— 「键被收到了」看得见
        fake.buf = "6666665"
        fake.displayBuf = "mo'mo"
        ctrl.refresh()
        assertEquals("mo'mo···", ctrl.composingText())
        // 读数覆盖满（无待编键）⇒ 不加标记
        fake.buf = "6666"
        ctrl.refresh()
        assertEquals("mo'mo", ctrl.composingText())
    }
}
