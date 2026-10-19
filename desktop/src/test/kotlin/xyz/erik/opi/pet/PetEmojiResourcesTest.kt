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
 * 钉三件事：构建期 `copyPetStickers` 真把两份表拷进来；`map.tsv` 的 12 行与
 * [PetEmojiMap] 的形态/标签/文件一一对应；解析器对坏表头/坏码位/重复行会响亮地死。
 * 目录里**没有**字体文件（2026-10-20 用户裁决删除字体通道）。
 */
class PetEmojiResourcesTest {

    @Test
    fun `both tables are on the classpath`() {
        assertNotNull("classpath 缺 /pet-font/map.tsv", PetEmojiResources.map())
        assertNotNull("classpath 缺 /pet-font/shortcodes.tsv", PetEmojiResources.shortcodes())
    }

    @Test
    fun `map rows match the shared form contract`() {
        val glyphs = PetEmojiResources.map()!!
        assertEquals("12 个可挑形态", 12, glyphs.size)
        assertEquals(PetEmojiMap.forms.size, glyphs.size)
        for ((glyph, form) in glyphs.zip(PetEmojiMap.forms)) {
            assertEquals(form.mood.name, glyph.mood)
            assertEquals(PetStickerAssets.stickerFile(form.mood), glyph.file)
            assertEquals(form.label, glyph.label)
        }
        // 码位按枚举序从 U+E000 连续分配：只是**稳定内部 ID**，绝不插入输入框。
        assertEquals((0..11).map { 0xE000 + it }, glyphs.map { it.codepoint })
    }

    @Test
    fun `shortcodes agree with the map`() {
        val map = PetEmojiResources.map()!!
        val shorts = PetEmojiResources.shortcodes()!!
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
            assertTrue("坏 map 表应抛异常：$tsv", runCatching { PetEmojiTable.parseMap(tsv) }.isFailure)
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
                runCatching { PetEmojiTable.parseShortcodes(tsv) }.isFailure,
            )
        }
    }
}
