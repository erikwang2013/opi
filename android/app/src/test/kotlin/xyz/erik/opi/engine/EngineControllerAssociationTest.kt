// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** 联想态判据（Spec §5.1）：空缓冲 + 空逐字片段 + 有候选 + 中文模式。 */
class EngineControllerAssociationTest {
    @After
    fun resetGate() {
        AssociationGate.enabled = false
    }

    private fun associationController(): Pair<FakeEngine, EngineController> {
        AssociationGate.enabled = true
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
    fun activeRequiresEmptyBufferPendingCandidatesAndChineseMode() {
        val (fake, ctrl) = associationController()
        assertTrue(ctrl.associationActive)

        fake.buf = "ni"
        ctrl.refresh()
        assertFalse("缓冲非空不得是联想态", ctrl.associationActive)

        fake.buf = ""
        fake.pending = "·"
        ctrl.refresh()
        assertFalse("逐字片段非空不得是联想态", ctrl.associationActive)

        fake.pending = ""
        fake.cands = null
        ctrl.refresh()
        assertFalse("无候选不得是联想态", ctrl.associationActive)

        fake.cands = arrayOf("回家")
        fake.mode = EngineMode.ENGLISH.value
        ctrl.refresh()
        assertFalse("英文模式不得是联想态", ctrl.associationActive)

        fake.mode = EngineMode.TRADITIONAL.value
        ctrl.refresh()
        assertTrue("繁体模式同样是联想态", ctrl.associationActive)
    }

    @Test
    fun disabledGateHidesAssociationCandidates() {
        AssociationGate.enabled = false
        val fake = FakeEngine()
        val ctrl = EngineController(fake)
        fake.buf = ""
        fake.pending = ""
        fake.cands = arrayOf("回家")
        fake.mode = EngineMode.PINYIN.value
        ctrl.refresh()
        assertTrue("默认关闭：联想候选不得进 UI", ctrl.candidates.isEmpty())
        assertFalse(ctrl.associationActive)
        assertEquals("读数也不得残留", "", ctrl.reading)
    }

    @Test
    fun shiftTapRefreshesAfterSetShift() {
        val (fake, ctrl) = associationController()
        ctrl.shiftTap() // fake.setShift 在空缓冲时清 cands，镜像真引擎的 set_shift 清联想
        assertFalse(ctrl.associationActive)
    }
}
