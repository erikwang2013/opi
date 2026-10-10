// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.engine.EngineMode
import xyz.erik.opi.engine.InputScheme
import xyz.erik.opi.keyboard.SkinOption
import xyz.erik.opi.engine.FakeEngine
import xyz.erik.opi.engine.ShiftState
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

    @Test
    fun selectInputModeWritesEngineAndHandlesPanels() {
        val fake = FakeEngine()
        val state = newState(fake)
        // 9 键：写 scheme + mode
        state.selectInputMode(InputMode.PINYIN9)
        assertEquals(listOf(1), fake.schemeCalls) // 1 = Nine
        assertEquals(EngineMode.PINYIN, state.controller.mode)
        assertEquals(ImeState.View.QWERTY, state.view)
        // 生僻字：只切面板、**不碰引擎**（scheme/mode 调用数不变）
        state.selectInputMode(InputMode.RARE)
        assertEquals(listOf(1), fake.schemeCalls)
        assertEquals(ImeState.View.RARE, state.view)
        // 回字母盘
        state.backToLetters()
        assertEquals(ImeState.View.QWERTY, state.view)
    }

    @Test
    fun cycleModeKeepsSchemeAndRestoresLastChinese() {
        // 2026-10-09 二期起三条腿全走 switchLanguage（换模式/方案但**保留缓冲**，
        // 用户裁决「切英文，拼音保留可继续使用」）——不再 commit、不再走清缓冲的
        // switchMode/switchScheme。本用例钉「谁变到哪」；「没提交、没清」的判据在
        // cycleModeKeepsTheBufferAndDoesNotCommit。
        val fake = FakeEngine()
        val state = newState(fake)
        state.selectInputMode(InputMode.PINYIN9)
        fake.switchLanguageCalls.clear()
        fake.schemeCalls.clear()
        // 中 → 繁：只切引擎模式（方案不动 —— 9 键仍是 9 键）
        state.cycleMode()
        assertEquals(
            listOf(EngineMode.TRADITIONAL.value to InputScheme.NINE.value),
            fake.switchLanguageCalls,
        )
        assertTrue("中→繁不许走清缓冲的 switchScheme", fake.schemeCalls.isEmpty())
        // 繁 → 英：换英文 + 全键盘方案（9 键上打英文没有候选路）
        state.cycleMode()
        assertEquals(EngineMode.ENGLISH, state.controller.mode)
        assertEquals(
            listOf(
                EngineMode.TRADITIONAL.value to InputScheme.NINE.value,
                EngineMode.ENGLISH.value to InputScheme.FULL.value,
            ),
            fake.switchLanguageCalls,
        )
        // 英 → 中：回**上次的中文方案**（9 键）
        state.cycleMode()
        assertEquals(EngineMode.PINYIN, state.controller.mode)
        assertEquals(InputMode.PINYIN9, state.inputMode)
        assertEquals(
            EngineMode.PINYIN.value to InputScheme.NINE.value,
            fake.switchLanguageCalls.last(),
        )
    }

    // ---- 皮肤（2026-10-09 用户裁决「移动端没有皮肤设置」）----

    @Test
    fun selectSkinSwitchesPanelAndPersists() {
        var persisted = 0
        val state = newState().apply { persist = { persisted++ } }
        state.openSkin()
        assertEquals(ImeState.View.SKIN, state.view)
        state.selectSkin(SkinOption.CINNABAR)
        assertEquals(SkinOption.CINNABAR, state.skin)
        assertEquals("选择即落盘", 1, persisted)
        // 面板不自动关（连换着比才顺手）
        assertEquals(ImeState.View.SKIN, state.view)
        state.backToLetters()
        assertEquals(ImeState.View.QWERTY, state.view)
    }

    @Test
    fun openSkinDoesNotTouchTheEngine() {
        // 皮肤只换观感：不切 scheme/mode、不提交缓冲（与输入模式那类切换的区别）
        val fake = FakeEngine()
        val state = newState(fake)
        state.openSkin()
        assertEquals(emptyList<Int>(), fake.schemeCalls)
        assertEquals(emptyList<Int>(), fake.switchCalls)
    }

    @Test
    fun restoreCarriesTheSkin() {
        val state = newState()
        state.restore(InputMode.PINYIN26, KeyboardModes(), SkinOption.PAPER)
        assertEquals(SkinOption.PAPER, state.skin)
    }

    @Test
    fun cycleModeKeepsTheBufferAndDoesNotCommit() {
        // 2026-10-09 二期用户裁决「中文输入的时候切换英文，拼音保留可继续使用」：
        // 中/繁/英三条腿全部走 switchLanguage（引擎侧保留缓冲），**不再 commitPendingBuffer**
        // （旧行为会把打到一半的拼音上屏），也不走清缓冲的 switchMode/switchScheme。
        val fake = FakeEngine().apply { buf = "ni" }
        val state = newState(fake)
        state.cycleMode() // 中→繁：只换模式（方案不动），缓冲保留
        assertEquals(
            listOf(EngineMode.TRADITIONAL.value to InputScheme.FULL.value),
            fake.switchLanguageCalls,
        )
        state.cycleMode() // 繁→英：换到英文 + 全键盘方案
        assertEquals(
            EngineMode.ENGLISH.value to InputScheme.FULL.value,
            fake.switchLanguageCalls.last(),
        )
        state.cycleMode() // 英→中：回上次的中文方案
        assertEquals(
            EngineMode.PINYIN.value to InputScheme.FULL.value,
            fake.switchLanguageCalls.last(),
        )
        assertTrue("三条腿都不许提交 pending", commits.isEmpty())
        assertTrue("不许走清缓冲的 switchScheme", fake.schemeCalls.isEmpty())
    }

    @Test
    fun modeEntriesOpenPanelsNotMenus() {
        // 2026-10-09 用户裁决「输入模式/键盘模式做成像皮肤那样的面板，不要弹框」：
        // 两个入口切的是**面板视图**（与皮肤同一条 when 分发），且都不碰引擎 ——
        // 真正的选择/开关动作在面板内各自既有路径上做。
        val fake = FakeEngine()
        val state = newState(fake)
        state.openInputMode()
        assertEquals(ImeState.View.INPUT_MODE, state.view)
        state.openKeyboardMode()
        assertEquals(ImeState.View.KEYBOARD_MODE, state.view)
        assertEquals(emptyList<Int>(), fake.schemeCalls)
        assertEquals(emptyList<Int>(), fake.switchCalls)
    }

    // ---- 密码字段临时英文（2026-10-10）----

    @Test
    fun transientEnglishEntersWithoutPersisting() {
        var persisted = 0
        val fake = FakeEngine()
        val state = newState(fake).apply { persist = { persisted++ } }
        state.selectInputMode(InputMode.PINYIN9)
        persisted = 0 // selectInputMode 自己会落盘一次，从这里开始数
        state.enterTransientEnglish()
        assertEquals("临时切英文不许落盘", 0, persisted)
        assertEquals(InputMode.ENGLISH26, state.inputMode)
        assertEquals(
            EngineMode.ENGLISH.value to InputScheme.FULL.value,
            fake.switchLanguageCalls.last(),
        )
    }

    @Test
    fun leaveTransientEnglishRestoresTheChineseMode() {
        var persisted = 0
        val fake = FakeEngine()
        val state = newState(fake).apply { persist = { persisted++ } }
        state.selectInputMode(InputMode.PINYIN9)
        persisted = 0 // selectInputMode 自己会落盘一次，从这里开始数
        state.enterTransientEnglish()
        state.leaveTransientEnglish()
        assertEquals("还原回进密码场之前的中文方案", InputMode.PINYIN9, state.inputMode)
        assertEquals(
            EngineMode.PINYIN.value to InputScheme.NINE.value,
            fake.switchLanguageCalls.last(),
        )
        assertEquals("还原也不落盘", 0, persisted)
    }

    @Test
    fun transientEnglishIsNoOpWhenAlreadyEnglish() {
        val fake = FakeEngine()
        val state = newState(fake)
        // 用户自己先切到英文（cycleMode 的英那条腿）
        state.selectInputMode(InputMode.ENGLISH26)
        val callsBefore = fake.switchLanguageCalls.size
        state.enterTransientEnglish()
        assertEquals("已在英文：不记状态也不切换", callsBefore, fake.switchLanguageCalls.size)
        state.leaveTransientEnglish()
        assertEquals("没进过就不许把英文还原成中文", InputMode.ENGLISH26, state.inputMode)
    }
}
