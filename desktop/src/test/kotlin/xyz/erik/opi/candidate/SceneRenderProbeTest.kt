// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 动态插画背景的像素探针（2026-10-30）：证明 `shared/scene` 的两支绘制
//! （手机整幅 / 桌面条外角色）真画得出东西，并把 PNG 落到
//! `build/opi-render-probe/scene-*.png` 供人眼检查。
//!
//! 它证的是「画得出来」，**不证**「窗开起来长这样」（窗口裁剪/透明由
//! BarHaloTest 的几何门禁与实际桌面验收负责）。

package xyz.erik.opi.candidate

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import org.jetbrains.skia.EncodedImageFormat
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import xyz.erik.opi.scene.HALO_BOTTOM_ANCHORS
import xyz.erik.opi.scene.HALO_TOP_ANCHORS
import xyz.erik.opi.scene.ScenePalette
import xyz.erik.opi.scene.SceneTheme
import xyz.erik.opi.scene.drawOpiSceneBackdrop
import xyz.erik.opi.scene.drawOpiSceneHalo
import java.awt.image.BufferedImage
import java.io.File
import javax.imageio.ImageIO

class SceneRenderProbeTest {

    private val outDir = File("build/opi-render-probe")

    /** 宣纸色：浅底 + 深字 + 赭色强调，验证浅色皮肤的自动调色。 */
    private val palette = ScenePalette(
        base = Color(0xFFF5EFE1),
        ink = Color(0xFF3D3222),
        accent = Color(0xFF8A5A2B),
    )

    private fun render(name: String, width: Int, height: Int, content: @Composable () -> Unit): BufferedImage {
        val scene = ImageComposeScene(width = width, height = height, density = Density(1f)) { content() }
        val bytes = try {
            scene.render() // 首帧：布局 + 资源
            scene.render().encodeToData(EncodedImageFormat.PNG)?.bytes
                ?: throw AssertionError("PNG 编码失败")
        } finally {
            scene.close()
        }
        outDir.mkdirs()
        val file = File(outDir, "scene-$name.png")
        file.writeBytes(bytes)
        return ImageIO.read(file) ?: throw AssertionError("读不出 scene-$name.png")
    }

    private fun hasInk(img: BufferedImage, y0: Int, y1: Int): Boolean {
        for (y in y0 until y1) for (x in 0 until img.width) {
            if (((img.getRGB(x, y) ushr 24) and 0xFF) > 8) return true
        }
        return false
    }

    /** 深色皮肤（星光）：角色仍要浮得出来（描边用主文字色翻转）。 */
    @Test
    fun `halo scene stays visible on a dark skin`() {
        val dark = ScenePalette(
            base = Color(0xFF1B2138),
            ink = Color(0xFFE8EBF7),
            accent = Color(0xFFFFD966),
        )
        val w = 420
        val h = 86
        val halo = 22f
        val img = render("halo-dark", w, h) {
            Canvas(Modifier.fillMaxSize()) {
                drawOpiSceneHalo(
                    palette = dark,
                    theme = SceneTheme.MEADOW,
                    phase = 0.2f,
                    halo = halo,
                    barTop = halo,
                    barBottom = halo + 42f,
                    topXs = haloAnchorXs(size.width, halo, HALO_TOP_ANCHORS),
                    bottomXs = haloAnchorXs(size.width, halo, HALO_BOTTOM_ANCHORS),
                )
            }
        }
        assertTrue("深色皮肤上条外角色也该有墨迹", hasInk(img, 0, 21) && hasInk(img, 65, h))
    }

    @Test
    fun `halo scene draws above and below the bar`() {
        val w = 420
        val h = 86
        val halo = 22f
        val barHeight = 42f
        val img = render("halo", w, h) {
            Canvas(Modifier.fillMaxSize()) {
                drawOpiSceneHalo(
                    palette = palette,
                    theme = SceneTheme.MEADOW,
                    phase = 0.2f,
                    halo = halo,
                    barTop = halo,
                    barBottom = halo + barHeight,
                    topXs = haloAnchorXs(size.width, halo, HALO_TOP_ANCHORS),
                    bottomXs = haloAnchorXs(size.width, halo, HALO_BOTTOM_ANCHORS),
                )
            }
        }
        assertTrue("条沿上方（y < 22）该有猫/兔/小人/气球", hasInk(img, 0, 21))
        assertTrue("条沿下方（y > 64）该有花草", hasInk(img, 65, h))
    }

    @Test
    fun `backdrop scene fills the keyboard area with more than a flat colour`() {
        val img = render("backdrop", 420, 250) {
            Canvas(Modifier.fillMaxSize()) {
                drawOpiSceneBackdrop(palette, SceneTheme.MEADOW, phase = 0.3f)
            }
        }
        assertTrue("整幅背景该有内容", hasInk(img, 0, img.height))
        val colors = mutableSetOf<Int>()
        for (y in 0 until img.height step 4) for (x in 0 until img.width step 4) {
            colors += img.getRGB(x, y)
        }
        assertTrue("背景不该是一块纯色（采样到 ${colors.size} 色）", colors.size > 8)
    }

    @Test
    fun `scene themes have unique ids and fall back to meadow`() {
        assertEquals(SceneTheme.entries.size, SceneTheme.entries.map { it.id }.toSet().size)
        assertEquals(SceneTheme.MEADOW, SceneTheme.fromId("no-such-theme"))
        assertEquals(SceneTheme.MEADOW, SceneTheme.fromId(null))
    }

    /** 每套主题各渲一张条外 + 一张整幅，落 PNG 供人眼比较；新主题不得是空白。 */
    @Test
    fun `every theme renders a halo and a backdrop`() {
        val haloSize = 22f
        for (theme in SceneTheme.entries) {
            val halo = render("halo-${theme.id}", 420, 86) {
                Canvas(Modifier.fillMaxSize()) {
                    drawOpiSceneHalo(
                        palette = palette,
                        theme = theme,
                        phase = 0.25f,
                        halo = haloSize,
                        barTop = haloSize,
                        barBottom = haloSize + 42f,
                        topXs = haloAnchorXs(size.width, haloSize, HALO_TOP_ANCHORS),
                        bottomXs = haloAnchorXs(size.width, haloSize, HALO_BOTTOM_ANCHORS),
                    )
                }
            }
            assertTrue("${theme.label} 的条外角色画空了", hasInk(halo, 0, 21) || hasInk(halo, 65, halo.height))
            val backdrop = render("backdrop-${theme.id}", 420, 250) {
                Canvas(Modifier.fillMaxSize()) {
                    drawOpiSceneBackdrop(palette, theme, phase = 0.25f)
                }
            }
            assertTrue("${theme.label} 的整幅背景画空了", hasInk(backdrop, 0, backdrop.height))
        }
    }

    /** 整条构图（条 + 键帽底 + 条外角色）：给人眼看，也防「条盖住整幅画」。 */
    @Test
    fun `full status bar with the overflow scene renders`() {
        val buttons = StatusBarButton.entries
        val metrics = barMetrics(100, false)
        val barW = statusBarWidthDp(buttons.size, 100, false)
        val halo = 22.dp
        val w = (barW + 2f * halo.value).toInt()
        val h = (metrics.height.value + 2f * halo.value).toInt()
        val barPalette = skinColors(SkinOption.PAPER)
        val img = render("statusbar", w, h) {
            CompositionLocalProvider(
                LocalBarPalette provides barPalette,
                LocalBarSceneActive provides true,
            ) {
                Box(Modifier.fillMaxSize()) {
                    StatusBarOverflowScene(barPalette, SceneTheme.MEADOW, halo, metrics.height, Modifier.fillMaxSize())
                    val shape = RoundedCornerShape((metrics.button.value * 0.18f).dp)
                    Box(
                        Modifier.offset(halo, halo).size(barW.dp, metrics.height)
                            .clip(barShape(false))
                            .border(1.dp, barPalette.line),
                    ) {
                        StatusBarSceneBackdrop(barPalette, SceneTheme.MEADOW, Modifier.fillMaxSize())
                        Box(Modifier.fillMaxSize().background(barPalette.bg.copy(alpha = 0.42f)))
                        Row(
                            Modifier.fillMaxSize().padding(horizontal = metrics.pad),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(metrics.gap),
                        ) {
                            for (b in buttons) {
                                Box(
                                    Modifier.size(metrics.button).clip(shape)
                                        .background(lerp(barPalette.bg, barPalette.fg, 0.07f).copy(alpha = 0.62f))
                                        .border(1.dp, barPalette.line.copy(alpha = 0.72f), shape),
                                    contentAlignment = Alignment.Center,
                                ) {
                                    StatusBarButtonFace(b, ImeState(), MODE_DEFAULT, metrics.button)
                                }
                            }
                        }
                    }
                }
            }
        }
        assertTrue("整条构图该有内容", hasInk(img, 0, img.height))
    }
}
