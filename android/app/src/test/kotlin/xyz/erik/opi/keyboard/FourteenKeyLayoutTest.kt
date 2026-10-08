// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** 14 键布局表门禁。发送键 = 字母对首字母（与 scheme.rs 的规范符号约定一致）。 */
class FourteenKeyLayoutTest {

    /** scheme.rs `Scheme::Fourteen.key_class` 的快照（2026-10-09，交叉比对用）。 */
    private val fourteenPairs = mapOf(
        'q' to "qw", 'e' to "er", 't' to "ty", 'u' to "ui", 'o' to "op",
        'a' to "as", 'd' to "df", 'g' to "gh", 'j' to "jk", 'l' to "l",
        'z' to "zx", 'c' to "cv", 'b' to "bn", 'm' to "m",
    )

    @Test
    fun fourteenKeysWithExactlyTwentySixLetters() {
        val keys = fourteenKeyRows.flatten()
        assertEquals(14, keys.size)
        val letters = keys.flatMap { it.label.toList() }
        assertEquals(26, letters.size)
        assertEquals(26, letters.toSet().size)
    }

    @Test
    fun labelLettersMatchSchemePairs() {
        for (k in fourteenKeyRows.flatten()) {
            val expected = fourteenPairs[k.key.first()] ?: error("键 ${k.key} 不在对表里")
            assertEquals("键 ${k.label} 与 scheme.rs 的字母对不符", expected, k.label)
            assertTrue("发送键必须是首字母", k.key.first() == k.label.first())
        }
    }
}
