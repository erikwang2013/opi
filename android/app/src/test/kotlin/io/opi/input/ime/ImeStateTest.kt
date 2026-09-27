// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package io.opi.input.ime

import io.opi.input.engine.EngineController
import io.opi.input.engine.EngineMode
import io.opi.input.engine.FakeEngine
import io.opi.input.engine.ShiftState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** 假防抖：手动推进 250ms，无 Thread.sleep（JVM 确定性）。 */
class FakeDebouncer : Debouncer {
    var delayMs = -1L
    private var pending: (() -> Unit)? = null

    override fun schedule(delayMs: Long, action: () -> Unit): () -> Unit {
        this.delayMs = delayMs
        pending = action
        return { if (pending === action) pending = null }
    }

    /** 模拟 250ms 到期。 */
    fun fire() {
        pending?.invoke()
        pending = null
    }

    fun isPending(): Boolean = pending != null
}

class ImeStateTest {
    private val commits = mutableListOf<String>()

    private fun newState(
        fake: FakeEngine = FakeEngine(),
        debounce: FakeDebouncer = FakeDebouncer(),
    ) = ImeState(EngineController(fake), commit = { commits += it }, debouncer = debounce)

    // ---- pending buffer 提交（开面板前：有候选选第一个，无候选清掉） ----

    @Test
    fun openNumberWithCandidatesCommitsFirstAndSwitches() {
        val fake = FakeEngine().apply {
            buf = "wo"
            cands = arrayOf("我", "握")
            selectResults[0] = "我"
        }
        val state = newState(fake)
        state.openNumber()
        assertEquals(ImeState.View.NUMBER, state.view)
        assertEquals(listOf("我"), commits)
        assertEquals("", fake.buf) // 选中即提交，buffer 清空
    }

    @Test
    fun openNumberWithoutCandidatesClearsJunkBuffer() {
        val fake = FakeEngine().apply { buf = "abc"; cands = emptyArray() }
        val state = newState(fake)
        state.openNumber()
        assertEquals(ImeState.View.NUMBER, state.view)
        assertTrue(commits.isEmpty())
        assertEquals(1, fake.clearCalls)
        assertEquals("", fake.buf)
    }

    @Test
    fun openNumberWithEmptyBufferIsNoop() {
        val fake = FakeEngine()
        val state = newState(fake)
        state.openNumber()
        assertEquals(ImeState.View.NUMBER, state.view)
        assertEquals(0, fake.clearCalls)
        assertTrue(commits.isEmpty())
    }

    @Test
    fun openSymbolCommitsPendingSameAsNumber() {
        val fake = FakeEngine().apply {
            buf = "wo"
            cands = arrayOf("我")
            selectResults[0] = "我"
        }
        val state = newState(fake)
        state.openSymbol()
        assertEquals(ImeState.View.SYMBOL, state.view)
        assertEquals(listOf("我"), commits)
    }

    @Test
    fun emptySelectResultIsNotCommitted() {
        // 有候选但 select 返回空串（引擎异常）：不提交也不留 buffer
        val fake = FakeEngine().apply { buf = "wo"; cands = arrayOf("我") }
        val state = newState(fake)
        state.openNumber()
        assertTrue(commits.isEmpty())
        assertEquals("", fake.buf) // 引擎未清 buffer 时 state 也要清掉，不留残留
    }

    // ---- 250ms 搜索防抖 ----

    @Test
    fun searchQueryFiresAfterDebounceWindow() {
        val debounce = FakeDebouncer()
        val state = newState(debounce = debounce)
        state.updateSearchText("ji")
        assertEquals("", state.searchQuery) // 防抖期内不出结果
        assertEquals(ImeState.SEARCH_DEBOUNCE_MS, debounce.delayMs)
        debounce.fire()
        assertEquals("ji", state.searchQuery)
    }

    @Test
    fun rapidTypingOnlyLatestQuerySurvives() {
        val debounce = FakeDebouncer()
        val state = newState(debounce = debounce)
        state.updateSearchText("a")
        state.updateSearchText("ab")
        state.updateSearchText("abc")
        debounce.fire()
        assertEquals("abc", state.searchQuery)
    }

    @Test
    fun queryTrimmedWhenDebounceFires() {
        val debounce = FakeDebouncer()
        val state = newState(debounce = debounce)
        state.updateSearchText("  a ")
        debounce.fire()
        assertEquals("a", state.searchQuery)
    }

    // ---- editorChanged 全量重置 ----

    @Test
    fun editorChangedClearsAllState() {
        val debounce = FakeDebouncer()
        val state = newState(debounce = debounce)
        state.openSymbol()
        state.onSearchFocus(true)
        state.updateSearchText("ji")
        debounce.fire()
        state.onEditorChanged()
        assertEquals(ImeState.View.QWERTY, state.view)
        assertFalse(state.searchActive)
        assertEquals("", state.searchText)
        assertEquals("", state.searchQuery)
        assertFalse(debounce.isPending())
    }

    @Test
    fun editorChangedClearsLockedShift() {
        // 锁定的 ⇧ 不得跨输入目标残留：换输入框/换 app 后再开键盘仍全大写
        val fake = FakeEngine().apply { mode = EngineMode.ENGLISH.value }
        val state = newState(fake)
        state.controller.shiftLongPress()
        assertEquals(ShiftState.LOCK, state.controller.shiftState)

        state.onEditorChanged()

        assertEquals(ShiftState.OFF, state.controller.shiftState)
        assertEquals(listOf(true, false), fake.shiftCalls)
    }

    @Test
    fun editorChangedCancelsPendingDebounce() {
        val debounce = FakeDebouncer()
        val state = newState(debounce = debounce)
        state.updateSearchText("a")
        assertTrue(debounce.isPending())
        state.onEditorChanged()
        assertFalse(debounce.isPending())
        debounce.fire() // 已取消，fire 不应生效
        assertEquals("", state.searchQuery)
    }

    // ---- 数字面板：引擎切到 Number，离开恢复 ----
    // 面板的键是引擎键码（数字进缓冲、`,` `.` 由引擎标点表出文本），引擎不在 Number
    // 模式就没有键有归宿：拼音模式收不了数字，`1` 会变成死键、`,` 还会被映射成 `，`。

    @Test
    fun openNumberSwitchesEngineToNumberAndBackRestoresLetterMode() {
        val fake = FakeEngine().apply { mode = EngineMode.TRADITIONAL.value }
        val state = newState(fake)
        state.openNumber()
        assertEquals(EngineMode.NUMBER.value, fake.mode)
        state.backToLetters()
        assertEquals(EngineMode.TRADITIONAL.value, fake.mode)
    }

    @Test
    fun symbolPanelRoundTripThroughNumberKeepsLetterMode() {
        // 数字 → 符号 → 数字 → 字母：记录不能被 NUMBER 自己覆盖掉
        val fake = FakeEngine().apply { mode = EngineMode.ENGLISH.value }
        val state = newState(fake)
        state.openNumber()
        state.openSymbol()
        state.openNumber()
        state.backToLetters()
        assertEquals(EngineMode.ENGLISH.value, fake.mode)
    }

    @Test
    fun leavingNumberPadCommitsPendingDigitsInsteadOfDroppingThem() {
        // 数字缓冲就是要上屏的文本：它没有候选，落进「无候选就清掉」那支等于吃掉用户输入
        val fake = FakeEngine().apply {
            mode = EngineMode.NUMBER.value
            buf = "12"
            cands = emptyArray()
            spaceResult = "12"
        }
        val state = newState(fake)
        state.backToLetters()
        assertEquals(listOf("12"), commits)
        assertEquals(0, fake.clearCalls)
    }

    @Test
    fun switchingFromNumberPadToSymbolPanelFlushesDigits() {
        val fake = FakeEngine().apply {
            mode = EngineMode.NUMBER.value
            buf = "12"
            cands = emptyArray()
            spaceResult = "12"
        }
        val state = newState(fake)
        state.openSymbol()
        assertEquals(listOf("12"), commits)
        assertEquals(0, fake.clearCalls)
    }

    @Test
    fun leavingPadInLetterModeDoesNotSwitchModes() {
        val fake = FakeEngine()
        val state = newState(fake)
        state.backToLetters()
        state.onEditorChanged()
        assertTrue("字母流程不得被面板的模式恢复碰到", fake.switchCalls.isEmpty())
    }

    // ---- 面板往返 ----

    @Test
    fun backToLettersClearsSearchAndReturnsToQwerty() {
        val debounce = FakeDebouncer()
        val state = newState(debounce = debounce)
        state.openSymbol()
        state.onSearchFocus(true)
        state.updateSearchText("ji")
        debounce.fire()
        state.backToLetters()
        assertEquals(ImeState.View.QWERTY, state.view)
        assertFalse(state.searchActive)
        assertEquals("", state.searchText)
        assertEquals("", state.searchQuery)
    }

    // ---- 离开符号面板必须重置搜索态 ----
    // 面板的 TextField 随 view 变化被移出组合，onFocusChanged(false) 不会回调；不主动清，
    // searchActive 会卡在 true：切回来时搜索框带旧文本、结果网格直接进搜索态、叠盘自动
    // 展开，但那个框其实没有焦点，且期间挂起的防抖仍会写入 searchQuery。

    @Test
    fun openNumberResetsSearchStateAndPendingDebounce() {
        val debounce = FakeDebouncer()
        val s = newState(debounce = debounce)
        s.openSymbol()
        s.onSearchFocus(true)
        s.updateSearchText("heart")
        assertTrue("前置条件：防抖应处于挂起", debounce.isPending())

        s.openNumber()

        assertFalse("离开面板后 searchActive 必须复位", s.searchActive)
        assertEquals("", s.searchText)
        assertEquals("", s.searchQuery)
        assertFalse("挂起的防抖必须撤销", debounce.isPending())
        assertEquals(ImeState.View.NUMBER, s.view)
    }

    @Test
    fun openSymbolResetsSearchStateAfterRoundTrip() {
        val s = newState()
        s.openSymbol()
        s.onSearchFocus(true)
        s.updateSearchText("star")
        s.openNumber()
        s.openSymbol()
        assertFalse("绕一圈回到符号面板不应残留搜索态", s.searchActive)
        assertEquals("", s.searchText)
        assertEquals("", s.searchQuery)
    }

    @Test
    fun closeSearchHidesOverlayButKeepsText() {
        val state = newState()
        state.onSearchFocus(true)
        state.updateSearchText("ji")
        state.closeSearch()
        assertFalse(state.searchActive)
        assertEquals("ji", state.searchText)
    }

    // ---- 搜索盘键位路由 ----

    @Test
    fun searchKeysAppendAndBackspaceDropsLast() {
        val state = newState()
        state.searchKey("w")
        state.searchKey("o")
        assertEquals("wo", state.searchText)
        state.searchSpace()
        assertEquals("wo ", state.searchText)
        state.searchBackspace()
        assertEquals("wo", state.searchText)
    }

    @Test
    fun searchBackspaceRemovesFullCodePoint() {
        val state = newState()
        state.searchKey("😄")
        state.searchBackspace()
        assertEquals("", state.searchText) // 代理对不拆半
    }

    @Test
    fun searchBackspaceOnEmptyIsNoop() {
        val state = newState()
        state.searchBackspace()
        assertEquals("", state.searchText)
    }

    @Test
    fun searchFocusTogglesOverlayState() {
        val state = newState()
        assertFalse(state.searchActive)
        state.onSearchFocus(true)
        assertTrue(state.searchActive)
        state.onSearchFocus(false)
        assertFalse(state.searchActive)
    }
}
