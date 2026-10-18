// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.pet

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 桌面 classpath 的 `pet-font/` 接线 + 共享 TSV 解析器。
 *
 * 钉三件事：构建期 `copyPetStickers` 真把三份文件拷进来；`map.tsv` 的 12 行与
 * [PetEmojiMap] 的形态/标签/文件一一对应；解析器对坏表头/坏码位/重复行会响亮地死。
 */
class PetFontResourcesTest {

    @Test
    fun `three font files are on the classpath`() {
        assertNotNull("classpath 缺 /pet-font/map.tsv", PetFontResources.map())
        assertNotNull("classpath 缺 /pet-font/shortcodes.tsv", PetFontResources.shortcodes())
        val font = PetFontResources.fontBytes()
        assertNotNull("classpath 缺 /pet-font/opi-emoji.ttf", font)
        assertTrue("TTF 太小", font!!.size > 10_000)
        // sfnt version：TrueType 0x00010000 或 'OTTO' / 'true'。
        val sfnt = font.copyOfRange(0, 4)
        val recognized = sfnt.contentEquals(byteArrayOf(0, 1, 0, 0)) ||
            sfnt.contentEquals("OTTO".toByteArray(Charsets.US_ASCII)) ||
            sfnt.contentEquals("true".toByteArray(Charsets.US_ASCII))
        assertTrue("不是可识别的 sfnt 字体", recognized)
    }

    @Test
    fun `map rows match the shared form contract`() {
        val glyphs = PetFontResources.map()!!
        assertEquals("12 个可挑形态", 12, glyphs.size)
        assertEquals(PetEmojiMap.forms.size, glyphs.size)
        for ((glyph, form) in glyphs.zip(PetEmojiMap.forms)) {
            assertEquals(form.mood.name, glyph.mood)
            assertEquals(PetStickerAssets.stickerFile(form.mood), glyph.file)
            assertEquals(form.label, glyph.label)
        }
        // 码位按枚举序从 U+E000 连续分配（字体生成器的分配契约）。
        assertEquals((0..11).map { 0xE000 + it }, glyphs.map { it.codepoint })
    }

    @Test
    fun `shortcodes agree with the map`() {
        val map = PetFontResources.map()!!
        val shorts = PetFontResources.shortcodes()!!
        assertEquals(12, shorts.size)
        assertEquals(map.map { it.shortcode }, shorts.map { it.shortcode })
        assertEquals(map.map { it.file }, shorts.map { it.file })
        for (s in shorts) assertEquals(":opi_", s.shortcode.take(5))
    }

    @Test
    fun `parser rejects malformed tables instead of silently dropping rows`() {
        val mapHeader = "mood\tcodepoint\tfile\tlabel_zh\tshortcode"
        val mapCases = listOf(
            "wrong\tE000\tidle.png\t笑\t:opi_smile:",
            "$mapHeader\nIDLE\tU+0041\tidle.png\t笑\t:opi_smile:",
            "$mapHeader\nIDLE\tU+E000\tidle.png\t笑\tsmile",
            "$mapHeader\nIDLE\tU+E000\tidle.png\t笑\t:opi_smile:\nIDLE\tU+E001\ttwo.png\t二\t:opi_two:",
            "$mapHeader\nIDLE\tU+E000\tidle.png\t笑\t:opi_smile:\nWAITING\tU+E000\ttwo.png\t二\t:opi_two:",
        )
        for (tsv in mapCases) {
            assertTrue("坏 map 表应抛异常：$tsv", runCatching { PetEmojiFont.parseMap(tsv) }.isFailure)
        }

        val shortHeader = "shortcode\tfile\tlabel_zh"
        val shortCases = listOf(
            "wrong\tfile\tlabel_zh",
            "$shortHeader\n",
            "$shortHeader\n:opi_smile:\tidle.png\t笑\n:opi_smile:\tidle.png\t笑",
        )
        for (tsv in shortCases) {
            assertTrue(
                "坏 shortcodes 表应抛异常：$tsv",
                runCatching { PetEmojiFont.parseShortcodes(tsv) }.isFailure,
            )
        }
    }
}
