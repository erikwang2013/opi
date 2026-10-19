// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.pet

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Android 侧对共享表解析器的门禁。
 *
 * `AssetManager` 读 assets 需要真机/Robolectric（本模块没有），JVM 侧直接解析
 * 生成器产物，钉住「12 个形态、稳定码位 ID 连续、与 [PetEmojiMap] 同序」；运行时
 * [PetEmojiTables] 只负责把同一份字节从 assets 读出来。
 */
class PetEmojiTableTest {

    private val dir = File("../../shared/pet/emoji-font")

    @Test
    fun `generated map matches the shared forms`() {
        val glyphs = PetEmojiTable.parseMap(File(dir, PetEmojiTable.MAP_FILE).readText())
        assertEquals(12, glyphs.size)
        assertEquals(PetEmojiMap.forms.map { it.mood.name }, glyphs.map { it.mood })
        assertEquals(PetEmojiMap.forms.map { it.label }, glyphs.map { it.label })
        assertEquals(PetEmojiMap.forms.map { PetStickerAssets.stickerFile(it.mood) }, glyphs.map { it.file })
        assertEquals((0..11).map { 0xE000 + it }, glyphs.map { it.codepoint })
    }

    @Test
    fun `generated shortcodes agree with the map`() {
        val map = PetEmojiTable.parseMap(File(dir, PetEmojiTable.MAP_FILE).readText())
        val shorts = PetEmojiTable.parseShortcodes(File(dir, PetEmojiTable.SHORTCODES_FILE).readText())
        assertEquals(map.map { it.shortcode }, shorts.map { it.shortcode })
        assertEquals(map.map { it.file }, shorts.map { it.file })
    }

    @Test
    fun `sticker gifs exist next to the tables`() {
        val gifDir = File(dir.parentFile, "stickers-gif")
        for (form in PetEmojiMap.forms) {
            val gif = File(gifDir, PetStickerAssets.gifFile(form.mood))
            assertTrue("缺 GIF：${gif.path}", gif.length() > 0)
            val head = gif.inputStream().use { it.readNBytes(6) }
            assertEquals("GIF89a", String(head, Charsets.US_ASCII))
        }
    }
}
