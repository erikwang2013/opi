// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 键面底板：半透明键面 + 1dp 描边，描边用主文字色反向衬出边界。
//! 动态皮肤只替换整页背景；动态主题下键面用主题纯色（不透明），不在按键上贴插画。
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
import xyz.erik.opi.ime.LocalSceneTheme

/** 键面不透明度：纯色皮肤留 14% 让底色透一点（保持原来观感）。 */
internal const val KEY_PLATE_ALPHA = 0.86f

/** 键面描边的不透明度：浅底深边、深底浅边（颜色取 keyFg，自动反向）。 */
internal const val KEY_EDGE_ALPHA = 0.28f

/** 圆角键面 + 描边。调用方负责先 `padding(1.dp)` 留键缝。 */
@Composable
internal fun Modifier.keyPlate(highlighted: Boolean = false): Modifier {
    val p = LocalKeyboardPalette.current
    val shape = RoundedCornerShape(6.dp)
    // 动态皮肤只替换整页背景：键面保持主题默认纯色，不让插画透进按键。
    val bg = if (highlighted) p.keyHighlightBg
    else if (LocalSceneTheme.current != null) p.keyBg
    else p.keyBg.copy(alpha = KEY_PLATE_ALPHA)
    val edge = if (highlighted) {
        p.keyHighlightFg.copy(alpha = 0.38f)
    } else {
        p.keyFg.copy(alpha = KEY_EDGE_ALPHA)
    }
    return clip(shape).background(bg).border(1.dp, edge, shape)
}
