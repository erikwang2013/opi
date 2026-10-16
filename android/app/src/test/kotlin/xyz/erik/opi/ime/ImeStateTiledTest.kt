// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.engine.EngineMode
import xyz.erik.opi.engine.FakeEngine
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * **平铺候选视图**的状态机（2026-10-16 用户裁决「左右翻页改成替换键盘平铺出来……上下滑动
 * 翻页」）：折叠行右端 ⌄ 进、占满「候选｜工具条」共用格 + 键盘区。返回、选词、退到空
 * 都要**自动收拢**回键盘（否则键盘回不来）。
 *
 * 渲染不在这里测（Compose 无 JVM 测试面）——本文件钉的是视图状态与收拢判据，
 * 界面以模拟器截屏核验（同本仓移动端改动的既有口径）。
 */
class ImeStateTiledTest {
    private val commits = mutableListOf<String>()

    private fun newState(fake: FakeEngine = FakeEngine()) =
        ImeState(EngineController(fake), commit = { commits += it }, debouncer = FakeDebouncer())

    @Test
    fun expandEntersTiledAndCollapseReturnsToKeyboard() {
        val fake = FakeEngine().apply { buf = "wo"; cands = arrayOf("我", "握") }
        val state = newState(fake)
        state.expandCandidates()
        assertEquals(ImeState.View.CANDIDATES_TILED, state.view)
        state.collapseCandidates()
        assertEquals(ImeState.View.QWERTY, state.view)
    }

    @Test
    fun tiledStaysWhileCandidatesStillShowable() {
        val fake = FakeEngine().apply { buf = "wo"; cands = arrayOf("我") }
        val state = newState(fake)
        state.expandCandidates()
        state.collapseIfNoCandidates()
        assertEquals(ImeState.View.CANDIDATES_TILED, state.view)
    }

    @Test
    fun tiledAutoCollapsesAfterCommitEmptiesTheArea() {
        val fake = FakeEngine().apply {
            buf = "wo"
            cands = arrayOf("我")
            selectResults[0] = "我"
        }
        val state = newState(fake)
        state.expandCandidates()
        // 选词：真引擎此刻缓冲清空、候选随即为空（FakeEngine 的 candidates() 直接回
        // `cands`，故先置空再选，等价于提交后那一拍的状态）。平铺视图的选词回调
        // 末尾会调 collapseIfNoCandidates（见 ImeScreen 的接线）。
        fake.cands = emptyArray()
        assertEquals("我", state.controller.select(0))
        state.collapseIfNoCandidates()
        assertEquals(ImeState.View.QWERTY, state.view)
    }

    @Test
    fun tiledAutoCollapsesAfterBackspaceEmptiesBuffer() {
        val fake = FakeEngine().apply { buf = "w"; cands = arrayOf("我") }
        val state = newState(fake)
        state.expandCandidates()
        fake.cands = emptyArray()
        state.controller.backspace() // 退到空：无候选、缓冲空
        state.collapseIfNoCandidates()
        assertEquals(ImeState.View.QWERTY, state.view)
    }

    /** 判据与「候选｜工具条」共用格同一支：英文模式（EN 提示条）也算有东西可显示。 */
    @Test
    fun tiledStaysInEnglishModeEvenWithEmptyBuffer() {
        val fake = FakeEngine().apply { mode = EngineMode.ENGLISH.value }
        val state = newState(fake)
        state.expandCandidates()
        state.collapseIfNoCandidates()
        assertEquals(ImeState.View.CANDIDATES_TILED, state.view)
    }

    /** 换输入目标（onEditorChanged）必须把平铺态收掉（它占着键盘区，不得跨字段残留）。 */
    @Test
    fun editorChangedLeavesTiledView() {
        val fake = FakeEngine().apply { buf = "wo"; cands = arrayOf("我") }
        val state = newState(fake)
        state.expandCandidates()
        state.onEditorChanged()
        assertEquals(ImeState.View.QWERTY, state.view)
    }
}
