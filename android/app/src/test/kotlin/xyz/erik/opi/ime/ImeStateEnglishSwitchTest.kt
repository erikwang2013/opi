// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.engine.EngineMode
import xyz.erik.opi.engine.FakeEngine
import xyz.erik.opi.engine.InputScheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * ImeState 的「英文向」切换语义（从 ImeStateTest 拆出，2026-10-10，500 行门禁——
 * 纯搬移，断言一字未动）：
 * - 密码字段**临时英文**（进密码场自动切、离开还原、不落盘）；
 * - **切英文 = 拼音上屏**（用户裁决「默认是拼音，不要是候选词」；三处英文入口统一）。
 */
class ImeStateEnglishSwitchTest {

    private val commits = mutableListOf<String>()

    // ⚠️ FakeDebouncer 必须显式传：默认的 HandlerDebouncer 构造时碰 android.os.Looper，
    // JVM 单测里必炸（原 ImeStateTest 一直这么传，拆文件时别把这条丢了）。
    private fun newState(fake: FakeEngine = FakeEngine()) =
        ImeState(EngineController(fake), commit = { commits += it }, debouncer = FakeDebouncer())

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

    // ---- 切英文 = 拼音上屏（2026-10-10 用户裁决「默认是拼音，不要是候选词」）----

    @Test
    fun switchToEnglishKeyboardCommitsPinyinNotCandidate() {
        val fake = FakeEngine().apply {
            buf = "64426" // 9 键数字缓冲
            displayBuf = "ni'hao" // 读数 = 分词拼音
            cands = arrayOf("你好", "你敢")
            selectResults[0] = "你好"
        }
        val state = newState(fake)
        state.switchToEnglishKeyboard()
        assertEquals("上屏的是拼音（读数），不是首候选", listOf("ni'hao"), commits)
        assertEquals("缓冲清空", "", fake.buf)
        assertEquals(InputMode.ENGLISH26, state.inputMode)
        // selectInputMode 的路径是 switchScheme + switchMode（不是 switchLanguage）
        assertEquals(InputScheme.FULL.value, fake.schemeCalls.last())
        assertEquals(EngineMode.ENGLISH.value, fake.switchCalls.last())
    }

    @Test
    fun cycleToEnglishAlsoCommitsPinyin() {
        val fake = FakeEngine().apply {
            mode = EngineMode.TRADITIONAL.value
            buf = "nihao"
            displayBuf = "nihao"
            cands = arrayOf("你好")
            selectResults[0] = "你好"
        }
        val state = newState(fake)
        state.cycleMode() // 繁 → 英 那条腿
        assertEquals("不许把候选词上屏", listOf("nihao"), commits)
        assertEquals("", fake.buf)
        assertEquals(InputMode.ENGLISH26, state.inputMode)
    }

    @Test
    fun switchToEnglishKeyboardWithEmptyBufferCommitsNothing() {
        val state = newState(FakeEngine())
        state.switchToEnglishKeyboard()
        assertTrue("空缓冲切英文不许凭空提交", commits.isEmpty())
        assertEquals(InputMode.ENGLISH26, state.inputMode)
    }

    @Test
    fun switchToEnglishWithUnmatchedNineKeyDigitsCommitsNothing() {
        // 9 键无匹配（读数空）切英文：不回退数字（2026-10-11 修「9 键没匹配的时候会
        // 变成数字输入」——本入口与 preedit/空格同类：缓冲是按键编码不是文本）。
        val fake = FakeEngine().apply {
            buf = "94642"
            displayBuf = "" // 无匹配 ⇒ 读数空
            schemeValue = InputScheme.NINE.value
        }
        val state = newState(fake)
        state.switchToEnglishKeyboard()
        assertTrue("9 键无匹配切英文不许把数字上屏", commits.isEmpty())
        assertEquals("缓冲照清（离开中文即弃）", "", fake.buf)
        assertEquals(InputMode.ENGLISH26, state.inputMode)
    }

    @Test
    fun inputModePanelEnglishAlsoCommitsPinyin() {
        // 2026-10-10 深夜裁决统一三处英文入口（底行[英文]/中繁→英循环/输入模式面板的
        // 英文26键）：「默认是拼音，不要是候选词」——面板那条此前会先上首候选。
        val fake = FakeEngine().apply {
            buf = "64426"
            displayBuf = "ni'hao"
            cands = arrayOf("你好", "你敢")
            selectResults[0] = "你好"
        }
        val state = newState(fake)
        state.selectInputMode(InputMode.ENGLISH26)
        assertEquals("面板切英文也上拼音，不许首候选", listOf("ni'hao"), commits)
        assertEquals("", fake.buf)
        assertEquals(InputMode.ENGLISH26, state.inputMode)
    }

    @Test
    fun inputModePanelOtherModesStillCommitFirstCandidate() {
        // 对照：非英文的模式切换保持面板旧语义（有候选选首候选）——裁决只点名英文。
        val fake = FakeEngine().apply {
            buf = "64"
            cands = arrayOf("你", "尼")
            selectResults[0] = "你"
        }
        val state = newState(fake)
        state.selectInputMode(InputMode.STROKE)
        assertEquals(listOf("你"), commits)
    }
}
