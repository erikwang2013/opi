// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Test
import java.awt.image.BufferedImage
import java.nio.ByteBuffer
import java.nio.ByteOrder

class StickerClipboardWindowsTest {
    @Test
    fun `dibv5 carries alpha and bottom-up bgra`() {
        val image = BufferedImage(2, 2, BufferedImage.TYPE_INT_ARGB)
        image.setRGB(0, 0, 0x11223344) // top-left
        image.setRGB(0, 1, 0xAABBCCDD.toInt()) // bottom-left -> first pixel row written
        val bytes = WindowsStickerClipboard.dibV5(image)
        assertEquals(124 + 2 * 2 * 4, bytes.size)
        val b = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
        assertEquals(124, b.getInt(0))
        assertEquals(2, b.getInt(4))
        assertEquals(2, b.getInt(8))
        assertEquals(32, b.getShort(14).toInt())
        assertEquals(3, b.getInt(16)) // BI_BITFIELDS
        assertEquals(0xFF000000.toInt(), b.getInt(52))
        // 自底向上：文件里的第一行是图像最后一行；BGRA 顺序。
        assertEquals(0xDD.toByte(), b.get(124))
        assertEquals(0xCC.toByte(), b.get(125))
        assertEquals(0xBB.toByte(), b.get(126))
        assertEquals(0xAA.toByte(), b.get(127))
    }

    @Test
    fun `hdrop is a wide dropfiles path with double nul`() {
        val bytes = WindowsStickerClipboard.hdrop("C:\\表情\\a.gif")
        val b = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
        assertEquals(20, b.getInt(0))
        assertEquals(1, b.getInt(16))
        val path = String(bytes, 20, bytes.size - 22, Charsets.UTF_16LE)
        assertEquals("C:\\表情\\a.gif", path)
        assertEquals(0.toByte(), bytes[bytes.size - 2])
        assertEquals(0.toByte(), bytes[bytes.size - 1])
    }
}
