// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.engine.EngineMode
import xyz.erik.opi.engine.FakeEngine

/** 接线层：中文模式配对、英文/密码姿态不配对、⌫ 删整对（通道是否被调）。 */
class KeyRouterBracketTest {

    private class DocHost : BracketHost {
        var doc = ""
        var caret = 0
        var insertCalls = 0
        var deleteCalls = 0

        override fun textBefore(length: Int): String =
            doc.substring(maxOf(0, caret - length), caret)

        override fun textAfter(length: Int): String =
            doc.substring(caret, minOf(doc.length, caret + length))

        override fun insertPair(pair: BracketPair): Boolean {
            insertCalls++
            doc = doc.substring(0, caret) + pair.open + pair.close + doc.substring(caret)
            caret += pair.open.length
            return true
        }

        override fun skipClose() {
            if (caret < doc.length) caret++
        }

        override fun deletePair(): Boolean {
            deleteCalls++
            if (caret <= 0 || caret >= doc.length) return false
            doc = doc.removeRange(caret - 1, caret + 1)
            caret--
            return true
        }
    }

    private class Channel {
        val commits = mutableListOf<String>()
        var backspaces = 0
    }

    private fun router(
        fake: FakeEngine,
        host: DocHost,
        ch: Channel,
    ): KeyRouter {
        val ctrl = EngineController(fake)
        return KeyRouter(
            ctrl,
            commit = { ch.commits += it },
            deleteBackward = { ch.backspaces++ },
            performEnter = {},
            pairing = BracketPairing(host),
        )
    }

    @Test
    fun chinesePunctuationKeyInsertsPairThroughHost() {
        val fake = FakeEngine().apply { inputResults["("] = "（" }
        val host = DocHost()
        val ch = Channel()
        router(fake, host, ch).handleKey("(")
        assertEquals("（）", host.doc)
        assertEquals(1, host.caret)
        assertTrue("配对走宿主，不再普通提交", ch.commits.isEmpty())
    }

    @Test
    fun englishKeyIsNeverPaired() {
        val fake = FakeEngine().apply {
            mode = EngineMode.ENGLISH.value
            inputResults["("] = "("
        }
        val host = DocHost()
        val ch = Channel()
        router(fake, host, ch).handleKey("(")
        assertEquals(0, host.insertCalls)
        assertEquals(listOf("("), ch.commits)
    }

    @Test
    fun chineseSymbolPanelCommitPairs() {
        val host = DocHost()
        val ch = Channel()
        router(FakeEngine(), host, ch).commitText("「")
        assertEquals("「」", host.doc)
        assertEquals(1, host.caret)
        assertTrue(ch.commits.isEmpty())
    }

    @Test
    fun englishSymbolPanelCommitDoesNotPair() {
        val fake = FakeEngine().apply { mode = EngineMode.ENGLISH.value }
        val host = DocHost()
        val ch = Channel()
        router(fake, host, ch).commitText("「")
        assertEquals(0, host.insertCalls)
        assertEquals(listOf("「"), ch.commits)
    }

    @Test
    fun backspaceDeletesOnlyTheInsertedPair() {
        val fake = FakeEngine().apply { inputResults["["] = "【" }
        val host = DocHost()
        val ch = Channel()
        val r = router(fake, host, ch)
        r.handleKey("[")
        r.handleBackspace()
        assertEquals("", host.doc)
        assertEquals(0, host.caret)
        assertEquals(1, host.deleteCalls)
        assertEquals("不走普通退格", 0, ch.backspaces)
    }
}
