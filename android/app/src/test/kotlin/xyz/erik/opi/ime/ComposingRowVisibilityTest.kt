// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 顶部槽位的取舍（任务 2026-10-23）：**输入中（含只剩候选/缓冲的每一拍）必须画拼音行**，
 * 空闲才画宠物条。判据抽成纯函数 [composingRowActive]，这里钉住四条边。
 *
 * 为什么不靠 Compose 测试：本仓单测是纯 JVM（无 Robolectric/instrumentation），
 * 布局断言够不着；这个函数是 ImeScreen 做分支的唯一依据。
 */
class ComposingRowVisibilityTest {

    @Test
    fun idleShowsThePetStrip() {
        assertFalse(
            "全空 = 空闲（画宠物条）",
            composingRowActive(composingText = "", wubiHint = "", buffer = "", candidateCount = 0),
        )
    }

    @Test
    fun candidatesAloneKeepTheComposingRow() {
        // 产品硬要求：候选出现的那一拍拼音行绝不能消失/换成装饰条。
        // 即使引擎这拍没给显示串（9 键无匹读数空），有候选也必须占住槽位。
        assertTrue(
            composingRowActive(composingText = "", wubiHint = "", buffer = "", candidateCount = 3),
        )
    }

    @Test
    fun bufferAloneKeepsTheComposingRow() {
        // 9 键整段无匹：composingText 是空串、候选也可能为空，但用户正在输入 ——
        // 槽位不能切回宠物条。
        assertTrue(
            composingRowActive(composingText = "", wubiHint = "", buffer = "94642", candidateCount = 0),
        )
    }

    @Test
    fun wubiHintAndDisplayTextEachKeepTheComposingRow() {
        assertTrue(composingRowActive("ni'hao", "", "", 0))
        assertTrue(composingRowActive("", "g k", "", 0))
    }

    @Test
    fun emptyDisplayFallsBackToTheRawKeySequence() {
        // 9 键无匹：引擎不给假拼音，行显示原按键序列（次要色），不能空白。
        assertEquals("94642", composingRowText("", "94642"))
        assertTrue(composingRowIsFallback("", "94642"))
        // 有真实读数时永远用读数，不显示键序。
        assertEquals("ni'hao", composingRowText("ni'hao", "64426"))
        assertFalse(composingRowIsFallback("ni'hao", "64426"))
        // 全空仍是空闲，不伪造内容。
        assertEquals("", composingRowText("", ""))
        assertFalse(composingRowIsFallback("", ""))
    }
}
