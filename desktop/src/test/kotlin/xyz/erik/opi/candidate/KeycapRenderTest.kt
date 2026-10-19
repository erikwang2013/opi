// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 键帽像素门禁（2026-10-19 设计图裁决）：enpro 与 MODE 的 en26 面。
//!
//! 两件事分开钉：
//! · **颜色**：白底圆角键帽 + 固定设计色（enpro 描边 #9AA0A6 / en #3C4043 / pro #5F6368；
//!   en26 三处全 #1A73E8）。白底在黑背景夹具上量（键帽固定白，白底白背景分不开）；
//! · **比例**：设计图量得 en 墨迹宽约 40%、垂直中心约 46%；pro/26 宽约 29%、在右下。
//!   槽位若被放大到填满键帽（2026-10-19 修过的偏差），宽度断言会红。
//!
//! 3× 渲染：1dp 描边 = 3px、角标有实心像素，颜色采样与 bbox 不被抗锯齿带偏。
//! PNG 落在 `build/opi-render-probe/keycap-*.png`，人眼可直接看。
//! 界同 StatusBarRenderProbeTest：只证离屏画得出来，不证真窗/真机字体渲染。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
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

class KeycapRenderTest {

    private val outDir = File("build/opi-render-probe")

    /** 3×：1dp 描边 = 3px，采样/bbox 稳定。 */
    private val d = 3

    /** 键帽边长与左上角（画布 px）；30dp 按钮居中、26dp 键帽居中。 */
    private val cap = 26 * d
    private val origin = 5 * d + (BTN * d - cap) / 2

    private val probeColors = SkinColors(
        bg = Color.White, fg = Color.Black, hover = Color.Black,
        line = Color.Black, accent = Color.Black,
    )

    /** 把一枚键帽面画进 ([boxDp] + 10dp)×[d] 的画布中心，落 PNG，返回图。 */
    private fun probe(
        name: String,
        bg: Color = Color.White,
        boxDp: Int = BTN,
        content: @Composable () -> Unit,
    ): BufferedImage {
        val side = (boxDp + 2 * 5) * d
        val scene = ImageComposeScene(width = side, height = side, density = Density(d.toFloat())) {
            CompositionLocalProvider(LocalBarPalette provides probeColors) {
                Box(Modifier.fillMaxSize().background(bg), contentAlignment = Alignment.Center) { content() }
            }
        }
        val bytes = try {
            scene.render()
            scene.render().encodeToData(EncodedImageFormat.PNG)?.bytes ?: throw AssertionError("PNG 编码失败")
        } finally {
            scene.close()
        }
        outDir.mkdirs()
        val file = File(outDir, "keycap-$name.png")
        file.writeBytes(bytes)
        return ImageIO.read(file) ?: throw AssertionError("读不出 ${file.name}")
    }

    @Composable
    private fun face(b: StatusBarButton, mode: String = MODE_DEFAULT, box: Int = BTN) {
        StatusBarButtonFace(b, ImeState(), mode, box.dp)
    }

    /** 两个 RGB 的最大通道差（[Color] 的 0..1 分量四舍五入回 8 bit）。 */
    private fun colorDiff(argb: Int, c: Color): Int {
        fun ch(shift: Int) = (argb shr shift) and 0xFF
        fun expected(v: Float) = (v * 255f + 0.5f).toInt()
        return maxOf(abs(ch(16) - expected(c.red)), abs(ch(8) - expected(c.green)), abs(ch(0) - expected(c.blue)))
    }

    private fun closest(img: BufferedImage, cols: IntRange, rows: IntRange, c: Color): Int {
        var best = Int.MAX_VALUE
        for (y in rows) for (x in cols) best = minOf(best, colorDiff(img.getRGB(x, y), c))
        return best
    }

    /** 墨迹（与白底差 > 40）的像素 bbox；[empty] = 这块区域没有墨迹。 */
    private data class Bbox(val l: Int, val t: Int, val r: Int, val b: Int) {
        val w get() = r - l + 1
        val h get() = b - t + 1
        val empty get() = r < l
        val cx get() = (l + r + 1) / 2f
        val cy get() = (t + b + 1) / 2f
    }

    private fun isInk(argb: Int): Boolean = maxOf(
        abs(((argb shr 16) and 0xFF) - 255),
        abs(((argb shr 8) and 0xFF) - 255),
        abs((argb and 0xFF) - 255),
    ) > 40

    private fun inkIn(img: BufferedImage, cols: IntRange, rows: IntRange): Bbox {
        var l = Int.MAX_VALUE
        var t = Int.MAX_VALUE
        var r = -1
        var b = -1
        for (y in rows) for (x in cols) {
            if (isInk(img.getRGB(x, y))) {
                if (x < l) l = x
                if (x > r) r = x
                if (y < t) t = y
                if (y > b) b = y
            }
        }
        return Bbox(l, t, r, b)
    }

    /** 画布 px → 键帽内相对坐标（0..1）。 */
    private fun rel(px: Float): Float = (px - origin) / cap

    private val cases = listOf(
        Triple("enpro", StatusBarButton.ENPRO, MODE_DEFAULT),
        Triple("en26", StatusBarButton.MODE, "en26"),
    )

    @Test
    fun `enpro and en26 are white keycaps with the design colors`() {
        // 黑底只是夹具 —— 白键帽要和背景分开才量得到「白底」。
        val fillX = origin + 3 * d
        val fillY = origin + 23 * d // 左下空白：en 槽与角标槽都不占
        for ((name, b, mode) in cases) {
            val img = probe(name, bg = Color.Black) { face(b, mode) }
            // enpro：描边 #9AA0A6 / en #3C4043 / pro #5F6368；en26：三处全 #1A73E8。
            val enPro = b == StatusBarButton.ENPRO
            val stroke = if (enPro) KEYCAP_ENPRO_STROKE else KEYCAP_EN26_BLUE
            val main = if (enPro) KEYCAP_ENPRO_MAIN else KEYCAP_EN26_BLUE
            val corner = if (enPro) KEYCAP_ENPRO_CORNER else KEYCAP_EN26_BLUE
            val white = colorDiff(img.getRGB(fillX, fillY), Color.White)
            // 左边缘 y=22dp：en 槽（底 0.60）与角标槽（x≥0.56）都不占，只有描边。
            val strokeInk = colorDiff(img.getRGB(origin + 1, origin + 22 * d), stroke)
            val mainInk = closest(
                img,
                origin + 3 * d..origin + 23 * d,
                origin + 2 * d..origin + 16 * d,
                main,
            )
            val cornerInk = closest(
                img,
                origin + 13 * d..origin + 24 * d,
                origin + 17 * d..origin + 24 * d,
                corner,
            )
            println("$name 键帽：白底差 $white、描边差 $strokeInk、en 差 $mainInk、角标差 $cornerInk")
            assertTrue("$name 键帽内该是纯白底（实测通道差 $white）", white <= 8)
            assertTrue("$name 描边该是 $stroke（实测通道差 $strokeInk）", strokeInk <= 24)
            assertTrue("$name 主字该是 $main（扫描最近像素差 $mainInk）", mainInk <= 24)
            assertTrue("$name 右下角标该是 $corner（扫描最近像素差 $cornerInk）", cornerInk <= 24)
        }
        // en26 只在 MODE 当前面就是 en+26 时套键帽；拼音档保持原字形面（无白底）。
        val pinyin = probe("mode-pinyin", bg = Color.Black) { face(StatusBarButton.MODE, mode = "pinyin") }
        val pinyinFill = colorDiff(pinyin.getRGB(fillX, fillY), Color.White)
        assertTrue("拼音档的 MODE 不该有白键帽（该点离白差 $pinyinFill）", pinyinFill > 200)
    }

    @Test
    fun `keycap glyphs keep the design proportions`() {
        // 扫描带留出比槽更大的范围（纵向放大要能露出来），同时避开 1dp 描边与圆角弧：
        // · en：x 0.10..0.80、y 0.25..0.68（槽 0.32..0.60）；
        // · 角标：x 0.45..0.93、y 0.68..0.93（右下圆角弧的内沿从 y≈0.94 起进入 x≤0.93）。
        for ((name, b, mode) in cases) {
            val img = probe("geom-$name") { face(b, mode) }
            val colsAll = origin + (cap * 0.10f).toInt()..origin + (cap * 0.80f).toInt()
            val enRows = origin + (cap * 0.25f).toInt()..origin + (cap * 0.68f).toInt()
            val cornerCols = origin + (cap * 0.45f).toInt()..origin + (cap * 0.93f).toInt()
            val cornerRows = origin + (cap * 0.68f).toInt()..origin + (cap * 0.93f).toInt()
            val en = inkIn(img, colsAll, enRows)
            val corner = inkIn(img, cornerCols, cornerRows)
            assertTrue("$name 的 en 该有墨迹（扫描带 $en）", !en.empty)
            assertTrue("$name 的角标该有墨迹（扫描带 $corner）", !corner.empty)
            val enW = en.w / cap.toFloat()
            val enH = en.h / cap.toFloat()
            val cornerW = corner.w / cap.toFloat()
            val cornerH = corner.h / cap.toFloat()
            val enCx = rel(en.cx)
            val enCy = rel(en.cy)
            val cornerCx = rel(corner.cx)
            val cornerCy = rel(corner.cy)
            println(
                "$name 比例：en ${"%.3f".format(enW)}×${"%.3f".format(enH)} 中心 " +
                    "(${"%.3f".format(enCx)}, ${"%.3f".format(enCy)})、角标 " +
                    "${"%.3f".format(cornerW)}×${"%.3f".format(cornerH)} 中心 " +
                    "(${"%.3f".format(cornerCx)}, ${"%.3f".format(cornerCy)})",
            )
            assertTrue("$name en 宽 $enW 应 ≤ 0.50 键帽宽（槽放大到填满键帽会在这里红）", enW <= 0.50f)
            assertTrue("$name en 高 $enH 应 ≤ 0.35 键帽高（设计 ~0.215）", enH <= 0.35f)
            assertTrue("$name 角标宽 $cornerW 应 ≤ 0.35 键帽宽（设计 ~0.29、槽 0.34）", cornerW <= 0.35f)
            assertTrue("$name 角标高 $cornerH 应 ≤ 0.25 键帽高（槽 0.21）", cornerH <= 0.25f)
            // 1× 下角标仍 ≥6px 宽（旧的「角落还读得出两个数字」产品下限）。
            assertTrue("$name 角标在 1× 下该 ≥ 6px（实测 ${"%.1f".format(cornerW * 26f)}px）", cornerW * 26f >= 6f)
            assertTrue("$name en 水平中心 $enCx 应 ≈ 0.44", abs(enCx - 0.44f) <= 0.05f)
            assertTrue("$name en 垂直中心 $enCy 应 ≈ 0.46（不是上半区）", abs(enCy - 0.46f) <= 0.05f)
            assertTrue("$name 角标水平中心 $cornerCx 应 ≈ 0.73（右下）", abs(cornerCx - 0.73f) <= 0.05f)
            assertTrue("$name 角标垂直中心 $cornerCy 应 ≈ 0.83（右下）", abs(cornerCy - 0.83f) <= 0.05f)
        }
    }

    @Test
    fun `keycap scales with the button box and keeps the design ratios`() {
        // 2026-10-19 统一缩放（StatusBarScale）：150% ⇒ 45dp 按钮盒；键帽仍按 26/30 × box
        // 画、en/角标比例不变（100% 那两条是同一套判据），且像素真的画出来（非黑底）。
        val box = (BTN * 1.5f).toInt() // 45
        val capPx = box * 26f / 30f * d
        val originPx = 5 * d + (box * d - capPx) / 2f
        fun lit(argb: Int) = maxOf((argb shr 16) and 0xFF, (argb shr 8) and 0xFF, argb and 0xFF) > 40
        for ((name, b, mode) in cases) {
            val img = probe("scale-$name", bg = Color.Black, boxDp = box) { face(b, mode, box) }
            // 非黑像素的 bbox = 键帽外沿（白底 + 描边 + 字），宽高应 ≈ box × 26/30。
            var l = Int.MAX_VALUE
            var t = Int.MAX_VALUE
            var r = -1
            var bt = -1
            for (y in 0 until img.height) for (x in 0 until img.width) {
                if (lit(img.getRGB(x, y))) {
                    if (x < l) l = x
                    if (x > r) r = x
                    if (y < t) t = y
                    if (y > bt) bt = y
                }
            }
            assertTrue(
                "$name 150% 键帽宽 ${r - l + 1}px 应 ≈ 26/30 × ${box}dp = ${capPx.toInt()}px",
                abs((r - l + 1) - capPx.toInt()) <= 2,
            )
            assertTrue(
                "$name 150% 键帽高 ${bt - t + 1}px 应 ≈ 26/30 × ${box}dp = ${capPx.toInt()}px",
                abs((bt - t + 1) - capPx.toInt()) <= 2,
            )
            val enPro = b == StatusBarButton.ENPRO
            val stroke = if (enPro) KEYCAP_ENPRO_STROKE else KEYCAP_EN26_BLUE
            val main = if (enPro) KEYCAP_ENPRO_MAIN else KEYCAP_EN26_BLUE
            val cornerColor = if (enPro) KEYCAP_ENPRO_CORNER else KEYCAP_EN26_BLUE
            val fillX = (originPx + capPx * 0.10f).toInt()
            val fillY = (originPx + capPx * 0.85f).toInt()
            assertTrue("$name 150% 键帽内该是纯白底", colorDiff(img.getRGB(fillX, fillY), Color.White) <= 8)
            val strokeInk = colorDiff(
                img.getRGB(originPx.toInt() + 1, (originPx + capPx * 0.85f).toInt()),
                stroke,
            )
            assertTrue("$name 150% 描边该是 $stroke（实测通道差 $strokeInk）", strokeInk <= 24)
            // 扫描带与 100% 用例同源（按键帽比例），只把原点/边长换成 150% 的。
            val enCols = (originPx + capPx * 0.10f).toInt()..(originPx + capPx * 0.80f).toInt()
            val enRows = (originPx + capPx * 0.25f).toInt()..(originPx + capPx * 0.68f).toInt()
            val cornerCols = (originPx + capPx * 0.45f).toInt()..(originPx + capPx * 0.93f).toInt()
            val cornerRows = (originPx + capPx * 0.68f).toInt()..(originPx + capPx * 0.93f).toInt()
            val en = inkIn(img, enCols, enRows)
            val corner = inkIn(img, cornerCols, cornerRows)
            assertTrue("$name 150% 的 en 该有墨迹（扫描带 $en）", !en.empty)
            assertTrue("$name 150% 的角标该有墨迹（扫描带 $corner）", !corner.empty)
            assertTrue("$name 150% en 宽 ${en.w / capPx} 应 ≤ 0.50 键帽宽", en.w / capPx <= 0.50f)
            assertTrue("$name 150% 角标宽 ${corner.w / capPx} 应 ≤ 0.35 键帽宽", corner.w / capPx <= 0.35f)
            assertTrue(
                "$name 150% 主字该是 $main",
                closest(img, enCols, enRows, main) <= 24,
            )
            assertTrue(
                "$name 150% 右下角标该是 $cornerColor",
                closest(img, cornerCols, cornerRows, cornerColor) <= 24,
            )
        }
    }
}
