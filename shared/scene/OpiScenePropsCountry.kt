// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 乡村/狗群/路灯主题的道具：农舍、风车、庄稼垄、狗、球、长椅。

package xyz.erik.opi.scene

import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import kotlin.math.PI
import kotlin.math.sin

private val FarmCream = Color(0xFFF2E3C8)
private val RoofRed = Color(0xFFC96B5A)
private val WindmillWood = Color(0xFFB58A5A)
private val CropGreen = Color(0xFF7FAE6A)
private val DogBrown = Color(0xFFB58A5A)
private val DogCream = Color(0xFFF2E3C8)
private val BallRed = Color(0xFFD96B5F)
private val BenchWood = Color(0xFF8A6A4F)

/** 农舍：奶白墙 + 红屋顶 + 门窗 + 烟囱。 */
internal fun DrawScope.drawFarmhouse(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val body = sceneFill(FarmCream, p, muted)
    val roof = sceneFill(RoofRed, p, muted)
    drawRect(body, Offset(center.x - s * 0.65f, center.y - s * 0.55f), Size(s * 1.30f, s * 0.55f))
    drawPath(
        Path().apply {
            moveTo(center.x - s * 0.80f, center.y - s * 0.55f)
            lineTo(center.x, center.y - s * 0.98f)
            lineTo(center.x + s * 0.80f, center.y - s * 0.55f)
            close()
        },
        roof,
    )
    drawRect(sceneFill(WindmillWood, p, muted), Offset(center.x - s * 0.12f, center.y - s * 0.32f), Size(s * 0.24f, s * 0.32f))
    drawRect(sceneFill(Color(0xFFBFE4EF), p, muted), Offset(center.x - s * 0.48f, center.y - s * 0.42f), Size(s * 0.18f, s * 0.16f))
    drawRect(sceneFill(Color(0xFFBFE4EF), p, muted), Offset(center.x + s * 0.30f, center.y - s * 0.42f), Size(s * 0.18f, s * 0.16f))
    drawRect(sceneFill(WindmillWood, p, muted), Offset(center.x + s * 0.45f, center.y - s * 0.95f), Size(s * 0.12f, s * 0.28f))
    // 炊烟。
    val smoke = sceneFill(Color.White, p, (muted + 0.2f).coerceAtMost(0.85f)).copy(alpha = 0.45f)
    for (i in 0..1) {
        val drift = sin((phase + i * 0.3f) * SCENE_TAU) * s * 0.05f
        drawCircle(smoke, s * (0.06f + i * 0.03f), Offset(center.x + s * 0.51f + drift, center.y - s * (1.02f + i * 0.18f)))
    }
}

/** 风车：塔身 + 四片叶，叶随相位转。 */
internal fun DrawScope.drawWindmill(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val wood = sceneFill(WindmillWood, p, muted)
    drawPath(
        Path().apply {
            moveTo(center.x - s * 0.30f, center.y)
            lineTo(center.x - s * 0.18f, center.y - s * 1.10f)
            lineTo(center.x + s * 0.18f, center.y - s * 1.10f)
            lineTo(center.x + s * 0.30f, center.y)
            close()
        },
        wood,
    )
    drawCircle(sceneFill(RoofRed, p, muted), s * 0.22f, Offset(center.x, center.y - s * 1.10f))
    rotate(degrees = phase * 360f, pivot = Offset(center.x, center.y - s * 1.10f)) {
        for (i in 0..3) {
            rotate(degrees = i * 90f, pivot = Offset(center.x, center.y - s * 1.10f)) {
                drawRect(wood, Offset(center.x - s * 0.035f, center.y - s * 1.95f), Size(s * 0.07f, s * 0.85f))
                drawRect(sceneFill(FarmCream, p, muted), Offset(center.x + s * 0.02f, center.y - s * 1.90f), Size(s * 0.28f, s * 0.55f))
            }
        }
    }
    drawCircle(sceneFill(WindmillWood, p, muted), s * 0.07f, Offset(center.x, center.y - s * 1.10f))
}

/** 庄稼垄：一排排小苗。 */
internal fun DrawScope.drawCropRows(
    p: ScenePalette,
    left: Float,
    right: Float,
    y: Float,
    h: Float,
    phase: Float,
    muted: Float,
) {
    val green = sceneFill(CropGreen, p, muted)
    for (row in 0..2) {
        val yy = y + row * h * 0.30f
        var x = left
        while (x < right) {
            val sway = sin((phase + x * 0.01f + row * 0.2f) * SCENE_TAU) * h * 0.05f
            drawLine(green, Offset(x, yy), Offset(x + sway, yy - h * 0.22f), h * 0.035f, StrokeCap.Round)
            drawOval(green, Offset(x + sway - h * 0.05f, yy - h * 0.30f), Size(h * 0.10f, h * 0.12f))
            x += h * 0.22f
        }
    }
}

/** 狗：身体 + 头 + 耳朵（垂/尖）+ 尾巴 + 腿；[floppy] 控制耳形。 */
internal fun DrawScope.drawDogFigure(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
    fillColor: Color = DogBrown,
    floppy: Boolean = true,
) {
    val fill = sceneFill(fillColor, p, muted)
    val cream = sceneFill(DogCream, p, muted)
    val edge = sceneEdge(p, 0.42f)
    val wag = sceneSway(phase, 0.3f) * 0.20f
    val c = Offset(center.x, center.y)
    // 尾巴。
    drawPath(
        Path().apply {
            moveTo(c.x + s * 0.42f, c.y - s * 0.05f)
            quadraticTo(c.x + s * 0.70f, c.y - s * (0.25f + wag), c.x + s * 0.85f, c.y - s * 0.55f)
        },
        edge,
        style = Stroke(s * 0.08f, cap = StrokeCap.Round),
    )
    // 身体/腿。
    drawOval(fill, Offset(c.x - s * 0.48f, c.y - s * 0.18f), Size(s * 0.90f, s * 0.52f))
    for (dx in listOf(-0.30f, -0.08f, 0.14f, 0.34f)) {
        drawLine(edge, Offset(c.x + s * dx, c.y + s * 0.20f), Offset(c.x + s * dx, c.y + s * 0.48f), s * 0.055f, StrokeCap.Round)
    }
    // 头。
    val head = Offset(c.x - s * 0.48f, c.y - s * 0.34f)
    drawCircle(fill, s * 0.26f, head)
    drawOval(cream, Offset(head.x - s * 0.16f, head.y + s * 0.02f), Size(s * 0.32f, s * 0.22f))
    for (dir in listOf(-1f, 1f)) {
        if (floppy) {
            drawOval(fill, Offset(head.x + dir * s * 0.20f - s * 0.10f, head.y - s * 0.18f), Size(s * 0.20f, s * 0.38f))
        } else {
            drawPath(
                Path().apply {
                    moveTo(head.x + dir * s * 0.10f, head.y - s * 0.20f)
                    lineTo(head.x + dir * s * 0.22f, head.y - s * 0.52f)
                    lineTo(head.x + dir * s * 0.34f, head.y - s * 0.18f)
                    close()
                },
                fill,
            )
        }
    }
    for (dir in listOf(-1f, 1f)) drawCircle(edge, s * 0.028f, Offset(head.x + dir * s * 0.10f, head.y - s * 0.06f))
    drawCircle(edge, s * 0.05f, Offset(head.x, head.y + s * 0.10f))
    // 舌头。
    drawOval(sceneFill(Color(0xFFF2A7B3), p, muted), Offset(head.x - s * 0.035f, head.y + s * 0.18f), Size(s * 0.07f, s * 0.10f))
}

/** 球：圆 + 两道弧。 */
internal fun DrawScope.drawBall(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val bob = sceneSway(phase, 0.5f) * s * 0.05f
    val c = Offset(center.x, center.y + bob)
    drawCircle(sceneFill(BallRed, p, muted), s, c)
    drawArc(sceneEdge(p, 0.25f), startAngle = -30f, sweepAngle = 240f, useCenter = false, topLeft = Offset(c.x - s * 0.8f, c.y - s * 0.8f), size = Size(s * 1.6f, s * 1.6f), style = Stroke(s * 0.06f))
    drawArc(sceneEdge(p, 0.25f), startAngle = 150f, sweepAngle = 240f, useCenter = false, topLeft = Offset(c.x - s * 0.8f, c.y - s * 0.8f), size = Size(s * 1.6f, s * 1.6f), style = Stroke(s * 0.06f))
}

/** 长椅：座板 + 靠背 + 腿。 */
internal fun DrawScope.drawBench(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val wood = sceneFill(BenchWood, p, muted)
    drawRoundRect(wood, Offset(center.x - s * 0.70f, center.y - s * 0.05f), Size(s * 1.40f, s * 0.12f), CornerRadius(s * 0.03f))
    drawRoundRect(wood, Offset(center.x - s * 0.70f, center.y - s * 0.45f), Size(s * 1.40f, s * 0.10f), CornerRadius(s * 0.03f))
    for (dir in listOf(-1f, 1f)) {
        drawLine(wood, Offset(center.x + dir * s * 0.55f, center.y), Offset(center.x + dir * s * 0.55f, center.y + s * 0.40f), s * 0.07f, StrokeCap.Round)
        drawLine(wood, Offset(center.x + dir * s * 0.55f, center.y - s * 0.40f), Offset(center.x + dir * s * 0.55f, center.y), s * 0.06f, StrokeCap.Round)
    }
}
