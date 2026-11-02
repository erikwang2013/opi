// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 键盘动态背景的 Android 接线（2026-10-30；2026-11-01 用户订正）：插画是**整页
//! 皮肤**，不是只铺在按键背后的背景 —— 工具条、候选栏、键区、面板都只压一层半透明
//! 主题色，让同一张连续插画从所有表面透出来；按键不贴独立插画/纹理。
//! 绘制放在 `drawBehind` 里、相位只在绘制阶段读 —— 每帧只重画背景，不重组键盘。

package xyz.erik.opi.ime

import androidx.compose.foundation.background
import androidx.compose.runtime.Composable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.text.TextStyle
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.currentStateAsState
import xyz.erik.opi.keyboard.LocalKeyboardPalette
import xyz.erik.opi.scene.ScenePalette
import xyz.erik.opi.scene.SceneTheme
import xyz.erik.opi.scene.drawOpiSceneBackdrop
import xyz.erik.opi.scene.rememberOpiScenePhase

/** 键区底色不透明度：留 55% 给插画，颜色仍来自当前皮肤。 */
internal const val KEY_AREA_SCRIM_ALPHA = 0.45f

/** 动态皮肤下的键区底色：几乎全透明，插画连成一片（键面自己再压薄薄一层）。 */
internal const val KEY_AREA_SCENE_SCRIM_ALPHA = 0.06f

/** 当前动态皮肤；各面板据此铺插画（null = 纯色皮肤，保持原来的纯色底）。 */
internal val LocalSceneTheme = compositionLocalOf<SceneTheme?> { null }

/** 工具条/候选栏底色不透明度：只留一层主题色，插画仍清晰可见。 */
internal const val TOOLBAR_SCENE_ALPHA = 0.40f

/** 候选栏与工具条同一档（候选文字另加阴影保证可读）。 */
internal const val BAR_SCENE_ALPHA = 0.42f

/** 动态皮肤下的表面色：只压一层薄主题色（纯色皮肤原值返回）。 */
@Composable
internal fun sceneSurface(color: Color, alpha: Float = 0.42f): Color =
    if (LocalSceneTheme.current != null) color.copy(alpha = alpha) else color

/** 动态皮肤下文字阴影：浅字配黑、深字配白，插画再花也读得清。 */
@Composable
internal fun sceneTextStyle(fg: Color): TextStyle {
    if (LocalSceneTheme.current == null) return TextStyle.Default
    val shadowColor = if (fg.luminance() > 0.5f) {
        Color.Black.copy(alpha = 0.55f)
    } else {
        Color.White.copy(alpha = 0.55f)
    }
    return TextStyle(
        shadow = Shadow(
            color = shadowColor,
            offset = Offset(0f, 1f),
            blurRadius = 3f,
        ),
    )
}

/**
 * 是否该跑无限动画：IME 窗口可见时生命周期是 RESUMED，收起后 [xyz.erik.opi.OpiImeService]
 * 把它降到 STARTED —— 此时 Compose 重组器仍在跑，不主动停相位就会继续按 60fps 空转。
 */
@Composable
internal fun animationsActive(): Boolean {
    val state by LocalLifecycleOwner.current.lifecycle.currentStateAsState()
    return state == Lifecycle.State.RESUMED
}

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
    val phase = rememberOpiScenePhase(animate = animationsActive())
    return drawBehind {
        drawRect(p.barBg)
        drawOpiSceneBackdrop(scene, theme, phase.value)
    }
}

/** 面板底色不透明度：插画是整页皮肤，面板只压薄薄一层主题色。 */
private const val PANEL_SCENE_SCRIM_ALPHA = 0.42f

/** 面板底：动态皮肤 = 插画 + 半透明面板色；纯色皮肤 = 原来的不透明面板色。 */
@Composable
internal fun Modifier.opiPanelBackground(base: Color): Modifier {
    val theme = LocalSceneTheme.current
    if (theme == null) return background(base)
    val p = LocalKeyboardPalette.current
    val scene = remember(p, base) {
        ScenePalette(base = base, ink = p.keyFg, accent = p.accent)
    }
    val phase = rememberOpiScenePhase(animate = animationsActive())
    return drawBehind {
        drawOpiSceneBackdrop(scene, theme, phase.value)
        drawRect(base.copy(alpha = PANEL_SCENE_SCRIM_ALPHA))
    }
}
