// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// Windows 自有表情剪贴板：AWT 的 imageFlavor 走 CF_DIB，很多聊天软件把透明通道
// 铺成白底；这里直接写原生剪贴板：
//   · 静态：PNG（注册格式 "PNG"/"image/png"）+ CF_DIBV5（32bpp + alpha 掩码）；
//   · 动图：注册格式 "GIF"/"image/gif" + CF_HDROP（临时 .gif 文件），聊天软件按
//     文件粘贴即可保留动图。
// 不写 CF_DIB：老的 32bpp DIB 不带 alpha，微信等会优先取它，结果还是白底。
// 原生写失败（剪贴板被占用等）由调用方回退到 AWT，不改变既有兜底。

package xyz.erik.opi.candidate

import com.sun.jna.Library
import com.sun.jna.Native
import com.sun.jna.Pointer
import com.sun.jna.WString
import java.awt.image.BufferedImage
import java.io.ByteArrayOutputStream
import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import javax.imageio.ImageIO

private interface WinClipboard : Library {
    fun OpenClipboard(owner: Pointer?): Boolean
    fun EmptyClipboard(): Boolean
    fun CloseClipboard(): Boolean
    fun SetClipboardData(format: Int, mem: Pointer): Pointer?
    fun RegisterClipboardFormatW(name: WString): Int
}

private interface WinGlobal : Library {
    fun GlobalAlloc(flags: Int, bytes: Int): Pointer?
    fun GlobalLock(mem: Pointer): Pointer?
    fun GlobalUnlock(mem: Pointer): Boolean
    fun GlobalFree(mem: Pointer): Pointer?
}

internal object WindowsStickerClipboard {
    private const val GMEM_MOVEABLE = 0x0002
    private const val CF_HDROP = 15
    private const val CF_DIBV5 = 17

    private val clip: WinClipboard by lazy {
        Native.load("user32", WinClipboard::class.java)
    }
    private val mem: WinGlobal by lazy {
        Native.load("kernel32", WinGlobal::class.java)
    }

    /** true = 已写入原生剪贴板；false = 调用方回退 AWT。 */
    fun copy(image: BufferedImage, gif: ByteArray?): Boolean {
        val png = encodePng(image) ?: return false
        return withClipboard {
            if (gif != null) {
                // 动图只放 GIF/文件：混入静态 DIB 会被微信优先取走，变成不动的一张。
                val file = writeTempGif(gif)
                var ok = false
                if (file != null) {
                    ok = setBytes("GIF", gif) or setBytes("image/gif", gif) or
                        setData(CF_HDROP, hdrop(file.absolutePath))
                }
                ok
            } else {
                setBytes("PNG", png) or setBytes("image/png", png) or
                    setData(CF_DIBV5, dibV5(image))
            }
        } || withClipboard {
            // 静态：PNG/DIBV5 全都失败时，保底再试一次单 PNG。
            setBytes("PNG", png)
        }
    }

    private fun withClipboard(body: () -> Boolean): Boolean {
        var opened = false
        for (i in 0 until 10) {
            if (clip.OpenClipboard(null)) {
                opened = true
                break
            }
            try {
                Thread.sleep(20L)
            } catch (_: InterruptedException) {
                return false
            }
        }
        if (!opened) return false
        return try {
            if (!clip.EmptyClipboard()) false else body()
        } catch (_: Throwable) {
            false
        } finally {
            runCatching { clip.CloseClipboard() }
        }
    }

    private fun setBytes(name: String, data: ByteArray): Boolean {
        if (data.isEmpty()) return false
        val id = clip.RegisterClipboardFormatW(WString(name))
        return id != 0 && setData(id, data)
    }

    private fun setData(format: Int, data: ByteArray): Boolean {
        if (data.isEmpty()) return false
        val handle = mem.GlobalAlloc(GMEM_MOVEABLE, data.size) ?: return false
        val dst = mem.GlobalLock(handle)
        if (dst == null) {
            mem.GlobalFree(handle)
            return false
        }
        dst.write(0L, data, 0, data.size)
        mem.GlobalUnlock(handle)
        val ok = clip.SetClipboardData(format, handle) != null
        if (!ok) mem.GlobalFree(handle)
        return ok
    }

    private fun encodePng(image: BufferedImage): ByteArray? = try {
        ByteArrayOutputStream().use {
            if (!ImageIO.write(image, "png", it)) return null
            it.toByteArray()
        }
    } catch (_: Throwable) {
        null
    }

    /** CF_DIBV5：124 字节头 + 自底向上的 BGRA。 */
    internal fun dibV5(image: BufferedImage): ByteArray {
        val w = image.width
        val h = image.height
        val out = ByteBuffer.allocate(124 + w * h * 4).order(ByteOrder.LITTLE_ENDIAN)
        out.putInt(124) // bV5Size
        out.putInt(w)
        out.putInt(h)
        out.putShort(1) // planes
        out.putShort(32) // bpp
        out.putInt(3) // BI_BITFIELDS
        out.putInt(w * h * 4)
        out.putInt(0).putInt(0) // xppm / yppm
        out.putInt(0).putInt(0) // clrUsed / clrImportant
        out.putInt(0x00FF0000) // red mask
        out.putInt(0x0000FF00) // green mask
        out.putInt(0x000000FF) // blue mask
        out.putInt(0xFF000000.toInt()) // alpha mask
        out.putInt(0x73524742) // LCS_sRGB
        repeat(9) { out.putInt(0) } // endpoints
        out.putInt(0).putInt(0).putInt(0) // gamma
        out.putInt(4) // LCS_GM_IMAGES
        out.putInt(0).putInt(0).putInt(0) // profile / reserved
        for (y in h - 1 downTo 0) {
            for (x in 0 until w) {
                val argb = image.getRGB(x, y)
                out.put((argb and 0xFF).toByte())
                out.put(((argb ushr 8) and 0xFF).toByte())
                out.put(((argb ushr 16) and 0xFF).toByte())
                out.put(((argb ushr 24) and 0xFF).toByte())
            }
        }
        return out.array()
    }

    /** CF_HDROP：DROPFILES(20 bytes, fWide=1) + UTF-16LE 路径 + 双 NUL。 */
    internal fun hdrop(path: String): ByteArray {
        val utf16 = path.toByteArray(Charsets.UTF_16LE)
        val out = ByteBuffer.allocate(20 + utf16.size + 2).order(ByteOrder.LITTLE_ENDIAN)
        out.putInt(20) // pFiles
        out.putInt(0).putInt(0) // pt
        out.putInt(0) // fNC
        out.putInt(1) // fWide
        out.put(utf16)
        out.putShort(0)
        return out.array()
    }

    private fun writeTempGif(gif: ByteArray): File? = try {
        val dir = File(System.getProperty("java.io.tmpdir") ?: ".", "opi-stickers")
        dir.mkdirs()
        val cutoff = System.currentTimeMillis() - 24L * 60 * 60 * 1000
        dir.listFiles()?.forEach { if (it.lastModified() < cutoff) it.delete() }
        val file = File(dir, "opi-${System.currentTimeMillis()}.gif")
        file.writeBytes(gif)
        file
    } catch (_: Throwable) {
        null
    }
}
