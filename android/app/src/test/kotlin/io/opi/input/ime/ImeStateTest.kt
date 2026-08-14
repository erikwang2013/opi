package io.opi.input.ime

import io.opi.input.engine.EngineController
import io.opi.input.engine.FakeEngine
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
