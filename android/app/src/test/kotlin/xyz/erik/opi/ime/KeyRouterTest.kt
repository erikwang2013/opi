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

class KeyRouterTest {
    /** 假通道：记录提交/删除/回车。 */
    private class Channel {
        val commits = mutableListOf<String>()
        var backspaceCount = 0
        var enterCount = 0
        var syncCount = 0

        /** commit 与 sync 的**顺序**流水（二者次序是 2026-10-13 那条修复的判据）。 */
        val order = mutableListOf<String>()
    }

    private fun router(fake: FakeEngine, ch: Channel): Pair<EngineController, KeyRouter> {
        val ctrl = EngineController(fake)
        return ctrl to KeyRouter(
            ctrl,
            commit = { ch.commits += it; ch.order += "commit" },
            deleteBackward = { ch.backspaceCount++ },
            performEnter = { ch.enterCount++ },
            syncComposing = { ch.syncCount++; ch.order += "sync" },
        )
    }

    @Test
    fun englishEmptyBufferLetterPassesThrough() {
        val fake = FakeEngine().apply { mode = EngineMode.ENGLISH.value }
        val ch = Channel()
        val (ctrl, r) = router(fake, ch)
        r.handleKey("a")
        assertEquals(listOf("a"), ch.commits)
        assertTrue(fake.inputCalls.isEmpty())
        assertEquals("", ctrl.buffer)
    }

    @Test
    fun englishShiftSingleUppercasesThenConsumes() {
        val fake = FakeEngine().apply { mode = EngineMode.ENGLISH.value }
        val ch = Channel()
        val (ctrl, r) = router(fake, ch)
        ctrl.shiftTap()
        r.handleKey("a")
        r.handleKey("b")
        assertEquals(listOf("A", "b"), ch.commits)
        assertEquals(xyz.erik.opi.engine.ShiftState.OFF, ctrl.shiftState)
    }

    @Test
    fun englishShiftLockKeepsUppercase() {
        val fake = FakeEngine().apply { mode = EngineMode.ENGLISH.value }
        val ch = Channel()
        val (ctrl, r) = router(fake, ch)
        ctrl.shiftLongPress()
        r.handleKey("a")
        r.handleKey("b")
        assertEquals(listOf("A", "B"), ch.commits)
        assertEquals(xyz.erik.opi.engine.ShiftState.LOCK, ctrl.shiftState)
    }

    @Test
    fun englishAlwaysDirectCommitsEvenWithAPreservedBuffer() {
        // 2026-10-09 二期：语言切换**保留缓冲** ⇒「英文模式 + 缓冲非空」是个正常状态
        // （保留着的中文拼音，见 ImeState.cycleMode 的注释）。字母必须照英文直传
        // （shift SINGLE → 大写并消费），**绝不进引擎** —— 旧防御路径（非空走引擎）
        // 随这条语义一起退役，否则英文打不进去、全喂给保留的旧拼音。
        val fake = FakeEngine().apply {
            mode = EngineMode.ENGLISH.value
            buf = "w" // 保留中的拼音
        }
        val ch = Channel()
        val (ctrl, r) = router(fake, ch)
        ctrl.shiftTap()
        r.handleKey("a")
        assertEquals(emptyList<String>(), fake.inputCalls)
        assertEquals(listOf("A"), ch.commits)
        assertEquals(xyz.erik.opi.engine.ShiftState.OFF, ctrl.shiftState)
        assertEquals("保留的拼音不许被英文打字动到", "w", fake.buf)
    }

    @Test
    fun pinyinLetterRoutesToEngine() {
        val fake = FakeEngine().apply { buf = "w" }
        val ch = Channel()
        val (_, r) = router(fake, ch)
        r.handleKey("a")
        assertEquals(listOf("a"), fake.inputCalls)
        assertTrue(ch.commits.isEmpty())
    }

    @Test
    fun spaceWithBufferCommitsEngineResult() {
        val fake = FakeEngine().apply {
            buf = "w"
            spaceResult = "我"
        }
        val ch = Channel()
        val (_, r) = router(fake, ch)
        r.handleSpace()
        assertEquals(listOf("我"), ch.commits)
    }

    @Test
    fun spaceWithBufferAndEmptyResultDoesNotCommit() {
        val fake = FakeEngine().apply {
            buf = "w"
            spaceResult = ""
        }
        val ch = Channel()
        val (_, r) = router(fake, ch)
        r.handleSpace()
        assertTrue(ch.commits.isEmpty())
    }

    @Test
    fun spaceEmptyBufferCommitsSpace() {
        val fake = FakeEngine()
        val ch = Channel()
        val (_, r) = router(fake, ch)
        r.handleSpace()
        assertEquals(listOf(" "), ch.commits)
    }

    @Test
    fun enterWithBufferSelectsFirstAndCommits() {
        val fake = FakeEngine().apply {
            buf = "w"
            selectResults[0] = "我"
        }
        val ch = Channel()
        val (_, r) = router(fake, ch)
        r.handleEnter()
        assertEquals(listOf("我"), ch.commits)
        assertEquals(0, ch.enterCount)
    }

    @Test
    fun enterEmptyBufferPerformsEnter() {
        val fake = FakeEngine()
        val ch = Channel()
        val (_, r) = router(fake, ch)
        r.handleEnter()
        assertEquals(1, ch.enterCount)
        assertTrue(ch.commits.isEmpty())
    }

    @Test
    fun backspaceRoutesToEngineOrDeleteBackward() {
        val fake = FakeEngine()
        val ch = Channel()
        val (_, r) = router(fake, ch)
        r.handleBackspace()
        assertEquals(1, ch.backspaceCount)
        assertEquals(0, fake.backspaceCalls)
        // 有 buffer 走引擎
        val fake2 = FakeEngine().apply { buf = "w" }
        val ch2 = Channel()
        val (_, r2) = router(fake2, ch2)
        r2.handleBackspace()
        assertEquals(1, fake2.backspaceCalls)
        assertEquals(0, ch2.backspaceCount)
    }

    // ---- 数字面板走引擎（旧路径是 UI 直提 commitText，绕开引擎） ----

    @Test
    fun numberPadKeyCommitsEngineText() {
        // 面板的 `,` 是键盘键码：引擎给文本（半角原样 / 全角 `，`）就得上屏 ——
        // 丢弃返回值等于吞键，标点表在面板上就白加了
        val fake = FakeEngine().apply {
            mode = EngineMode.NUMBER.value
            inputResults[","] = ","
        }
        val ch = Channel()
        val (_, r) = router(fake, ch)
        r.handleKey(",")
        assertEquals(listOf(","), fake.inputCalls)
        assertEquals(listOf(","), ch.commits)
    }

    @Test
    fun numberPadDigitCommitsImmediatelyNotIntoBuffer() {
        // 语义订正（2026-10-10 用户报障）：旧版钉的是「数字进引擎缓冲、**不提交**」
        // （引擎 Number 模式攒缓冲、离开面板才收尾提交）——在数字面板上表现为
        // 「点了没反应、切回拼音才出来」。现改为一律**立即 commit、不经引擎**。
        val fake = FakeEngine().apply { mode = EngineMode.NUMBER.value }
        val ch = Channel()
        val (_, r) = router(fake, ch)
        r.handleKey("1")
        assertEquals("立即上屏", listOf("1"), ch.commits)
        assertEquals("不许再攒进引擎缓冲", "", fake.buf)
        assertTrue("不许再经引擎", fake.inputCalls.isEmpty())
    }

    @Test
    fun candidateSelectCommitsFromPage() {
        val fake = FakeEngine().apply {
            cands = Array(20) { "c$it" }
            selectResults[8] = "中"
        }
        val ch = Channel()
        val (ctrl, r) = router(fake, ch)
        ctrl.nextPage()
        r.handleCandidate(0)
        assertEquals(listOf("中"), ch.commits)
    }

    /**
     * 用户报障回归（2026-10-09）：「不停输入到没匹配，回退再输入，键盘没反应」。
     * 根因：显示读数曾被塞进 buffer() 出口，9 键无候选时读数为空 ⇒ 回退的
     * 「缓冲非空？」判假 ⇒ 走了 deleteBackward()（删**编辑器**文字），引擎缓冲里的
     * 数字一个没退，连着打看不出变化。判据：读数空但有数字缓冲时，回退必须走引擎。
     */
    @Test
    fun backspaceGoesToEngineEvenWhenTheReadingIsEmpty() {
        val fake = FakeEngine().apply {
            buf = "999" // 9 键的数字缓冲（无候选 → 读数空）
            displayBuf = ""
        }
        val ch = Channel()
        val (ctrl, r) = router(fake, ch)
        assertEquals("前置：读数空而缓冲非空", "", ctrl.reading)
        r.handleBackspace()
        assertEquals("不许删编辑器文字", 0, ch.backspaceCount)
        assertEquals("引擎缓冲要退回一格", 1, fake.backspaceCalls)
    }

    // ---- 数字面板立即上屏（2026-10-10 用户报障）----

    @Test
    fun numberPadDigitsCommitImmediately() {
        // 报障「切换到数字键盘点击数字没输入，再切回拼音才出来」：引擎 Number 模式把
        // 数字**攒进缓冲、收尾才提交**（composer.rs 的 Mode::Number）——在数字面板上
        // 读起来就是点了没反应。判据：Number 模式下数字必须**直接 commit 且不经引擎**。
        val fake = FakeEngine().apply { mode = EngineMode.NUMBER.value }
        val ch = Channel()
        val (_, r) = router(fake, ch)
        r.handleKey("1")
        assertEquals(listOf("1"), ch.commits)
        assertTrue("数字不许再进引擎缓冲", fake.inputCalls.isEmpty())
    }

    @Test
    fun numberPadPunctuationStillGoesThroughEngine() {
        // 直提只针对数字：`,`/`.` 仍走引擎（标点表与全角那两档在它们身上生效）。
        val fake = FakeEngine().apply {
            mode = EngineMode.NUMBER.value
            inputResults[","] = "，"
        }
        val ch = Channel()
        val (_, r) = router(fake, ch)
        r.handleKey(",")
        assertEquals(listOf(","), fake.inputCalls)
        assertEquals(listOf("，"), ch.commits)
    }

    /**
     * 2026-10-13 用户报障「连按一个键出省略号，确认后把所有内容替换」的回归钉：
     * 引擎驱动的提交**之后**必须重推组合串（剩余缓冲继续显示，见
     * KeyRouter.commitAndResync 头注）——且次序必须是 commit 在前、sync 在后。
     */
    @Test
    fun candidateCommitResyncsComposingAfterTheCommit() {
        val fake = FakeEngine().apply {
            schemeValue = InputScheme.NINE.value
            buf = "6666666666666"
            displayBuf = "mo'mo"
            selectResults[0] = "默默"
        }
        val ch = Channel()
        val (_, r) = router(fake, ch)
        r.handleCandidate(0)
        assertEquals(listOf("默默"), ch.commits)
        assertEquals("提交后必须重推一次组合串", 1, ch.syncCount)
        assertEquals("次序：先 commit 再 sync", listOf("commit", "sync"), ch.order)
    }

    /** 空格上屏（引擎候选）同样收尾。 */
    @Test
    fun spaceCommitResyncsComposing() {
        val fake = FakeEngine().apply { buf = "ni"; spaceResult = "你" }
        val ch = Channel()
        val (_, r) = router(fake, ch)
        r.handleSpace()
        assertEquals(listOf("你"), ch.commits)
        assertEquals(1, ch.syncCount)
        assertEquals(listOf("commit", "sync"), ch.order)
    }

    /**
     * 英文/数字**直提**路径绝不重推：那时缓冲可能是「切英文时保留的拼音」，
     * 重推会把它复活成预编辑（见 KeyRouter.commitAndResync 头注的 ⚠️）。
     */
    @Test
    fun englishDirectPathNeverResyncs() {
        val fake = FakeEngine().apply { mode = EngineMode.ENGLISH.value; buf = "ni" }
        val ch = Channel()
        val (_, r) = router(fake, ch)
        r.handleKey("a")
        assertEquals(listOf("a"), ch.commits)
        assertEquals("英文直提不许推组合串（会把保留的拼音复活）", 0, ch.syncCount)
    }
}
