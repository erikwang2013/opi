// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.engine.EngineMode
import xyz.erik.opi.engine.FakeEngine
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class KeyRouterTest {
    /** 假通道：记录提交/删除/回车。 */
    private class Channel {
        val commits = mutableListOf<String>()
        var backspaceCount = 0
        var enterCount = 0
    }

    private fun router(fake: FakeEngine, ch: Channel): Pair<EngineController, KeyRouter> {
        val ctrl = EngineController(fake)
        return ctrl to KeyRouter(
            ctrl,
            commit = { ch.commits += it },
            deleteBackward = { ch.backspaceCount++ },
            performEnter = { ch.enterCount++ },
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
    fun numberPadDigitEntersBufferAndCommitsNothing() {
        val fake = FakeEngine().apply { mode = EngineMode.NUMBER.value }
        val ch = Channel()
        val (_, r) = router(fake, ch)
        r.handleKey("1")
        assertEquals("1", fake.buf)
        assertTrue(ch.commits.isEmpty())
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
}
