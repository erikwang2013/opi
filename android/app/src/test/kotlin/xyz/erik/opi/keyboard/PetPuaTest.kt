// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import xyz.erik.opi.pet.PetEmojiFont
import xyz.erik.opi.pet.PetMood
import java.io.File

/**
 * 自有表情 PUA 模式（2026-10-19）。
 *
 * 码位真源 = `shared/pet/emoji-font/map.tsv`：这里直接读源文件，再用共享解析器 +
 * [PetEmojiFont.puaFor] 逐形态比对。图片通道的真实插入要真机；上屏分支用源码门禁
 * 钉住（PUA 是 opt-in，不得顶掉既有 GIF/PNG 通道），开关默认值在 ImePreferencesTest。
 */
class PetPuaTest {

    private fun src(path: String): String = File(path).readText()

    @Test
    fun `pua characters match the source map tsv codepoints`() {
        val tsv = File("../../shared/pet/emoji-font/map.tsv")
        assertTrue("map.tsv 源文件不在了（搬家了？）：$tsv", tsv.exists())
        val lines = tsv.readLines().drop(1).filter { it.isNotBlank() }
        assertEquals("自有 = 十二形态", 12, lines.size)
        val glyphs = PetEmojiFont.parseMap(tsv.readText())
        for (line in lines) {
            val cells = line.split('\t')
            val mood = PetMood.valueOf(cells[0])
            val expected = String(Character.toChars(cells[1].removePrefix("U+").toInt(16)))
            assertEquals("${mood.name} 的 PUA 字符应来自 map.tsv", expected, PetEmojiFont.puaFor(glyphs, mood))
        }
    }

    @Test
    fun `android commit branch uses the preference and keeps the image path`() {
        val commit = src("src/main/kotlin/xyz/erik/opi/PetStickerCommit.kt")
        for (needle in listOf(
            "ImePreferences.from(this).puaEmoji",
            "PetFontAssets.preload(assets)",
            "PetEmojiFont.puaFor(it, mood)",
            "ic.commitText(pua, 1)",
            "回退图片通道",
        )) {
            assertTrue("PUA 分支缺：$needle", commit.contains(needle))
        }
        // PUA 是 opt-in：默认路径的 GIF → PNG → 文本标签一档都不能少。
        assertTrue(commit.contains("commitPetStickerImage(ic, gif, \"image/gif\")"))
        assertTrue(commit.contains("commitPetStickerImage(ic, png, \"image/png\")"))
        assertTrue(commit.contains("[\${PetEmoji.label(mood)}]"))
    }

    @Test
    fun `settings page exposes the pua toggle and the tofu warning`() {
        val screen = src("src/main/kotlin/xyz/erik/opi/settings/SettingsScreen.kt")
        assertTrue(screen.contains("自有表情用字体字符（PUA）"))
        assertTrue("开关说明必须写明 Android 其他应用加载不了该字体", screen.contains("无法加载"))
        assertTrue("开关说明必须写明方框代价", screen.contains("显示为方框"))
        assertTrue("开关没有落盘", screen.contains("prefs.puaEmoji = v"))
    }
}
