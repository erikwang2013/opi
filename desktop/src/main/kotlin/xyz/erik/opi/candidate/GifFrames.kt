// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! GIF 帧解码（自有表情页用，2026-10-19 用户裁决「面板里要显示真 GIF 动图」）。
//!
//! Compose Desktop 的 Image 不播 GIF ⇒ 用 JDK 自带的 ImageIO 把帧与 delay 解析出来，
//! 页面按时间取帧。不引新依赖；帧先降到 [GIF_FRAME_MAX_PX]，12 张图 × 10 帧的内存
//! 可控（128² RGBA ≈ 64KB/帧 ≈ 7.7MB 全部）。
//!
//! 坏图/非 GIF/无帧都返回 null —— 调用方退回静态 PNG + 相位动效，不让整页崩。

package xyz.erik.opi.candidate

import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.toComposeImageBitmap
import java.awt.RenderingHints
import java.awt.image.BufferedImage
import java.io.ByteArrayInputStream
import javax.imageio.ImageIO
import javax.imageio.metadata.IIOMetadata
import javax.imageio.metadata.IIOMetadataNode
import javax.imageio.stream.MemoryCacheImageInputStream
import kotlin.math.roundToInt

/** 解码目标边长（px）：格子约 40dp，高 DPI 下 128 仍够用。 */
internal const val GIF_FRAME_MAX_PX = 128

/** 缺 delay 元数据（录制/转码工具常见）时按 100ms，避免整段快闪。 */
private const val DEFAULT_DELAY_MS = 100

/**
 * 逐帧 delay 的下限：显式 0 的帧会让 [GifFrames.indexAt] 永远选不中它（`t < 0` 恒假），
 * 播起来等于跳帧；20ms 保住「每帧至少露一面」（[GifFrames.totalMs] 另有 ≥1 的兜底）。
 */
private const val MIN_DELAY_MS = 20

/** 一段可循环播放的 GIF：帧 + 每帧时长（ms）。 */
internal class GifFrames(val frames: List<ImageBitmap>, val delaysMs: List<Int>) {

    init {
        require(frames.isNotEmpty()) { "GIF 没有可用的帧" }
        require(frames.size == delaysMs.size) { "帧与 delay 数量不一致" }
    }

    val totalMs: Int = delaysMs.sum().coerceAtLeast(1)

    /** 循环时间 [elapsedMs] 落在第几帧。 */
    fun indexAt(elapsedMs: Long): Int {
        var t = (elapsedMs % totalMs).toInt()
        for (i in delaysMs.indices) {
            if (t < delaysMs[i]) return i
            t -= delaysMs[i]
        }
        return frames.lastIndex // 浮点/取整余量：落回最后一帧，不越界
    }
}

/** 解码 GIF 字节；不是多帧 GIF / 坏图 / 无帧返回 null。 */
internal fun decodeGifFrames(bytes: ByteArray, maxPx: Int = GIF_FRAME_MAX_PX): GifFrames? {
    val reader = ImageIO.getImageReadersByFormatName("gif").asSequence().firstOrNull() ?: return null
    val stream = MemoryCacheImageInputStream(ByteArrayInputStream(bytes))
    return try {
        reader.input = stream
        val count = reader.getNumImages(true)
        if (count <= 0) return null
        val frames = ArrayList<ImageBitmap>(count)
        val delays = ArrayList<Int>(count)
        for (i in 0 until count) {
            val image = reader.read(i) ?: continue
            frames += scaleToMax(image, maxPx).toComposeImageBitmap()
            delays += frameDelayMs(runCatching { reader.getImageMetadata(i) }.getOrNull())
        }
        if (frames.isEmpty()) null else GifFrames(frames, delays)
    } catch (e: Exception) {
        null
    } finally {
        reader.dispose()
        stream.close()
    }
}

/** GIF 帧的 delay（delayTime 单位 = 1/100 秒）；元数据缺失 → 100ms，显式 0 → 20ms 下限。 */
private fun frameDelayMs(metadata: IIOMetadata?): Int {
    val root = metadata?.getAsTree("javax_imageio_gif_image_1.0") as? IIOMetadataNode
        ?: return DEFAULT_DELAY_MS
    val control = root.getElementsByTagName("GraphicControlExtension").item(0) as? IIOMetadataNode
        ?: return DEFAULT_DELAY_MS
    val centis = control.getAttribute("delayTime").toIntOrNull() ?: return DEFAULT_DELAY_MS
    return (centis * 10).coerceAtLeast(MIN_DELAY_MS)
}

/** 等比缩到最长边 ≤ [maxPx]（不放大）；透明底用 ARGB 保住。 */
private fun scaleToMax(src: BufferedImage, maxPx: Int): BufferedImage {
    val scale = minOf(1.0, maxPx.toDouble() / maxOf(src.width, src.height))
    if (scale >= 1.0) return src
    val w = (src.width * scale).roundToInt().coerceAtLeast(1)
    val h = (src.height * scale).roundToInt().coerceAtLeast(1)
    val out = BufferedImage(w, h, BufferedImage.TYPE_INT_ARGB)
    val g = out.createGraphics()
    try {
        g.setRenderingHint(RenderingHints.KEY_INTERPOLATION, RenderingHints.VALUE_INTERPOLATION_BILINEAR)
        g.drawImage(src, 0, 0, w, h, null)
    } finally {
        g.dispose()
    }
    return out
}
