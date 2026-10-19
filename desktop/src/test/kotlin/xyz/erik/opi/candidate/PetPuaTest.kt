// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 自有表情 PUA 模式的门禁（2026-10-19）。
//!
//! 钉三件事：
//! · 码位真源 = `shared/pet/emoji-font/map.tsv`（直接读源文件；classpath 副本经
//!   [PetEmojiFont.puaFor] 必须与之逐形态相同）；
//! · 开关缺省关、能经 ini 往返与 mergeIni 保存；
//! · 点击分流：关 = 图片剪贴板原样；Linux 开 = commit（`opi-ctl insert`）；
//!   Windows 开 = 文本剪贴板 + 粘贴提示（没有 TSF 插入面）。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import xyz.erik.opi.pet.PetEmojiFont
import xyz.erik.opi.pet.PetEmojiMap
import xyz.erik.opi.pet.PetFontResources
import xyz.erik.opi.pet.PetMood
import java.awt.image.BufferedImage
import java.nio.file.Files
import java.nio.file.Paths

class PetPuaTest {

    /** 源 TSV 的 mood → PUA 字符（不经 PetEmojiFont，避免自己证明自己）。 */
    private fun sourcePua(): Map<String, String> {
        val path = Paths.get("../shared/pet/emoji-font/map.tsv")
        assertTrue("map.tsv 源文件不在了（搬家了？）：$path", Files.exists(path))
        return Files.readAllLines(path).drop(1).filter { it.isNotBlank() }.associate { line ->
            val cells = line.split('\t')
            cells[0] to String(Character.toChars(cells[1].removePrefix("U+").toInt(16)))
        }
    }

    @Test
    fun `pua characters match the source map tsv codepoints`() {
        val expected = sourcePua()
        val glyphs = PetFontResources.map()
        assertNotNull("classpath 缺 map.tsv", glyphs)
        val rows = checkNotNull(glyphs)
        assertEquals("自有 = 十二形态", 12, expected.size)
        for (form in PetEmojiMap.forms) {
            assertEquals(
                "${form.mood} 的 PUA 字符应来自 map.tsv",
                expected[form.mood.name],
                PetEmojiFont.puaFor(rows, form.mood),
            )
        }
        // 输入状态（不记事 / 引擎降级）不是可挑形态，没有码位。
        assertNull(PetEmojiFont.puaFor(rows, PetMood.SLEEPY))
        assertNull(PetEmojiFont.puaFor(rows, PetMood.DEGRADED))
    }

    @Test
    fun `pua switch defaults off and persists through ini`() {
        assertFalse("PUA 必须缺省关（默认仍插图片）", OpSettings().puaEmoji)
        assertFalse(OpSettings.fromIni("").puaEmoji)
        val on = OpSettings(puaEmoji = true)
        assertTrue("toIni 缺 PetPua=True", on.toIni().contains("PetPua=True\n"))
        assertTrue("fromIni 读不回 PetPua", OpSettings.fromIni(on.toIni()).puaEmoji)
        // mergeIni 漏掉 PetPua 时，设置页保存会把开关悄悄抹回 False。
        val merged = mergeIni("Learn=True\n", on)
        assertTrue("mergeIni 没管 PetPua", OpSettings.fromIni(merged).puaEmoji)
        assertTrue("mergeIni 缺键时没补 PetPua", mergeIni("", OpSettings()).contains("PetPua=False"))
    }

    @Test
    fun `pua off keeps the image channel untouched`() {
        val form = PetEmojiMap.forms.first()
        var committed: String? = null
        var textCopied: String? = null
        val result = pickPetSticker(
            form,
            settings = { OpSettings(puaEmoji = false) },
            puaOf = { error("开关关着时不该解析 PUA 码位") },
            windows = false,
            commit = { committed = it; null },
            textClipboard = TextClipboard { textCopied = it; true },
            clipboard = ImageClipboard { _, _ -> true },
            load = { BufferedImage(1, 1, BufferedImage.TYPE_INT_ARGB) },
            loadGif = { null },
        )
        assertTrue(result.ok)
        assertEquals("图片通道的提示必须原样", "小欧表情已复制，可 Ctrl+V 粘贴", result.notice)
        assertNull("关着时不得上屏文本", committed)
        assertNull("关着时不得写文本剪贴板", textCopied)
    }

    @Test
    fun `pua on commits on linux and copies text on windows`() {
        val form = PetEmojiMap.forms.first()
        val pua = PetEmojiFont.puaFor(PetFontResources.map()!!, form.mood)!!
        var committed: String? = null
        var textCopied: String? = null

        val linux = pickPetSticker(
            form,
            settings = { OpSettings(puaEmoji = true) },
            puaOf = { pua },
            windows = false,
            commit = { committed = it; null },
            textClipboard = TextClipboard { textCopied = it; true },
            clipboard = ImageClipboard { _, _ -> error("PUA 开时不得走图片剪贴板") },
        )
        assertTrue(linux.ok)
        assertEquals("Linux 应把 PUA 字符交给 opi-ctl insert", pua, committed)
        assertNull(textCopied)
        assertTrue("Linux 提示应说已上屏：${linux.notice}", linux.notice.contains("上屏"))

        // opi-ctl 的失败短语要原样透出（与符号上屏同一条纪律）。
        val failed = pickPetSticker(
            form,
            settings = { OpSettings(puaEmoji = true) },
            puaOf = { pua },
            windows = false,
            commit = { "「opi-ctl insert」退出码 3" },
            textClipboard = TextClipboard { textCopied = it; true },
            clipboard = ImageClipboard { _, _ -> error("PUA 开时不得走图片剪贴板") },
        )
        assertFalse(failed.ok)
        assertEquals("「opi-ctl insert」退出码 3", failed.notice)

        textCopied = null
        committed = null
        val windows = pickPetSticker(
            form,
            settings = { OpSettings(puaEmoji = true) },
            puaOf = { pua },
            windows = true,
            commit = { error("Windows 没有 TSF 插入面，不该走 opi-ctl") },
            textClipboard = TextClipboard { textCopied = it; true },
            clipboard = ImageClipboard { _, _ -> error("PUA 开时不得走图片剪贴板") },
        )
        assertTrue(windows.ok)
        assertEquals("Windows 应把 PUA 字符放进文本剪贴板", pua, textCopied)
        assertNull(committed)
        assertTrue("Windows 提示应含粘贴指引：${windows.notice}", windows.notice.contains("Ctrl+V"))
        assertTrue("Windows 提示应带形态标签：${windows.notice}", windows.notice.contains(form.label))
    }

    @Test
    fun `missing pua row falls back to the image channel`() {
        val form = PetEmojiMap.forms.first()
        var imageCopied = false
        val result = pickPetSticker(
            form,
            settings = { OpSettings(puaEmoji = true) },
            puaOf = { null },
            commit = { error("没有码位就不该上屏") },
            textClipboard = TextClipboard { error("没有码位就不该写文本剪贴板") },
            clipboard = ImageClipboard { _, _ -> imageCopied = true; true },
            load = { BufferedImage(1, 1, BufferedImage.TYPE_INT_ARGB) },
            loadGif = { null },
        )
        assertTrue(result.ok)
        assertTrue("表缺失时应回退图片通道，而不是点了没反应", imageCopied)
    }
}
