// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 小欧六形态的**离屏渲染门禁**（2026-10-17 移动端顶部宠物条裁决：笑、欢乐、哭、
//! 沮丧、惊恐、愤怒 —— 「要是动态的、有色彩感」）。
//!
//! 判据分三层，都是像素级的：
//! 1. **每张脸都画出来了**（墨迹非空）—— 空表 = 小欧没画，不能靠「不溢出」那种
//!    恒真式混过去；
//! 2. **六张脸互不相同**（两两像素差）—— 防止某个分支漏接、哭/沮丧/惊恐画成同一张；
//! 3. **有色**：哭有蓝眼泪、愤怒有红标记、欢乐有粉腮红 —— 用户要的「有色彩感」若
//!    只在背景不在小欧本体上，或点缀色被漏画，这里会红。
//!
//! PNG 落在 `/tmp/opi-render-probe/pet-*.png`，人眼可直接看（同 StatusBarRenderProbeTest
//! 的界：证「这些像素画得出来」，不证真机窗里的样子）。没有一条断言依赖本机字体 ——
//! 小欧是纯形状，CI 的 ubuntu runner 缺 CJK 字体也不影响。

package xyz.erik.opi.pet

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Alignment
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import org.jetbrains.skia.EncodedImageFormat
import org.junit.Assert.assertTrue
import org.junit.Test
import java.awt.image.BufferedImage
import java.io.File
import javax.imageio.ImageIO
import kotlin.math.abs

class OpiPetFormsRenderProbeTest {

    private val outDir = File("/tmp/opi-render-probe")

    /** 六形态与用户点名的次序一一对应（只用于出图/报错名，规格在 PetStripTest）。 */
    private val forms = listOf(
        "smile" to PetMood.IDLE,
        "joy" to PetMood.WAITING,
        "cry" to PetMood.CRYING,
        "dejected" to PetMood.PUZZLED,
        "terrified" to PetMood.TERRIFIED,
        "angry" to PetMood.ANGRY,
    )

    private fun render(name: String, mood: PetMood): BufferedImage {
        val scene = ImageComposeScene(width = 280, height = 280, density = Density(1f)) {
            Box(
                Modifier.fillMaxSize().background(Color.White),
                contentAlignment = Alignment.Center,
            ) {
                // size 是键帽**宽**；高按 240:250 推导（250dp）—— 280 画布四周各留 15px。
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
        File(outDir, "pet-$name.png").writeBytes(bytes)
        return ImageIO.read(File(outDir, "pet-$name.png"))
            ?: throw AssertionError("读不出 pet-$name.png")
    }

    /** 与纯白底差多少才算墨迹（与 StatusBarRenderProbeTest 同口径）。 */
    private fun isInk(argb: Int): Boolean = maxOf(
        abs(((argb shr 16) and 0xFF) - 255),
        abs(((argb shr 8) and 0xFF) - 255),
        abs((argb and 0xFF) - 255),
    ) > 40

    private fun inkCount(img: BufferedImage): Int {
        var n = 0
        for (y in 0 until img.height) for (x in 0 until img.width) {
            if (isInk(img.getRGB(x, y))) n++
        }
        return n
    }

    /** 两张图里「肉眼可辨不同」的像素数（任一通道差 > 24，抗锯齿边缘也算）。 */
    private fun diffCount(a: BufferedImage, b: BufferedImage): Int {
        var n = 0
        for (y in 0 until a.height) for (x in 0 until a.width) {
            val p = a.getRGB(x, y)
            val q = b.getRGB(x, y)
            if (abs(((p shr 16) and 0xFF) - ((q shr 16) and 0xFF)) > 24 ||
                abs(((p shr 8) and 0xFF) - ((q shr 8) and 0xFF)) > 24 ||
                abs((p and 0xFF) - (q and 0xFF)) > 24
            ) {
                n++
            }
        }
        return n
    }

    private fun colorCount(img: BufferedImage, test: (r: Int, g: Int, b: Int) -> Boolean): Int {
        var n = 0
        for (y in 0 until img.height) for (x in 0 until img.width) {
            val argb = img.getRGB(x, y)
            val r = (argb shr 16) and 0xFF
            val g = (argb shr 8) and 0xFF
            val b = argb and 0xFF
            if (test(r, g, b)) n++
        }
        return n
    }

    @Test
    fun `all six forms are drawn and visibly different`() {
        val rendered = forms.associate { (name, mood) -> name to render(name, mood) }
        val inks = rendered.mapValues { (_, img) -> inkCount(img) }
        println("小欧六形态墨迹像素：$inks")
        for ((name, ink) in inks) {
            // 正控：小欧总要占一块（240px 宽、约 236 高，墨迹几千像素起步）。
            assertTrue("$name 该画出小欧（墨迹 $ink 像素，不能是空表）", ink > 3000)
        }
        // 两两不同：六张脸必须真是六张脸（漏接分支会画成同一张）。
        val names = rendered.keys.toList()
        for (i in names.indices) for (j in i + 1 until names.size) {
            val d = diffCount(rendered.getValue(names[i]), rendered.getValue(names[j]))
            println("  ${names[i]} vs ${names[j]}：差异 $d 像素")
            // 脸上的眼睛/嘴/点缀至少几百像素；阈值取 120 是回归钉（实测按实现校准）。
            assertTrue(
                "${names[i]} 与 ${names[j]} 该是两种形态（差异 $d 像素）",
                d > 120,
            )
        }
    }

    @Test
    fun `the expressions carry their colors`() {
        val cry = render("cry", PetMood.CRYING)
        val angry = render("angry", PetMood.ANGRY)
        val joy = render("joy", PetMood.WAITING)
        // 蓝眼泪：蓝通道明显压过红绿。
        val blue = colorCount(cry) { r, g, b -> b > r + 25 && b > g + 10 }
        // 红怒气符号/红腮：红通道明显压过绿蓝。
        val red = colorCount(angry) { r, g, b -> r > g + 40 && r > b + 40 }
        // 粉腮红：红高于绿（浅粉白底不会落在这一带）。
        val pink = colorCount(joy) { r, g, b -> r > g + 18 && r > b + 8 }
        println("情绪色像素：哭-蓝 $blue / 愤怒-红 $red / 欢乐-粉 $pink")
        assertTrue("哭该有蓝色眼泪（蓝像素 $blue）", blue > 30)
        assertTrue("愤怒该有红色怒气/腮红（红像素 $red）", red > 30)
        assertTrue("欢乐该有粉色腮红（粉像素 $pink）", pink > 30)
    }
}
