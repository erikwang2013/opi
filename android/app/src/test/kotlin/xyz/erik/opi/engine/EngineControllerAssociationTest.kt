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
        AssociationGate.onChange = null
        AssociationGate.enabled = false
    }

    /** 2026-10-30 用户报障「开关关了还会联想，热重启前不生效」：App 设置页改开关
     * 只写 gate、不碰服务控制器 —— gate 变化必须主动通知运行中的 IME 重算候选。 */
    @Test
    fun gateChangeNotifiesRunningIme() {
        var calls = 0
        AssociationGate.onChange = { calls++ }
        AssociationGate.enabled = true
        assertEquals("打开开关也要通知（候选立即按新值重算）", 1, calls)
        AssociationGate.enabled = false
        assertEquals("关闭开关要通知（否则旧联想候选留在屏幕上）", 2, calls)
        AssociationGate.enabled = false
        assertEquals("同值重复赋值不通知", 2, calls)
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
        assertEquals(
            "只清 UI 列表不够：引擎里的联想也要清，否则数字/链式能选中看不见的联想",
            1,
            fake.clearCalls,
        )
    }

    @Test
    fun shiftTapRefreshesAfterSetShift() {
        val (fake, ctrl) = associationController()
        ctrl.shiftTap() // fake.setShift 在空缓冲时清 cands，镜像真引擎的 set_shift 清联想
        assertFalse(ctrl.associationActive)
    }
}
