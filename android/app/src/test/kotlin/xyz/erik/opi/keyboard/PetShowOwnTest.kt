// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * 「表情中显示自有」开关（2026-10-20 用户裁决）。
 *
 * 缺省关 ⇒ 表情页顶栏不出现「自有」；打开后仍走既有 GIF/PNG → 文本标签通道，
 * 不插 PUA 文本。图片插入的真机路径不在这里，本文件钉住开关的纯逻辑与源码分支。
 */
class PetShowOwnTest {

    private fun src(path: String): String = File(path).readText()

    @Test
    fun `own tab is hidden by default and first when enabled`() {
        assertEquals(listOf(EmojiSubTab.Emoji, EmojiSubTab.Kaomoji), emojiSubTabs(showOwn = false))
        assertEquals(
            listOf(EmojiSubTab.Pet, EmojiSubTab.Emoji, EmojiSubTab.Kaomoji),
            emojiSubTabs(showOwn = true),
        )
    }

    @Test
    fun `commit keeps the image path and never inserts pua text`() {
        val commit = src("src/main/kotlin/xyz/erik/opi/PetStickerCommit.kt")
        assertTrue(commit.contains("petStickerImageOrder(gif, png)"))
        assertTrue(commit.contains("commitPetStickerImage(ic, file, mime)"))
        assertTrue("图片失败后必须尝试剪贴板", commit.contains("copyPetStickerToClipboard"))
        assertTrue(commit.contains("[\${PetEmoji.label(mood)}]"))
        // 码位只是内部 ID：输入框里不能出现 PUA 字符（文本回退只允许人读的
        // `[小欧·笑]` 标签，所以不断言「没有 commitText」）。
        assertFalse("上屏路径不得再碰 PUA", commit.lowercase().contains("pua"))
        assertFalse("上屏路径不得构造 PUA 字符", commit.contains("0xE00"))
        assertFalse(commit.contains("Character.toChars"))
    }

    @Test
    fun `settings page exposes the show-own switch`() {
        val screen = src("src/main/kotlin/xyz/erik/opi/settings/SettingsScreen.kt")
        assertTrue(screen.contains("表情中显示自有"))
        assertTrue("开关没有落盘", screen.contains("prefs.showOwn = v"))
    }
}
