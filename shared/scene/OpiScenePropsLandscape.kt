// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 山川/大海/池塘/迷雾主题的道具：山脊、太阳、海浪、帆船、灯塔、贝壳、荷花、
//! 芦苇、蜻蜓、鸭子、雾带。基础绘图函数仍在 OpiScene.kt/Props.kt。

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
import kotlin.math.abs
import kotlin.math.sin

private val MountainBack = Color(0xFF8FA9C4)
private val MountainFront = Color(0xFF5C7896)
private val MountainSnow = Color(0xFFF2F7FC)
private val SunWarm = Color(0xFFF6C85F)
private val SeaBlue = Color(0xFF5FA8C9)
private val SeaDeep = Color(0xFF3C7FA6)
private val SailWhite = Color(0xFFF8F4EA)
private val SailRed = Color(0xFFE8826A)
private val LighthouseWhite = Color(0xFFF4F0E6)
private val LighthouseRed = Color(0xFFD96B5F)
private val ShellPink = Color(0xFFF0C3B6)
private val LotusPink = Color(0xFFF2A7B3)
private val LotusGreen = Color(0xFF6FAF7A)
private val DragonflyBlue = Color(0xFF8FC7D9)
private val DuckCream = Color(0xFFF6E3B8)
private val DuckOrange = Color(0xFFE8A25A)

/** 三层山脊 + 雪顶；[left] 是整段山脊的左沿（条外岛用它把山对准锚点）。 */
internal fun DrawScope.drawMountains(
    p: ScenePalette,
    baseY: Float,
    width: Float,
    height: Float,
    muted: Float,
    left: Float = 0f,
) {
    fun ridge(color: Color, peaks: List<Pair<Float, Float>>) {
        val path = Path().apply {
            moveTo(left, baseY)
            for ((fx, fy) in peaks) lineTo(left + width * fx, baseY - height * fy)
            lineTo(left + width, baseY)
            close()
        }
        drawPath(path, color)
    }
    ridge(sceneFill(MountainBack, p, muted), listOf(0.18f to 0.55f, 0.48f to 0.95f, 0.82f to 0.48f))
    ridge(sceneFill(MountainFront, p, muted), listOf(0.02f to 0.35f, 0.30f to 0.70f, 0.62f to 0.52f, 0.90f to 0.30f))
    // 雪顶：主峰顶上一小块白。
    for ((fx, fy) in listOf(0.48f to 0.95f, 0.30f to 0.70f)) {
        val top = baseY - height * fy
        val half = width * 0.045f
        val snow = Path().apply {
            moveTo(left + width * fx, top)
            lineTo(left + width * fx - half, top + height * 0.12f)
            lineTo(left + width * fx - half * 0.25f, top + height * 0.08f)
            lineTo(left + width * fx + half * 0.3f, top + height * 0.13f)
            lineTo(left + width * fx + half, top + height * 0.10f)
            close()
        }
        drawPath(snow, sceneFill(MountainSnow, p, muted))
    }
}

/** 太阳：圆盘 + 八条短光。 */
internal fun DrawScope.drawSun(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val pulse = 0.85f + 0.15f * sceneSway(phase, 0.2f)
    drawCircle(sceneFill(SunWarm, p, muted).copy(alpha = 0.14f), s * 1.5f, center)
    drawCircle(sceneFill(SunWarm, p, muted), s, center)
    val ray = sceneFill(SunWarm, p, muted).copy(alpha = 0.75f)
    for (i in 0 until 8) {
        val a = i * PI.toFloat() / 4f
        val r0 = s * 1.35f * pulse
        val r1 = s * 1.7f * pulse
        drawLine(
            ray,
            Offset(center.x + sin(a) * r0, center.y - kotlin.math.cos(a) * r0),
            Offset(center.x + sin(a) * r1, center.y - kotlin.math.cos(a) * r1),
            s * 0.07f,
            StrokeCap.Round,
        )
    }
}

/** 三行海浪；[y] 是第一行基线，[h] 是波高。 */
internal fun DrawScope.drawWaves(
    p: ScenePalette,
    left: Float,
    right: Float,
    y: Float,
    h: Float,
    phase: Float,
    muted: Float,
) {
    for (row in 0 until 3) {
        val color = sceneFill(if (row % 2 == 0) SeaBlue else SeaDeep, p, muted)
            .copy(alpha = 0.55f + row * 0.12f)
        val yy = y + row * h * 0.45f
        val path = Path().apply {
            moveTo(left, yy)
            var x = left
            var i = 0
            while (x < right) {
                val up = sin((phase + i * 0.11f + row * 0.2f) * SCENE_TAU) * h * 0.18f
                quadraticTo(x + h * 0.30f, yy - h * 0.22f + up, x + h * 0.60f, yy)
                x += h * 0.60f
                i++
            }
        }
        drawPath(path, color, style = Stroke(h * 0.055f, cap = StrokeCap.Round))
    }
}

/** 帆船：船身 + 桅杆 + 两片帆；随波轻晃。 */
internal fun DrawScope.drawSailboat(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val rock = sceneSway(phase, 0.3f) * 4f
    rotate(degrees = rock, pivot = center) {
        val hull = Path().apply {
            moveTo(center.x - s * 0.55f, center.y)
            lineTo(center.x + s * 0.55f, center.y)
            lineTo(center.x + s * 0.32f, center.y + s * 0.26f)
            lineTo(center.x - s * 0.32f, center.y + s * 0.26f)
            close()
        }
        drawPath(hull, sceneFill(SailRed, p, muted))
        drawLine(
            sceneFill(Color(0xFF6B4A3A), p, muted),
            Offset(center.x, center.y),
            Offset(center.x, center.y - s * 0.95f),
            s * 0.045f,
            StrokeCap.Round,
        )
        val sail = sceneFill(SailWhite, p, muted)
        drawPath(
            Path().apply {
                moveTo(center.x + s * 0.04f, center.y - s * 0.90f)
                lineTo(center.x + s * 0.45f, center.y - s * 0.10f)
                lineTo(center.x + s * 0.04f, center.y - s * 0.10f)
                close()
            },
            sail,
        )
        drawPath(
            Path().apply {
                moveTo(center.x - s * 0.04f, center.y - s * 0.80f)
                lineTo(center.x - s * 0.36f, center.y - s * 0.10f)
                lineTo(center.x - s * 0.04f, center.y - s * 0.10f)
                close()
            },
            sail.copy(alpha = 0.85f),
        )
        drawPath(
            Path().apply {
                moveTo(center.x, center.y - s * 0.95f)
                lineTo(center.x + s * 0.16f, center.y - s * 0.86f)
                lineTo(center.x, center.y - s * 0.78f)
                close()
            },
            sceneFill(SunWarm, p, muted),
        )
    }
}

/** 灯塔：条纹塔身 + 灯室 + 两束光。 */
internal fun DrawScope.drawLighthouse(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val glow = 0.55f + 0.45f * abs(sin(phase * SCENE_TAU))
    drawOval(sceneFill(SeaDeep, p, muted), Offset(center.x - s * 0.85f, center.y - s * 0.10f), Size(s * 1.7f, s * 0.35f))
    val tower = Path().apply {
        moveTo(center.x - s * 0.28f, center.y)
        lineTo(center.x - s * 0.16f, center.y - s * 1.20f)
        lineTo(center.x + s * 0.16f, center.y - s * 1.20f)
        lineTo(center.x + s * 0.28f, center.y)
        close()
    }
    drawPath(tower, sceneFill(LighthouseWhite, p, muted))
    drawPath(tower, sceneEdge(p, 0.30f), style = Stroke(s * 0.03f))
    for (y in listOf(0.35f, 0.70f)) {
        drawLine(sceneFill(LighthouseRed, p, muted), Offset(center.x - s * 0.24f, center.y - s * y), Offset(center.x + s * 0.24f, center.y - s * y), s * 0.11f)
    }
    drawRect(sceneFill(LighthouseWhite, p, muted), Offset(center.x - s * 0.20f, center.y - s * 1.34f), Size(s * 0.40f, s * 0.16f))
    drawRect(sceneFill(SunWarm, p, muted), Offset(center.x - s * 0.12f, center.y - s * 1.30f), Size(s * 0.24f, s * 0.10f))
    // 光束。
    val beam = sceneFill(SunWarm, p, muted).copy(alpha = 0.16f * glow)
    drawPath(
        Path().apply {
            moveTo(center.x, center.y - s * 1.28f)
            lineTo(center.x - s * 1.2f, center.y - s * 1.50f)
            lineTo(center.x - s * 1.2f, center.y - s * 1.10f)
            close()
        },
        beam,
    )
    drawPath(
        Path().apply {
            moveTo(center.x, center.y - s * 1.28f)
            lineTo(center.x + s * 1.2f, center.y - s * 1.50f)
            lineTo(center.x + s * 1.2f, center.y - s * 1.10f)
            close()
        },
        beam,
    )
}

/** 贝壳：扇形 + 放射线。 */
internal fun DrawScope.drawShell(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val fill = sceneFill(ShellPink, p, muted)
    rotate(degrees = sceneSway(phase, 0.5f) * 8f, pivot = center) {
        drawArc(fill, startAngle = 180f, sweepAngle = 180f, useCenter = true, topLeft = Offset(center.x - s, center.y - s), size = Size(s * 2f, s * 2f))
        val edge = sceneEdge(p, 0.30f)
        for (i in 0..4) {
            val a = PI.toFloat() * (1f + i / 4f)
            drawLine(edge, Offset(center.x, center.y), Offset(center.x + sin(a) * s, center.y + kotlin.math.cos(a) * s * -1f), s * 0.035f, StrokeCap.Round)
        }
    }
}

/** 荷花：荷叶 + 五瓣花 + 花心。 */
internal fun DrawScope.drawLotus(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    drawOval(sceneFill(LotusGreen, p, muted), Offset(center.x - s * 0.70f, center.y - s * 0.10f), Size(s * 1.40f, s * 0.40f))
    drawOval(sceneEdge(p, 0.22f), Offset(center.x - s * 0.70f, center.y - s * 0.10f), Size(s * 1.40f, s * 0.40f), style = Stroke(s * 0.02f))
    drawOval(sceneFill(LotusGreen, p, muted).copy(alpha = 0.7f), Offset(center.x + s * 0.10f, center.y - s * 0.02f), Size(s * 0.70f, s * 0.26f))
    // 花茎：从水面/荷叶连到花托，不再让花悬空。
    drawLine(sceneFill(LotusGreen, p, muted), Offset(center.x, center.y), Offset(center.x, center.y - s * 0.34f), s * 0.05f, StrokeCap.Round)
    val petal = sceneFill(LotusPink, p, muted)
    for (i in 0..4) {
        rotate(degrees = i * 72f + sceneSway(phase, 0.2f) * 4f, pivot = center) {
            drawOval(petal, Offset(center.x - s * 0.16f, center.y - s * 0.62f), Size(s * 0.32f, s * 0.62f))
            drawOval(sceneEdge(p, 0.22f), Offset(center.x - s * 0.16f, center.y - s * 0.62f), Size(s * 0.32f, s * 0.62f), style = Stroke(s * 0.015f))
        }
    }
    drawCircle(sceneFill(SunWarm, p, muted), s * 0.16f, Offset(center.x, center.y - s * 0.30f))
}

/** 芦苇：三根茎 + 蒲棒。 */
internal fun DrawScope.drawReed(
    p: ScenePalette,
    base: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val green = sceneFill(LotusGreen, p, muted)
    val brown = sceneFill(Color(0xFF8A6A4F), p, muted)
    for (i in 0..2) {
        val lean = sceneSway(phase, 0.15f * i) * s * 0.10f
        val top = Offset(base.x + (i - 1) * s * 0.22f + lean, base.y - s * (0.75f + i * 0.12f))
        drawLine(green, Offset(base.x + (i - 1) * s * 0.22f, base.y), top, s * 0.035f, StrokeCap.Round)
        drawRoundRect(brown, Offset(top.x - s * 0.05f, top.y - s * 0.18f), Size(s * 0.10f, s * 0.24f), CornerRadius(s * 0.05f))
    }
}

/** 蜻蜓：细长身体 + 四片翅膀。 */
internal fun DrawScope.drawDragonfly(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val flap = 0.55f + 0.45f * abs(sin(phase * SCENE_TAU * 3f))
    val wing = sceneFill(DragonflyBlue, p, muted).copy(alpha = 0.55f)
    for (dir in listOf(-1f, 1f)) {
        drawOval(wing, Offset(center.x + dir * s * 0.08f - s * 0.22f, center.y - s * 0.30f), Size(s * 0.40f * flap + s * 0.08f, s * 0.16f))
        drawOval(wing, Offset(center.x + dir * s * 0.08f - s * 0.20f, center.y - s * 0.04f), Size(s * 0.36f * flap + s * 0.06f, s * 0.14f))
    }
    drawLine(sceneEdge(p, 0.55f), Offset(center.x, center.y - s * 0.34f), Offset(center.x, center.y + s * 0.34f), s * 0.06f, StrokeCap.Round)
    drawCircle(sceneEdge(p, 0.55f), s * 0.09f, Offset(center.x, center.y - s * 0.38f))
}

/** 鸭子：身体 + 头 + 橙嘴。 */
internal fun DrawScope.drawDuck(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val bob = sceneSway(phase, 0.4f) * s * 0.03f
    val c = Offset(center.x, center.y + bob)
    val body = sceneFill(DuckCream, p, muted)
    drawOval(body, Offset(c.x - s * 0.55f, c.y - s * 0.10f), Size(s * 1.10f, s * 0.50f))
    drawCircle(body, s * 0.24f, Offset(c.x + s * 0.38f, c.y - s * 0.30f))
    drawPath(
        Path().apply {
            moveTo(c.x + s * 0.55f, c.y - s * 0.32f)
            lineTo(c.x + s * 0.78f, c.y - s * 0.25f)
            lineTo(c.x + s * 0.55f, c.y - s * 0.18f)
            close()
        },
        sceneFill(DuckOrange, p, muted),
    )
    drawCircle(sceneEdge(p, 0.55f), s * 0.035f, Offset(c.x + s * 0.44f, c.y - s * 0.33f))
    drawOval(sceneFill(DuckCream, p, muted).copy(alpha = 0.9f), Offset(c.x - s * 0.25f, c.y - s * 0.04f), Size(s * 0.40f, s * 0.26f))
}

/** 雾带：三条半透明横向云带，缓慢漂移。 */
internal fun DrawScope.drawFog(
    p: ScenePalette,
    left: Float,
    right: Float,
    y: Float,
    h: Float,
    phase: Float,
    muted: Float,
) {
    val base = sceneFill(Color.White, p, (muted + 0.25f).coerceAtMost(0.9f))
    for (i in 0..2) {
        val drift = sin((phase + i * 0.2f) * SCENE_TAU) * h * 0.12f
        val yy = y + i * h * 0.28f
        drawRoundRect(
            base.copy(alpha = 0.22f - i * 0.04f),
            Offset(left - h * 0.4f + drift, yy),
            Size((right - left) * 1.25f, h * 0.30f),
            CornerRadius(h * 0.15f),
        )
    }
}
