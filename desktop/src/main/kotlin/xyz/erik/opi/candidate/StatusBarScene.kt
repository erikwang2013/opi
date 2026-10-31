// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏的动态插画层（2026-10-30）：把 `shared/scene` 的角色画在条窗的透明
//! 边距里。锚点与 [barWindowShape] 的岛屿同源（[haloAnchorXs]），否则角色会
//! 被窗口裁剪形状切掉。
//!
//! 相位只在 Canvas 的绘制阶段读 —— 每帧只重画这一层，按钮/轮询都不重组。

package xyz.erik.opi.candidate

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Dp
import xyz.erik.opi.scene.HALO_BOTTOM_ANCHORS
import xyz.erik.opi.scene.HALO_TOP_ANCHORS
import xyz.erik.opi.scene.ScenePalette
import xyz.erik.opi.scene.SceneTheme
import xyz.erik.opi.scene.drawOpiSceneBackdrop
import xyz.erik.opi.scene.drawOpiSceneHalo
import xyz.erik.opi.scene.rememberOpiScenePhase

/** 动态皮肤是否生效：条底板/按钮随之变半透明，让插画透到按键上。 */
internal val LocalBarSceneActive = compositionLocalOf { false }

/** 皮肤五色 → 插画三色（base/ink/accent），与 Android 侧同一条映射。 */
internal fun SkinColors.scenePalette(): ScenePalette =
    ScenePalette(base = bg, ink = fg, accent = accent)

/**
 * 条窗外溢出的插画。[halo] = 四周透明边距，[barHeight] = 条体高度；Canvas 大小
 * 就是整个窗口，条体会在它上面盖住角色下半身。
 */
@Composable
internal fun StatusBarOverflowScene(
    palette: SkinColors,
    theme: SceneTheme,
    halo: Dp,
    barHeight: Dp,
    modifier: Modifier = Modifier,
) {
    val scene = remember(palette) { palette.scenePalette() }
    val phase = rememberOpiScenePhase()
    Canvas(modifier.fillMaxSize()) {
        val h = halo.toPx()
        drawOpiSceneHalo(
            palette = scene,
            theme = theme,
            phase = phase.value,
            halo = h,
            barTop = h,
            barBottom = h + barHeight.toPx(),
            topXs = haloAnchorXs(size.width, h, HALO_TOP_ANCHORS),
            bottomXs = haloAnchorXs(size.width, h, HALO_BOTTOM_ANCHORS),
        )
    }
}

/** 条内插画：整窗铺一张低对比场景，条底板与按钮再压半透明层。 */
@Composable
internal fun StatusBarSceneBackdrop(
    palette: SkinColors,
    theme: SceneTheme,
    modifier: Modifier = Modifier,
) {
    val scene = remember(palette) { palette.scenePalette() }
    val phase = rememberOpiScenePhase()
    Canvas(modifier.fillMaxSize()) {
        drawOpiSceneBackdrop(scene, theme, phase.value, muted = 0.20f)
    }
}
