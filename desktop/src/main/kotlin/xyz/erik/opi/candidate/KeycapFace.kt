// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 白底圆角键帽（2026-10-19 用户按设计图裁决）：英文专业键（en+pro）与 MODE 的
//! en26 面共用。按钮盒（100% = 30dp，缩放见 StatusBar.kt 的 BarMetrics）内按设计
//! 比例画键帽：26/30 边长、3.5/30 圆角、1/30 描边；「en」居中（大）、角标（pro / 26）
//! 右下（小）。
//!
//! ⚠️ 键帽是**设计图里的固定色**：白底 + 固定描边/文字色，不随皮肤变。激活态保留在
//! 键帽**外圈**（StatusBar.kt 的 BarButton 仍用 `palette.hover` 铺满按钮背景），
//! 所以切了 pro 也看得见那圈光晕，而键帽本身不被染色。
//!
//! 几何全按按钮边长等比（26/30、3.5/30），因此不走 [FACE_SCALE]（见 BarGlyphs.kt 的
//! `FACE_SCALE` 例外段）：设计量的是按钮内的绝对比例，不是 60% 内盒。

package xyz.erik.opi.candidate

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp

/** enpro 键帽的设计图色值（描边 / en / pro）。 */
internal val KEYCAP_ENPRO_STROKE = Color(0xFF9AA0A6)
internal val KEYCAP_ENPRO_MAIN = Color(0xFF3C4043)
internal val KEYCAP_ENPRO_CORNER = Color(0xFF5F6368)

/** en26（MODE 的英文档）键帽：描边与文字同色。 */
internal val KEYCAP_EN26_BLUE = Color(0xFF1A73E8)

/** 键帽边长 / 按钮边长（设计图：26dp / 30dp）。 */
private const val KEYCAP_FRACTION = 26f / 30f

/** 键帽圆角 / 按钮边长（设计图：约 3.5dp / 30dp）。 */
private const val KEYCAP_RADIUS_FRACTION = 3.5f / 30f

/** 描边 / 按钮边长（设计图：1dp / 30dp；随 box 等比，100% 时恰为 1dp）。 */
private const val KEYCAP_STROKE_FRACTION = 1f / 30f

/**
 * 把 [slots]（槽位坐标 = 键帽内盒 0..1）画进白底圆角键帽：[slots] 的**最后一槽**
 * 用 [cornerColor]（右下角标），其余用 [mainColor]。字号仍按墨迹取最大（[layoutGlyphs]）。
 */
@Composable
internal fun KeycapFace(
    slots: List<GlyphSlot>,
    mainColor: Color,
    cornerColor: Color,
    strokeColor: Color,
    box: Dp,
) {
    val density = LocalDensity.current
    val sidePx = with(density) { box.toPx() }
    val cap = sidePx * KEYCAP_FRACTION
    val inset = (sidePx - cap) / 2f
    val radius = sidePx * KEYCAP_RADIUS_FRACTION
    val stroke = sidePx * KEYCAP_STROKE_FRACTION
    // 槽位已按设计图留了边（不贴描边），直接按键帽内盒排版。
    val layout = remember(slots, cap) { layoutGlyphs(slots, cap, cap) }
    DisposableEffect(layout) { onDispose { layout.forEach { it.font.close() } } }
    val cornerText = slots.lastOrNull()?.text

    Canvas(Modifier.size(box)) {
        // 固定白底（设计图是固定色，不随皮肤变）。
        drawRoundRect(
            color = Color.White,
            topLeft = Offset(inset, inset),
            size = Size(cap, cap),
            cornerRadius = CornerRadius(radius),
        )
        // 描边收在键帽内：外沿正好贴 26dp 边，不会溢出按钮。
        drawRoundRect(
            color = strokeColor,
            topLeft = Offset(inset + stroke / 2f, inset + stroke / 2f),
            size = Size(cap - stroke, cap - stroke),
            cornerRadius = CornerRadius((radius - stroke / 2f).coerceAtLeast(0f)),
            style = Stroke(stroke),
        )
        // 字形按键帽内盒排好，画时整体平移到键帽左上角。
        translate(inset, inset) {
            drawGlyphs(layout.filter { it.text != cornerText }, mainColor)
            drawGlyphs(layout.filter { it.text == cornerText }, cornerColor)
        }
    }
}
