// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 自有表情页门禁（2026-10-18 用户裁决：自有是独立分类，点形态**插入图片**）。
//!
//! 钉四件事：
//! · 自有 = 12 个形态（PetEmojiMap 枚举序），标签/文件名一条对一条；
//! · 文件名契约来自 shared/pet/PetStickerAssets（与 Android 共用）；
//! · 点击自有格走**图片剪贴板**（注入假 Clipboard，不碰真系统剪贴板）；
//! · 12 张 PNG 真的在 classpath 上（`copyPetStickers` 构建接线断了会显示缺图占位）。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import xyz.erik.opi.pet.PetEmojiMap
import xyz.erik.opi.pet.PetMood
import xyz.erik.opi.pet.PetStickerAssets
import java.awt.image.BufferedImage

class PetStickerPageTest {

    @Test
    fun `own page has the twelve forms in enum order`() {
        assertEquals("自有 = 十二形态", 12, PetEmojiMap.forms.size)
        assertEquals(PetEmojiMap.forms, petStickerSearch(""))
        assertEquals(PetEmojiMap.forms.map { it.mood }, petStickerSearch("  ").map { it.mood })
    }

    @Test
    fun `sticker file names follow the shared contract`() {
        assertEquals("idle.png", PetStickerAssets.stickerFile(PetMood.IDLE))
        assertEquals("face-palm.png", PetStickerAssets.stickerFile(PetMood.FACE_PALM))
        assertEquals("tongue-out.png", PetStickerAssets.stickerFile(PetMood.TONGUE_OUT))
        val files = PetEmojiMap.forms.map { PetStickerAssets.stickerFile(it.mood) }
        assertEquals("12 个文件名不得重复", files.size, files.toSet().size)
        assertTrue("文件名必须是小写 .png", files.all { it == it.lowercase() && it.endsWith(".png") })
    }

    @Test
    fun `label search filters own forms locally`() {
        assertEquals(listOf("笑"), petStickerSearch("笑").map { it.label })
        assertEquals(listOf("哭"), petStickerSearch("哭").map { it.label })
        assertTrue(petStickerSearch("没有这个形态").isEmpty())
    }

    /** 点击自有格的唯一出口 = 图片剪贴板；返回的提示里没有可上屏文本。 */
    @Test
    fun `clicking an own cell copies the image instead of committing text`() {
        val image = BufferedImage(2, 2, BufferedImage.TYPE_INT_ARGB)
        val copied = ArrayList<BufferedImage>()
        val clipboard = ImageClipboard { copied += it; true }

        val result = copyPetSticker(PetEmojiMap.forms.first(), clipboard) { image }

        assertEquals(StickerCopyResult("小欧表情已复制，可 Ctrl+V 粘贴", true), result)
        assertSame("剪贴板应拿到同一张图", image, copied.single())
    }

    @Test
    fun `missing image and unavailable clipboard report distinct failures`() {
        val form = PetEmojiMap.forms.first()

        val missing = copyPetSticker(form, ImageClipboard { true }) { null }
        assertFalse(missing.ok)
        assertTrue("缺图提示应报路径", missing.notice.contains("缺失"))

        val unavailable = copyPetSticker(form, ImageClipboard { false }) { BufferedImage(1, 1, BufferedImage.TYPE_INT_ARGB) }
        assertFalse(unavailable.ok)
        assertTrue("剪贴板不可用应说剪贴板", unavailable.notice.contains("剪贴板"))
    }

    /** 构建期 `copyPetStickers` 的接线：12 张 PNG 都在 classpath 的 /pet/ 下。 */
    @Test
    fun `all twelve sticker images are on the classpath`() {
        for (form in PetEmojiMap.forms) {
            val file = PetStickerAssets.stickerFile(form.mood)
            assertNotNull("classpath 缺 /pet/$file（copyPetStickers 断了吗）", loadPetSticker(file))
        }
    }
}
