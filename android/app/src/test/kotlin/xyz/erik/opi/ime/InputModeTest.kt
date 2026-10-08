// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import xyz.erik.opi.engine.EngineMode
import xyz.erik.opi.engine.InputScheme

/** 输入模式的引擎映射（三根轴：方案 / 模式 / 面板）与布局反推。 */
class InputModeTest {

    @Test
    fun engineAxesArePinned() {
        assertEquals(InputScheme.FULL, InputMode.PINYIN26.scheme)
        assertEquals(EngineMode.PINYIN, InputMode.PINYIN26.engineMode)
        assertEquals(InputScheme.NINE, InputMode.PINYIN9.scheme)
        assertEquals(InputScheme.NINE, InputMode.BIG9.scheme)
        assertTrue(InputMode.BIG9.big)
        assertEquals(InputScheme.FOURTEEN, InputMode.PINYIN14.scheme)
        assertEquals(InputScheme.STROKE, InputMode.STROKE.scheme)
        assertEquals(EngineMode.ENGLISH, InputMode.ENGLISH26.engineMode)
        // 生僻字是面板：不进引擎（scheme/mode 是占位值，选中时 UI 层拦下）
        assertTrue(InputMode.RARE.panel)
    }

    @Test
    fun sevenModesWithSogouStyleLabels() {
        assertEquals(7, InputMode.entries.size)
        assertEquals(
            listOf("拼音26键", "英文26键", "拼音9键", "拼音14键", "拼音大9键", "笔画键盘", "生僻字键盘"),
            InputMode.entries.map { it.label },
        )
    }

    @Test
    fun layoutForDerivesFromEngineState() {
        // 英文模式恒 26 键（9 键上打英文没有候选路）
        assertEquals(
            InputMode.ENGLISH26,
            layoutFor(InputScheme.NINE, EngineMode.ENGLISH, InputMode.PINYIN9),
        )
        // 9 键方案下保留「大 9」记忆
        assertEquals(InputMode.BIG9, layoutFor(InputScheme.NINE, EngineMode.PINYIN, InputMode.BIG9))
        assertEquals(InputMode.PINYIN9, layoutFor(InputScheme.NINE, EngineMode.PINYIN, InputMode.PINYIN26))
        assertEquals(InputMode.PINYIN14, layoutFor(InputScheme.FOURTEEN, EngineMode.PINYIN, InputMode.PINYIN26))
        assertEquals(InputMode.STROKE, layoutFor(InputScheme.STROKE, EngineMode.PINYIN, InputMode.PINYIN26))
        assertEquals(InputMode.PINYIN26, layoutFor(InputScheme.FULL, EngineMode.TRADITIONAL, InputMode.PINYIN26))
    }
}
