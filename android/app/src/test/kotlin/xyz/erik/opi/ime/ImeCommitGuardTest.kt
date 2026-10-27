// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.engine.EngineMode
import xyz.erik.opi.engine.FakeEngine
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** 自我提交窗口与选区变化收起联想（Spec §5.2）。 */
class ImeCommitGuardTest {
    private fun activeController(): Pair<FakeEngine, EngineController> {
        val fake = FakeEngine()
        val ctrl = EngineController(fake)
        fake.buf = ""
        fake.pending = ""
        fake.cands = arrayOf("回家")
        fake.mode = EngineMode.PINYIN.value
        ctrl.refresh()
        return fake to ctrl
    }

    @Test
    fun windowIsInclusiveAndExpires() {
        var t = 1000L
        val g = ImeCommitGuard(windowMs = 200, now = { t })
        assertFalse(g.inSelfCommitWindow())
        g.noteSelfCommit()
        assertTrue(g.inSelfCommitWindow())
        t += 200
        assertTrue("窗口边界含等号", g.inSelfCommitWindow())
        t += 1
        assertFalse(g.inSelfCommitWindow())
    }

    @Test
    fun consecutiveSelfCommitsExtendWindow() {
        var t = 1000L
        val g = ImeCommitGuard(windowMs = 200, now = { t })
        g.noteSelfCommit()
        t += 150
        g.noteSelfCommit() // 第一次窗口到 1200；第二次应续到 1350
        t += 150
        assertTrue(g.inSelfCommitWindow())
        t += 51
        assertFalse(g.inSelfCommitWindow())
    }

    @Test
    fun selectionChangeOutsideWindowDismissesAssociation() {
        var t = 1000L
        val g = ImeCommitGuard(windowMs = 200, now = { t })
        val (fake, ctrl) = activeController()
        g.onSelectionChanged(ctrl, 1, 2)
        assertEquals(1, fake.clearCalls)
        assertTrue(g.selectionNonEmpty)
    }

    @Test
    fun selectionChangeInsideSelfCommitWindowKeepsAssociation() {
        var t = 1000L
        val g = ImeCommitGuard(windowMs = 200, now = { t })
        val (fake, ctrl) = activeController()
        g.noteSelfCommit()
        g.onSelectionChanged(ctrl, 1, 2)
        assertEquals("自我提交触发的光标变化不得把刚建好的联想收掉", 0, fake.clearCalls)
    }
}
