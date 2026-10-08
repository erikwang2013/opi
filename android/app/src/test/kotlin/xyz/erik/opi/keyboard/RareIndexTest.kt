// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** 生僻字索引：解析（含坏行容错）与康熙部首字形映射。 */
class RareIndexTest {

    @Test
    fun parsesAndGroups() {
        val idx = RareIndex.parse(
            """
            人	9	2
            仁	9	4
            保	9	7
            口	30	3
            """.trimIndent(),
        )
        assertEquals(listOf(2, 4, 7), RareIndex.strokeCounts(idx, 9))
        assertEquals(listOf("人"), RareIndex.chars(idx, 9, 2))
        assertEquals(listOf("保"), RareIndex.chars(idx, 9, 7))
        assertEquals(listOf("口"), RareIndex.chars(idx, 30, 3))
        assertTrue(RareIndex.chars(idx, 9, 99).isEmpty())
        assertTrue(RareIndex.strokeCounts(idx, 200).isEmpty())
    }

    @Test
    fun badLinesAreSkippedNotThrown() {
        val idx = RareIndex.parse(
            """
            人	9	2
            坏行
            人	x	2
            大	200	3
            多	9	4	5
            """.trimIndent(),
        )
        // 只剩第一行与「大」（部首 200 合法）
        assertEquals(listOf(9, 200), idx.keys.sorted())
        assertEquals(listOf(2), RareIndex.strokeCounts(idx, 9))
    }

    @Test
    fun kangxiRadicalGlyphsAreOneToOne() {
        // U+2F00 = 一（部首 1）、U+2FD5 = 龠（部首 214）—— 康熙部首区与 1..214 一一对应
        assertEquals("⼀", RareIndex.radicalGlyph(1))
        assertEquals("⿕", RareIndex.radicalGlyph(214))
        // 越界钳到边界（防御：调用方不该传越界，但不许崩）
        assertEquals("⼀", RareIndex.radicalGlyph(0))
        assertEquals("⿕", RareIndex.radicalGlyph(999))
    }
}
