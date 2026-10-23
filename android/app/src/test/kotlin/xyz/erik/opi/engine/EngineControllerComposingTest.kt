// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 组合串（preedit）一族的用例（500 行门禁从 EngineControllerTest.kt 搬出，本仓同惯例）。
// 覆盖的是「显示/上屏的文本」这条出口：9 键不落数字；显示串 = 引擎补全后的读数
// （2026-10-23 起不再在本层补「·」—— 完整显示由引擎 `display_buffer` 负责）。

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
        // 2026-10-13 用户订正（「拼音输入会偶尔出现直接变成两个点」）：整段无匹时
        // **不再补点** —— 字段留空，反馈交给候选栏的「无匹配」（条件 = 无候选且缓冲
        // 非空，见 CandidateBar）。旧期望「·····」是 2026-10-12 那版的落点，已被取代。
        assertEquals("9 键：读数空 ⇒ 字段留空（不落数字，也不补点）", "", ctrl.composingText())
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
    fun composingTextIsTheEngineReadingIncludingConfirmedChars() {
        // 逐字确认（2026-10-17）：已确认的字已前置进读数（displayBuffer 带 pending）。
        // 2026-10-23 起本层不再补「·」（完整显示由引擎 `display_buffer` 补全未覆盖键），
        // 所以显示串就是引擎读数本身 —— 已确认字不多不少。
        val fake = FakeEngine().apply {
            schemeValue = InputScheme.NINE.value
            buf = "66666"
            displayBuf = "中mo'mo"
            pending = "中"
        }
        val ctrl = EngineController(fake)
        assertEquals("中mo'mo", ctrl.composingText())
        fake.displayBuf = "mo'mo"
        fake.pending = ""
        fake.buf = "6666"
        ctrl.refresh()
        assertEquals("mo'mo", ctrl.composingText())
    }

    @Test
    fun composingTextDoesNotAppendPendingDots() {
        // 9 键尾巴（历史形态）：读数只覆盖前缀，旧实现补「·」，看起来像省略号
        // （2026-10-23 用户报障）。现在显示串**原样**是引擎读数；未覆盖的键由引擎
        // 按「键面首字母」补全（见 engine-core 的 nine_display_reading）。
        val fake = FakeEngine().apply {
            schemeValue = InputScheme.NINE.value
            buf = "666666"
            displayBuf = "mo'mo"
        }
        val ctrl = EngineController(fake)
        assertEquals("mo'mo", ctrl.composingText())
        assertFalse("不许出现旧的点号标记", ctrl.composingText().contains('·'))
        // 再按一键：显示串跟着引擎读数走，不在这里追加任何记号
        fake.buf = "6666665"
        fake.displayBuf = "mo'mo"
        ctrl.refresh()
        assertEquals("mo'mo", ctrl.composingText())
    }
}
