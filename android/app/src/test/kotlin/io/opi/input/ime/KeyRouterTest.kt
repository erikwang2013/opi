package io.opi.input.ime

import io.opi.input.engine.EngineController
import io.opi.input.engine.EngineMode
import io.opi.input.engine.FakeEngine
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
        assertEquals(io.opi.input.engine.ShiftState.OFF, ctrl.shiftState)
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
        assertEquals(io.opi.input.engine.ShiftState.LOCK, ctrl.shiftState)
    }

    @Test
    fun englishBufferShiftStillRoutesToEngine() {
        val fake = FakeEngine().apply {
            mode = EngineMode.ENGLISH.value
            buf = "w"
        }
        val ch = Channel()
        val (ctrl, r) = router(fake, ch)
        ctrl.shiftTap()
        r.handleKey("a")
        // 直传大写仅限空缓冲；buffer 非空时字母仍走引擎路由，shift 不消费
        assertEquals(listOf("a"), fake.inputCalls)
        assertTrue(ch.commits.isEmpty())
        assertEquals(io.opi.input.engine.ShiftState.SINGLE, ctrl.shiftState)
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
}
