// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.engine.EngineMode
import xyz.erik.opi.engine.FakeEngine
import org.junit.Assert.assertEquals
import org.junit.Test

/** 联想态下的按键（Spec §5.1）：五个入口先收起联想、再执行原本的动作；点候选不收起。 */
class KeyRouterAssociationTest {
    private fun associationController(): Pair<FakeEngine, EngineController> {
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
    fun spaceDismissesThenCommitsSpace() {
        val (fake, ctrl) = associationController()
        val committed = mutableListOf<String>()
        val r = KeyRouter(ctrl, { committed += it }, {}, {})
        r.handleSpace()
        assertEquals(1, fake.clearCalls)
        assertEquals(listOf(" "), committed)
    }

    @Test
    fun backspaceDismissesThenDeletes() {
        val (fake, ctrl) = associationController()
        var deletes = 0
        val r = KeyRouter(ctrl, {}, { deletes++ }, {})
        r.handleBackspace()
        assertEquals(1, fake.clearCalls)
        assertEquals(1, deletes)
    }

    @Test
    fun enterDismissesThenPerformsEnter() {
        val (fake, ctrl) = associationController()
        var enters = 0
        val r = KeyRouter(ctrl, {}, {}, { enters++ })
        r.handleEnter()
        assertEquals(1, fake.clearCalls)
        assertEquals(1, enters)
    }

    @Test
    fun digitInputDismissesAndDoesNotSelectAssociation() {
        val (fake, ctrl) = associationController()
        val r = KeyRouter(ctrl, {}, {}, {})
        r.handleKey("2")
        assertEquals("数字键必须先收联想", 1, fake.clearCalls)
        assertEquals("数字必须落回输入路由，不得当联想选择键", listOf("2"), fake.inputCalls)
    }

    @Test
    fun candidateTapDoesNotDismiss() {
        val (fake, ctrl) = associationController()
        fake.selectResults[0] = "回家"
        val r = KeyRouter(ctrl, {}, {}, {})
        r.handleCandidate(0)
        assertEquals(0, fake.clearCalls)
    }
}
