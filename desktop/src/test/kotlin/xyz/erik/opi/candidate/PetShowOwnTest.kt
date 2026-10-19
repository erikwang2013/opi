// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「表情中显示自有」开关的门禁（2026-10-20 用户裁决）。
//!
//! 钉三件事：缺省关 ⇒ 表情页顶栏没有「自有」；开关经 ini 往返与 mergeIni 保存；
//! 自有格只剩图片剪贴板出口（字体字符/PUA 通道已删）。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class PetShowOwnTest {

    @Test
    fun `own tab is hidden by default and first when enabled`() {
        assertEquals(
            listOf(EmojiSubTab.Emoji, EmojiSubTab.Kaomoji),
            emojiSubTabs(showOwn = false),
        )
        assertEquals(
            listOf(EmojiSubTab.Own, EmojiSubTab.Emoji, EmojiSubTab.Kaomoji),
            emojiSubTabs(showOwn = true),
        )
    }

    @Test
    fun `show-own switch defaults off and persists through ini`() {
        assertFalse("自有必须缺省隐藏", OpSettings().showOwn)
        assertFalse(OpSettings.fromIni("").showOwn)
        val on = OpSettings(showOwn = true)
        assertTrue("toIni 缺 ShowOwn=True", on.toIni().contains("ShowOwn=True\n"))
        assertTrue("fromIni 读不回 ShowOwn", OpSettings.fromIni(on.toIni()).showOwn)
        // mergeIni 漏掉 ShowOwn 时，设置页保存会把开关悄悄抹回 False。
        val merged = mergeIni("Learn=True\n", on)
        assertTrue("mergeIni 没管 ShowOwn", OpSettings.fromIni(merged).showOwn)
        assertTrue("mergeIni 缺键时没补 ShowOwn", mergeIni("", OpSettings()).contains("ShowOwn=False"))
        // 旧版的 PetPua=True 不再有字体通道，读侧必须忽略（不能借尸还魂成显示自有）。
        assertFalse("旧 PetPua 键不该被读成 ShowOwn", OpSettings.fromIni("PetPua=True\n").showOwn)
    }

    @Test
    fun `sticker page has no text-insertion path`() {
        val page = File("src/main/kotlin/xyz/erik/opi/candidate/PetStickerPage.kt").readText()
        assertFalse("自有页不得再碰 PUA", page.lowercase().contains("pua"))
        assertFalse("自有页不得再有文本剪贴板出口", page.contains("TextClipboard"))
    }
}
