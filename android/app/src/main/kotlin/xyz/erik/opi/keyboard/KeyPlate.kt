// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 键面底板（2026-10-30 用户裁决「按键有时候还显示不分明」）：动态背景铺到键区
//! 后面之后，键面不能只靠 `keyBg` 和底色拉开差距 —— 浅色皮肤里两者本来就只差
//! 一档，再叠背景必然糊在一起。统一成「半透明键面 + 1dp 描边」：键面还带一点
//! 背景色（[KEY_PLATE_ALPHA]），描边用主文字色反向衬出边界，文字对比度不动。
//!
//! [KeyButton] 与 14 键/笔画的 [KeyBox] 共用这一处，别各写一份 alpha。

package xyz.erik.opi.keyboard

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import xyz.erik.opi.ime.LocalSceneActive

/** 键面不透明度：纯色皮肤留 14% 让底色透一点（保持原来观感）。 */
internal const val KEY_PLATE_ALPHA = 0.86f

/** 动态皮肤下键面更透：让插画真正出现在键帽上（文字用阴影补对比度）。 */
internal const val KEY_PLATE_SCENE_ALPHA = 0.68f

/** 键面描边的不透明度：浅底深边、深底浅边（颜色取 keyFg，自动反向）。 */
internal const val KEY_EDGE_ALPHA = 0.28f

/** 圆角键面 + 描边。调用方负责先 `padding(1.dp)` 留键缝。 */
@Composable
internal fun Modifier.keyPlate(highlighted: Boolean = false): Modifier {
    val p = LocalKeyboardPalette.current
    val shape = RoundedCornerShape(6.dp)
    val plateAlpha = if (LocalSceneActive.current) KEY_PLATE_SCENE_ALPHA else KEY_PLATE_ALPHA
    val bg = if (highlighted) p.keyHighlightBg else p.keyBg.copy(alpha = plateAlpha)
    val edge = if (highlighted) {
        p.keyHighlightFg.copy(alpha = 0.38f)
    } else {
        p.keyFg.copy(alpha = KEY_EDGE_ALPHA)
    }
    return clip(shape).background(bg).border(1.dp, edge, shape)
}
