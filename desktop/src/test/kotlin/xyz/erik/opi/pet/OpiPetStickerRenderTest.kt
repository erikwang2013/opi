// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 小欧**透明底贴纸**的生成入口兼门禁（2026-10-18，「自有」表情页插入图片）。
//!
//! 与 [OpiPetFormsRenderProbeTest] 的分工：那个探针铺白底、断言语义（墨迹/差异/情绪色），
//! 是**像素门禁**；本文件不铺底 —— 直接让 Compose 渲到透明画布，产出「自有」页点形态
//! 后插入键盘的 PNG。两者共用 [OpiPet] 的同一套形状代码，不做第二套几何。
//!
//! 断言（贴纸特有的三条，像素级）：
//! 1. 四角 **alpha = 0** —— 白底漏进来会在这里红（透明底是本轮硬要求）；
//! 2. 中心 alpha = 255、全图不透明像素 > 3000 —— 脸真画出来了，不是一张全透明空表；
//! 3. 十二个形态各写出一张，文件名走 [PetStickerAssets.stickerFile] 契约。
//!
//! PNG 落在 `/tmp/opi-render-probe/sticker-<file>`；`scripts/gen_pet_stickers.py`
//! 跑本测试后把结果拷进 `shared/pet/stickers/`（测试**不写**仓库树，避免每次
//! `gradle test` 都改工作区）。

package xyz.erik.opi.pet

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Alignment
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import org.jetbrains.skia.EncodedImageFormat
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.awt.image.BufferedImage
import java.io.File
import javax.imageio.ImageIO

class OpiPetStickerRenderTest {

    private val outDir = File("/tmp/opi-render-probe")

    private fun render(mood: PetMood): BufferedImage {
        val scene = ImageComposeScene(width = 280, height = 280, density = Density(1f)) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                // 与白色探针同几何：280 画布、键帽宽 240dp（高 250dp），四周各留 15px。
                // 不挂 background —— 未绘制处保持透明。
                OpiPet(mood = mood, size = 240.dp)
            }
        }
        val bytes = try {
            scene.render() // 首帧：布局
            scene.render().encodeToData(EncodedImageFormat.PNG)?.bytes
                ?: throw AssertionError("PNG 编码失败")
        } finally {
            scene.close()
        }
        outDir.mkdirs()
        val file = File(outDir, "sticker-${PetStickerAssets.stickerFile(mood)}")
        file.writeBytes(bytes)
        return ImageIO.read(file) ?: throw AssertionError("读不出 ${file.path}")
    }

    private fun alpha(argb: Int): Int = (argb ushr 24) and 0xFF

    private fun opaqueCount(img: BufferedImage): Int {
        var n = 0
        for (y in 0 until img.height) for (x in 0 until img.width) {
            if (alpha(img.getRGB(x, y)) > 0) n++
        }
        return n
    }

    @Test
    fun `the twelve stickers render on a transparent canvas`() {
        val moods = PetMood.entries.filter { it != PetMood.SLEEPY && it != PetMood.DEGRADED }
        assertEquals("可挑形态应是十二个", 12, moods.size)
        for (mood in moods) {
            val img = render(mood)
            val name = PetStickerAssets.stickerFile(mood)
            assertEquals("$name 尺寸", 280, img.width)
            assertEquals("$name 尺寸", 280, img.height)
            // 透明底：四角必须 alpha=0（白底/漏铺底会在这里红）。
            for ((x, y) in listOf(0 to 0, 279 to 0, 0 to 279, 279 to 279)) {
                assertEquals("$name 的角 ($x,$y) 该透明", 0, alpha(img.getRGB(x, y)))
            }
            // 正控：中心是键帽正面，必须不透明；整张小欧也要占足像素。
            assertEquals("$name 中心该不透明", 255, alpha(img.getRGB(140, 140)))
            val opaque = opaqueCount(img)
            println("贴纸 $name：不透明像素 $opaque / 78400")
            assertTrue("$name 该画出小欧（不透明像素 $opaque）", opaque > 3000)
        }
    }
}
