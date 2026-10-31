// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 键盘动态背景的 Android 接线（2026-10-30）：把 `shared/scene` 的插画铺在
//! ImeScreen 根列后面。绘制放在 `drawBehind` 里、相位只在绘制阶段读 —— 每帧只
//! 重画背景，不重组键盘。
//!
//! 键区自己还压一层 [KEY_AREA_SCRIM_ALPHA] 的 `barBg`：插画在键缝里可见，键面
//! 仍由不透明键帽（`keyPlate`）保证文字对比度 —— 背景不能为了好看牺牲可读性。

package xyz.erik.opi.ime

import androidx.compose.foundation.background
import androidx.compose.runtime.Composable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import xyz.erik.opi.keyboard.LocalKeyboardPalette
import xyz.erik.opi.scene.ScenePalette
import xyz.erik.opi.scene.SceneTheme
import xyz.erik.opi.scene.drawOpiSceneBackdrop
import xyz.erik.opi.scene.rememberOpiScenePhase

/** 键区底色不透明度：留 55% 给插画，颜色仍来自当前皮肤。 */
internal const val KEY_AREA_SCRIM_ALPHA = 0.45f

/** 动态皮肤下的键区底色：再让出插画，键面自己另有半透明键帽（见 KeyPlate.kt）。 */
internal const val KEY_AREA_SCENE_SCRIM_ALPHA = 0.16f

/** 当前是不是动态插画皮肤（键面需要透出插画并给文字加一点阴影）。 */
internal val LocalSceneActive = compositionLocalOf { false }

/** 工具条底色不透明度：图标区几乎不透明，只让插画透一成（候选栏保持全不透明保可读性）。 */
internal const val TOOLBAR_SCENE_ALPHA = 0.90f

/** 根列背景：动态主题 = 插画，纯色皮肤（null）= 皮肤底色。 */
@Composable
internal fun Modifier.opiSkinBackground(theme: SceneTheme?): Modifier =
    if (theme == null) background(LocalKeyboardPalette.current.barBg)
    else opiSceneBackground(theme)

/** 根列背景 = 皮肤底色 + 动态插画（只在绘制阶段刷新）。 */
@Composable
internal fun Modifier.opiSceneBackground(theme: SceneTheme): Modifier {
    val p = LocalKeyboardPalette.current
    val scene = remember(p) {
        ScenePalette(base = p.barBg, ink = p.keyFg, accent = p.accent)
    }
    val phase = rememberOpiScenePhase()
    return drawBehind {
        drawRect(p.barBg)
        drawOpiSceneBackdrop(scene, theme, phase.value)
    }
}
