// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * 自有贴纸上屏的**决策层**门禁（纯 JVM：真机通道见 PetStickerConfigTest 的配置面）。
 *
 * 钉两件事：图片尝试顺序 GIF → PNG；失败后的回退互斥且文案能区分
 * 「已插入 / 已复制，请粘贴 / 已回退文字」—— 把「已复制」报成「已插入」会让用户
 * 对着没出现的图找不到原因。
 */
class PetStickerCommitTest {

    private val gif = File("idle.gif")
    private val png = File("idle.png")

    @Test
    fun `image attempts go gif then png`() {
        val order = petStickerImageOrder(gif, png)
        assertEquals(listOf("image/gif", "image/png"), order.map { it.first })
        assertEquals(listOf(gif, png), order.map { it.second })
        assertEquals(listOf("image/png"), petStickerImageOrder(null, png).map { it.first })
        assertEquals(listOf("image/gif"), petStickerImageOrder(gif, null).map { it.first })
        assertTrue("两张资产都缺时不得尝试任何图片通道", petStickerImageOrder(null, null).isEmpty())
    }

    @Test
    fun `outcome is exclusive and prefers the earliest working channel`() {
        assertEquals(PetStickerOutcome.INSERTED, petStickerOutcome(imageInserted = true, clipboardCopied = true))
        assertEquals(PetStickerOutcome.INSERTED, petStickerOutcome(imageInserted = true, clipboardCopied = false))
        assertEquals(PetStickerOutcome.COPIED, petStickerOutcome(imageInserted = false, clipboardCopied = true))
        assertEquals(PetStickerOutcome.TEXT_FALLBACK, petStickerOutcome(imageInserted = false, clipboardCopied = false))
    }

    @Test
    fun `notices distinguish pasted copied and text fallback`() {
        assertNull("图已插入时不需要提示", petStickerNotice(PetStickerOutcome.INSERTED))

        val copied = petStickerNotice(PetStickerOutcome.COPIED)!!
        assertTrue("已复制提示应教用户粘贴：$copied", copied.contains("复制") && copied.contains("粘贴"))
        assertTrue("应提示长按：$copied", copied.contains("长按"))

        val text = petStickerNotice(PetStickerOutcome.TEXT_FALLBACK)!!
        assertTrue("文字回退应说明用了文字标签：$text", text.contains("文字标签"))
        assertTrue("两种回退文案不得混用", copied != text)
    }
}
