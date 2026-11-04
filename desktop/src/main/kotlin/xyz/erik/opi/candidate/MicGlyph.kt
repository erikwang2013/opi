// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 话筒图标（从 BarGlyphs.kt 拆出守 500 行门禁；2026-11-02 用户要求「符号按钮后加话筒」）。

package xyz.erik.opi.candidate

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.Dp

/**
 * 话筒图标（2026-11-02 用户要求「符号按钮后加话筒」）：胶囊麦克风 + U 形托架 + 立杆。
 * 与其它自绘图标同一条纪律：填满传入的盒（= 按钮 × [FACE_SCALE]），几何全按盒比例。
 */
@Composable
internal fun MicIcon(box: Dp, active: Boolean = false, alpha: Float = 1f) {
    val palette = LocalBarPalette.current
    val fg = (if (active) palette.accent else palette.fg).copy(alpha = alpha)
    Canvas(Modifier.size(box)) {
        val d = size.minDimension
        val stroke = d * 0.08f
        val capW = d * 0.34f
        val capH = d * 0.52f
        drawRoundRect(
            color = fg,
            topLeft = Offset((size.width - capW) / 2f, d * 0.06f),
            size = Size(capW, capH),
            cornerRadius = CornerRadius(capW / 2f),
        )
        // 0°→180° = 下半圆（托架开口朝上）。
        drawArc(
            color = fg,
            startAngle = 0f,
            sweepAngle = 180f,
            useCenter = false,
            topLeft = Offset(d * 0.16f, d * 0.30f),
            size = Size(d * 0.68f, d * 0.52f),
            style = Stroke(stroke),
        )
        drawLine(fg, Offset(d * 0.5f, d * 0.82f), Offset(d * 0.5f, d * 0.94f), strokeWidth = stroke)
        drawLine(fg, Offset(d * 0.30f, d * 0.94f), Offset(d * 0.70f, d * 0.94f), strokeWidth = stroke)
    }
}

