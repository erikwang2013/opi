// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** 括号配对纯状态机：表与契约见 `shared/brackets/README.md`。 */
class BracketPairingTest {

    /** 假宿主：文档 + 光标，行为按 Android 宿主的最小模型。 */
    private class FakeHost : BracketHost {
        var doc = ""
        var caret = 0
        /** false = 模拟宿主不支持 setSelection（配对已插入，光标停在闭符号后）。 */
        var positionCaret = true
        var insertCalls = 0
        var skipCalls = 0
        var deleteCalls = 0

        override fun textBefore(length: Int): String =
            doc.substring(maxOf(0, caret - length), caret)

        override fun textAfter(length: Int): String =
            doc.substring(caret, minOf(doc.length, caret + length))

        override fun insertPair(pair: BracketPair): Boolean {
            insertCalls++
            rawInsert(pair.open + pair.close)
            if (positionCaret) caret -= pair.close.length
            return true
        }

        override fun skipClose() {
            skipCalls++
            if (caret < doc.length) caret++
        }

        override fun deletePair(): Boolean {
            deleteCalls++
            if (caret <= 0 || caret >= doc.length) return false
            doc = doc.removeRange(caret - 1, caret + 1)
            caret--
            return true
        }

        fun rawInsert(text: String) {
            doc = doc.substring(0, caret) + text + doc.substring(caret)
            caret += text.length
        }
    }

    /** 模拟 KeyRouter：配对没接住就原样上屏。 */
    private fun type(pairing: BracketPairing, host: FakeHost, text: String) {
        if (!pairing.commitTyped(text)) host.rawInsert(text)
    }

    @Test
    fun tableMatchesSharedReference() {
        assertEquals(
            listOf(
                "()", "[]", "{}", "（）", "【】", "《》", "「」", "『』", "“”", "‘’",
            ),
            BRACKET_PAIRS.map { it.open + it.close },
        )
        assertEquals("开符号不能重复", BRACKET_PAIRS.size, BRACKET_PAIRS.map { it.open }.toSet().size)
        assertEquals("闭符号不能重复", BRACKET_PAIRS.size, BRACKET_PAIRS.map { it.close }.toSet().size)
    }

    @Test
    fun openingInsertsPairAndLeavesCaretBetween() {
        val host = FakeHost().apply { doc = "ab"; caret = 2 }
        val pairing = BracketPairing(host)
        type(pairing, host, "（")
        assertEquals("ab（）", host.doc)
        assertEquals(3, host.caret)
        assertEquals(1, host.insertCalls)
    }

    @Test
    fun matchingCloseIsConsumed() {
        val host = FakeHost()
        val pairing = BracketPairing(host)
        type(pairing, host, "【")
        type(pairing, host, "】")
        assertEquals("【】", host.doc)
        assertEquals(2, host.caret)
        assertEquals("闭符号不重复插入", 1, host.insertCalls)
        assertEquals(1, host.skipCalls)
    }

    @Test
    fun closeWithoutTrackedPairIsNotConsumed() {
        val host = FakeHost().apply { doc = "（）"; caret = 1 }
        val pairing = BracketPairing(host)
        type(pairing, host, "）")
        assertEquals("（））", host.doc)
        assertEquals(0, host.skipCalls)
    }

    @Test
    fun backspaceDeletesTrackedEmptyPair() {
        val host = FakeHost().apply { doc = "ab"; caret = 2 }
        val pairing = BracketPairing(host)
        type(pairing, host, "「")
        assertTrue(pairing.backspace())
        assertEquals("ab", host.doc)
        assertEquals(2, host.caret)
        assertEquals(1, host.deleteCalls)
    }

    @Test
    fun backspaceAfterTypedContentIsNormal() {
        val host = FakeHost().apply { doc = "ab"; caret = 2 }
        val pairing = BracketPairing(host)
        type(pairing, host, "(")
        type(pairing, host, "x")
        assertFalse("有人打过的括号不算空配对", pairing.backspace())
        assertEquals("ab(x)", host.doc)
        assertEquals(0, host.deleteCalls)
    }

    @Test
    fun userTypedPairIsNotTracked() {
        val host = FakeHost().apply { doc = "（）"; caret = 1 }
        val pairing = BracketPairing(host)
        assertFalse(pairing.backspace())
        assertEquals("（）", host.doc)
        assertEquals(0, host.deleteCalls)
    }

    @Test
    fun caretMovedAwayDoesNotMatchSnapshot() {
        val host = FakeHost().apply { doc = "ab"; caret = 2 }
        val pairing = BracketPairing(host)
        type(pairing, host, "(")
        // 用户在前面又打了字：配对没变，但保存的上下文对不上了。
        host.doc = "z" + host.doc
        host.caret += 1
        assertFalse(pairing.backspace())
        assertEquals("zab()", host.doc)
        assertEquals(0, host.deleteCalls)
    }

    @Test
    fun failedCaretPositioningStillInsertsButDoesNotTrack() {
        val host = FakeHost().apply { positionCaret = false }
        val pairing = BracketPairing(host)
        assertTrue("配对已进文档 ⇒ 调用方不能再提交一次", pairing.commitTyped("（"))
        // 光标停在闭符号之后：再打闭符号应当原样插入，不能被当成「跳过」。
        assertFalse(pairing.commitTyped("）"))
        assertEquals("（）", host.doc)
        assertEquals(0, host.skipCalls)
    }

    @Test
    fun asciiAndCjkFormsPairWithTheSameWidthClose() {
        val host = FakeHost()
        val pairing = BracketPairing(host)
        type(pairing, host, "{")
        type(pairing, host, "}")
        assertEquals("{}", host.doc)
        assertEquals(2, host.caret)

        val cjk = FakeHost()
        val p2 = BracketPairing(cjk)
        type(p2, cjk, "《")
        type(p2, cjk, "》")
        assertEquals("《》", cjk.doc)
        assertEquals(2, cjk.caret)
    }
}
