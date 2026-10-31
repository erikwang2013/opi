// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 设置页（宿主 App）的动态插画底：与键盘面板同一份 `shared/scene` 绘制，
//! 只把底色取 Material 色板，纯色皮肤照旧不铺图。

package xyz.erik.opi.settings

import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import xyz.erik.opi.ime.LocalSceneTheme
import xyz.erik.opi.scene.ScenePalette
import xyz.erik.opi.scene.drawOpiSceneBackdrop
import xyz.erik.opi.scene.rememberOpiScenePhase

/** 设置页底色不透明度：Material 文字多，插画只做淡淡的底纹。 */
private const val SETTINGS_SCENE_SCRIM_ALPHA = 0.72f

/** 动态皮肤下的设置页底；纯色皮肤直接返回，不改原样式。 */
@Composable
internal fun Modifier.opiSettingsSceneBackground(): Modifier {
    val theme = LocalSceneTheme.current ?: return this
    val scheme = MaterialTheme.colorScheme
    val scene = remember(scheme) {
        ScenePalette(base = scheme.background, ink = scheme.onBackground, accent = scheme.primary)
    }
    val phase = rememberOpiScenePhase()
    return drawBehind {
        drawOpiSceneBackdrop(scene, theme, phase.value)
        drawRect(scheme.background.copy(alpha = SETTINGS_SCENE_SCRIM_ALPHA))
    }
}
