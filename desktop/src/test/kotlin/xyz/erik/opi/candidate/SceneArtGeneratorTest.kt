// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 插画资产生成器（2026-10-31）：把 19 套场景按各自默认皮肤渲成高清 PNG，
//! 供 iOS / macOS / 鸿蒙共用 —— 三端不重写绘制代码，画面与 Android/桌面同源。
//!
//! 平时是空测试；只有设置 `OPI_SCENE_ART_OUT=<目录>` 时才真正写文件：
//!   OPI_SCENE_ART_OUT=shared/scene/art desktop/gradlew -p desktop test \
//!     --tests 'xyz.erik.opi.candidate.SceneArtGeneratorTest' --rerun-tasks
//! `OPI_SCENE_ART_ONLY=garden` 可只渲一套，便于调画。

package xyz.erik.opi.candidate

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Density
import org.jetbrains.skia.EncodedImageFormat
import org.junit.Test
import xyz.erik.opi.scene.ScenePalette
import xyz.erik.opi.scene.SceneTheme
import xyz.erik.opi.scene.drawOpiSceneBackdrop
import java.io.File

class SceneArtGeneratorTest {

    @Test
    fun generateSceneArt() {
        val out = System.getenv("OPI_SCENE_ART_OUT") ?: return
        val only = System.getenv("OPI_SCENE_ART_ONLY")?.takeIf { it.isNotBlank() }
        val width = System.getenv("OPI_SCENE_ART_W")?.toIntOrNull() ?: 1440
        val height = System.getenv("OPI_SCENE_ART_H")?.toIntOrNull() ?: 720
        val dir = File(out).apply { mkdirs() }
        for (theme in SceneTheme.entries) {
            if (only != null && theme.id != only) continue
            val colors = skinColors(theme.defaultSkin())
            val palette = ScenePalette(base = colors.bg, ink = colors.fg, accent = colors.accent)
            val scene = ImageComposeScene(width, height, density = Density(1f)) {
                Frame(palette, theme)
            }
            val bytes = try {
                scene.render()
                scene.render().encodeToData(EncodedImageFormat.PNG)?.bytes
                    ?: throw AssertionError("PNG 编码失败：${theme.id}")
            } finally {
                scene.close()
            }
            File(dir, "scene-${theme.id}.png").writeBytes(bytes)
        }
    }

    @Composable
    private fun Frame(palette: ScenePalette, theme: SceneTheme) {
        Canvas(Modifier.fillMaxSize()) {
            drawOpiSceneBackdrop(palette, theme, phase = 0.25f)
        }
    }
}
