// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

//! 模式键（中/繁/英）与语言切换的用例（2026-10-15 自 ImeStateTest.kt 拆出：加
//! 「9 键只切简/繁」用例后源文件 517 行越了 500 行门禁）。**同包**：FakeEngine 与
//! FakeDebouncer（后者是 ImeStateTest.kt 里的顶层声明）照常可见，无 import 要改。

import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.engine.EngineMode
import xyz.erik.opi.engine.FakeEngine
import xyz.erik.opi.engine.InputScheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ImeStateModeKeyTest {
    private val commits = mutableListOf<String>()

    private fun newState(
        fake: FakeEngine = FakeEngine(),
        debounce: FakeDebouncer = FakeDebouncer(),
    ) = ImeState(EngineController(fake), commit = { commits += it }, debouncer = debounce)

    @Test
    fun cycleModeKeepsSchemeAndRestoresLastChinese() {
        // 中→繁、繁→英走 switchLanguage（换模式/方案但**保留缓冲**，用户裁决
        // 「切英文，拼音保留可继续使用」）——不走清缓冲的 switchMode/switchScheme。
        // 英→中反而是**输入重置**（2026-10-19：先 clear 再切，不提交；见
        // ImeStateEnglishSwitchTest）。本用例钉「谁变到哪」；「没提交」的判据在
        // cycleModeToTraditionalKeepsBufferAndToEnglishCommitsPinyin。
        // ⚠️ 本条钉的是**26/14/笔画盘**那条循环（2026-10-15 起 9 键不再走它，
        // 见本文件末条 nineKeyModeKeyOnlyTogglesTraditional）。
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

    @Test
    fun cycleModeToTraditionalKeepsBufferAndToEnglishCommitsPinyin() {
        // 语义时间线（两次用户裁决）：
        // · 2026-10-09：「中文输入的时候切换英文，拼音保留可继续使用」——中/繁两腿保留缓冲；
        // · 2026-10-10：「拼音输入的时候切换英文，就默认是拼音，不要是候选词」——
        //   **英向那条腿改成一进英文就把 pending 按拼音（读数/原字母）上屏、缓冲清空**
        //   （不是首候选；「保留缓冲」下字段里看不见、切回才复活，用户判为异常）。
        val fake = FakeEngine().apply {
            buf = "ni"
            displayBuf = "ni"
            cands = arrayOf("你")
            // 全新默认态（2026-10-11 起默认 9 键）：引擎方案 = NINE。
            // 真机上由 restore() 推入；本用例不走 restore，直接置成该态。
            schemeValue = InputScheme.NINE.value
        }
        val state = newState(fake)
        state.cycleMode() // 中→繁：只换模式（方案不动），缓冲保留
        assertEquals(
            listOf(EngineMode.TRADITIONAL.value to InputScheme.NINE.value),
            fake.switchLanguageCalls,
        )
        assertEquals("中→繁不提交", emptyList<String>(), commits)
        state.cycleMode() // 繁→英：**拼音上屏**（不是候选「你」）+ 换全键盘方案
        assertEquals(listOf("ni"), commits)
        assertEquals("缓冲已清", "", fake.buf)
        assertEquals(
            EngineMode.ENGLISH.value to InputScheme.FULL.value,
            fake.switchLanguageCalls.last(),
        )
        state.cycleMode() // 英→中：回上次的中文方案（全新默认态即 9 键；缓冲已清）
        assertEquals(
            EngineMode.PINYIN.value to InputScheme.NINE.value,
            fake.switchLanguageCalls.last(),
        )
        assertTrue("不许走清缓冲的 switchScheme", fake.schemeCalls.isEmpty())
    }

    @Test
    fun nineKeyModeKeyOnlyTogglesTraditional() {
        // 2026-10-15 用户裁决「简繁按钮就只切换简繁」：9 键底行模式键只切简/繁。
        // 断言三条：①走 switchLanguage（与 cycleMode 的中→繁腿同一条：换模式但
        // 方案不动、缓冲保留）；②**不再循环进英文**（第二次点击回简，不是英文）；
        // ③不提交、不走清缓冲的 switchScheme。
        val fake = FakeEngine().apply {
            buf = "ni"
            displayBuf = "ni"
            schemeValue = InputScheme.NINE.value
        }
        val state = newState(fake)
        state.toggleTraditional() // 简 → 繁
        assertEquals(
            listOf(EngineMode.TRADITIONAL.value to InputScheme.NINE.value),
            fake.switchLanguageCalls,
        )
        assertEquals("不提交", emptyList<String>(), commits)
        assertEquals("缓冲保留", "ni", fake.buf)
        state.toggleTraditional() // 繁 → 简（不是英文）
        assertEquals(
            EngineMode.PINYIN.value to InputScheme.NINE.value,
            fake.switchLanguageCalls.last(),
        )
        assertTrue("不许走清缓冲的 switchScheme", fake.schemeCalls.isEmpty())
        assertEquals("缓冲仍保留", "ni", fake.buf)
    }
}
