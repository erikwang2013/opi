// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 秋色/冬雪/迷雾/菜地主题的道具：圆冠树、南瓜、落叶、雪人、枯树、雪花、
//! 雨伞、白菜、胡萝卜、番茄、稻草人。

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

private val TrunkBrown = Color(0xFF6B4A3A)
private val AutumnLeaf = Color(0xFFE8824E)
private val AutumnLeaf2 = Color(0xFFD95E4A)
private val PumpkinOrange = Color(0xFFE8874A)
private val PumpkinStem = Color(0xFF5B7A4A)
private val SnowWhite = Color(0xFFF7FBFF)
private val SnowShadow = Color(0xFFDDE8F2)
private val ScarfRed = Color(0xFFD96B5F)
private val HatDark = Color(0xFF4A5266)
private val CabbageGreen = Color(0xFF8FBF7A)
private val CabbageLight = Color(0xFFBFDCA8)
private val CarrotOrange = Color(0xFFE8944A)
private val TomatoRed = Color(0xFFD96B5F)
private val TomatoLeaf = Color(0xFF5B8A5A)
private val ScarecrowShirt = Color(0xFF7FA8C9)
private val StrawYellow = Color(0xFFE8C87A)

/** 圆冠树：树干 + 三团重叠叶冠；[leaf] 由主题给（秋橙/夏绿）。 */
internal fun DrawScope.drawRoundTree(
    p: ScenePalette,
    center: Offset,
    s: Float,
    leaf: Color,
    phase: Float,
    muted: Float,
) {
    val sway = sceneSway(phase, 0.25f) * s * 0.025f
    val cx = center.x + sway
    drawRoundRect(
        sceneFill(TrunkBrown, p, muted),
        Offset(center.x - s * 0.075f, center.y - s * 0.55f),
        Size(s * 0.15f, s * 0.58f),
        CornerRadius(s * 0.05f),
    )
    val fill = sceneFill(leaf, p, muted)
    drawCircle(fill, s * 0.42f, Offset(cx - s * 0.28f, center.y - s * 0.75f))
    drawCircle(fill, s * 0.46f, Offset(cx + s * 0.26f, center.y - s * 0.82f))
    drawCircle(fill, s * 0.52f, Offset(cx, center.y - s * 1.02f))
    drawCircle(sceneFill(leaf, p, (muted + 0.08f).coerceAtMost(0.8f)), s * 0.30f, Offset(cx - s * 0.02f, center.y - s * 0.86f))
}

/** 南瓜：扁圆 + 纵棱 + 绿柄。 */
internal fun DrawScope.drawPumpkin(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val bob = sceneSway(phase, 0.4f) * s * 0.02f
    val c = Offset(center.x, center.y + bob)
    val fill = sceneFill(PumpkinOrange, p, muted)
    drawOval(fill, Offset(c.x - s * 0.70f, c.y - s * 0.45f), Size(s * 1.40f, s * 0.90f))
    val edge = sceneEdge(p, 0.20f)
    for (dx in listOf(-0.35f, 0f, 0.35f)) {
        drawArc(
            edge,
            startAngle = -90f,
            sweepAngle = 180f,
            useCenter = false,
            topLeft = Offset(c.x + s * dx - s * 0.22f, c.y - s * 0.45f),
            size = Size(s * 0.44f, s * 0.90f),
            style = Stroke(s * 0.025f),
        )
    }
    drawRoundRect(sceneFill(PumpkinStem, p, muted), Offset(c.x - s * 0.06f, c.y - s * 0.62f), Size(s * 0.12f, s * 0.20f), CornerRadius(s * 0.04f))
}

/** 飘落的秋叶：位置只由相位决定。 */
internal fun DrawScope.drawFallingLeaves(
    p: ScenePalette,
    w: Float,
    h: Float,
    phase: Float,
    muted: Float,
) {
    val colors = listOf(AutumnLeaf, AutumnLeaf2, StrawYellow)
    for (i in 0 until 8) {
        val t = (phase + i * 0.129f) % 1f
        val x = ((i * 0.191f + t * 0.18f) % 1f) * w
        val y = ((i * 0.263f + t * 0.45f) % 1f) * h
        val s = (0.016f + 0.006f * (i % 3)) * minOf(w, h)
        rotate(degrees = t * 220f + i * 37f, pivot = Offset(x, y)) {
            drawOval(
                sceneFill(colors[i % colors.size], p, muted).copy(alpha = 0.75f),
                Offset(x - s, y - s * 0.35f),
                Size(s * 2f, s * 0.7f),
            )
        }
    }
}

/** 雪人：三个雪球 + 帽子 + 围巾 + 树枝手。 */
internal fun DrawScope.drawSnowman(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val bob = sceneSway(phase, 0.3f) * s * 0.015f
    val c = Offset(center.x, center.y + bob)
    val snow = sceneFill(SnowWhite, p, muted)
    drawCircle(snow, s * 0.42f, Offset(c.x, c.y - s * 0.02f))
    drawCircle(snow, s * 0.32f, Offset(c.x, c.y - s * 0.55f))
    drawCircle(snow, s * 0.24f, Offset(c.x, c.y - s * 0.98f))
    // 树枝手。
    for (dir in listOf(-1f, 1f)) {
        drawLine(sceneFill(TrunkBrown, p, muted), Offset(c.x + dir * s * 0.26f, c.y - s * 0.55f), Offset(c.x + dir * s * 0.62f, c.y - s * 0.82f), s * 0.035f, StrokeCap.Round)
    }
    // 帽子、围巾、眼睛、扣子。
    drawRect(sceneFill(HatDark, p, muted), Offset(c.x - s * 0.22f, c.y - s * 1.30f), Size(s * 0.44f, s * 0.10f))
    drawRect(sceneFill(HatDark, p, muted), Offset(c.x - s * 0.14f, c.y - s * 1.52f), Size(s * 0.28f, s * 0.24f))
    drawLine(sceneFill(ScarfRed, p, muted), Offset(c.x - s * 0.20f, c.y - s * 0.80f), Offset(c.x + s * 0.20f, c.y - s * 0.80f), s * 0.09f, StrokeCap.Round)
    drawLine(sceneFill(ScarfRed, p, muted), Offset(c.x + s * 0.12f, c.y - s * 0.80f), Offset(c.x + s * 0.24f, c.y - s * 0.55f), s * 0.08f, StrokeCap.Round)
    for (dx in listOf(-0.09f, 0.09f)) drawCircle(sceneEdge(p, 0.7f), s * 0.035f, Offset(c.x + s * dx, c.y - s * 1.02f))
    for (dy in listOf(-0.60f, -0.48f)) drawCircle(sceneEdge(p, 0.35f), s * 0.025f, Offset(c.x, c.y + s * dy))
}

/** 枯树：树干 + 分叉枝 + 枝上薄雪。 */
internal fun DrawScope.drawBareTree(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val sway = sceneSway(phase, 0.7f) * s * 0.02f
    val wood = sceneFill(TrunkBrown, p, muted)
    drawLine(wood, Offset(center.x, center.y), Offset(center.x + sway, center.y - s * 0.95f), s * 0.10f, StrokeCap.Round)
    for ((dx, dy, len) in listOf(
        Triple(-1f, 0.55f, 0.40f), Triple(1f, 0.68f, 0.45f),
        Triple(-1f, 0.80f, 0.30f), Triple(1f, 0.88f, 0.26f),
    )) {
        val sx = center.x + sway * dy
        val sy = center.y - s * dy
        drawLine(wood, Offset(sx, sy), Offset(sx + dx * s * len, sy - s * len * 0.55f), s * 0.045f, StrokeCap.Round)
    }
    // 枝上薄雪。
    drawLine(sceneFill(SnowWhite, p, muted), Offset(center.x - s * 0.04f, center.y - s * 0.92f), Offset(center.x + sway - s * 0.25f, center.y - s * 1.12f), s * 0.035f, StrokeCap.Round)
}

/** 雪花：小十字/圆点，向下飘。 */
internal fun DrawScope.drawSnowflakes(
    p: ScenePalette,
    w: Float,
    h: Float,
    phase: Float,
    muted: Float,
) {
    val color = sceneFill(SnowWhite, p, muted)
    for (i in 0 until 18) {
        val t = (phase * 0.8f + i * 0.093f) % 1f
        val x = ((i * 0.173f + sin(t * PI.toFloat() * 2f) * 0.02f) % 1f) * w
        val y = t * h
        val r = (0.008f + 0.006f * (i % 4)) * minOf(w, h)
        val a = 0.35f + 0.55f * abs(sin((phase + i * 0.2f) * SCENE_TAU))
        if (i % 3 == 0) {
            drawLine(color.copy(alpha = a), Offset(x - r, y), Offset(x + r, y), r * 0.7f, StrokeCap.Round)
            drawLine(color.copy(alpha = a), Offset(x, y - r), Offset(x, y + r), r * 0.7f, StrokeCap.Round)
        } else {
            drawCircle(color.copy(alpha = a), r * 0.7f, Offset(x, y))
        }
    }
}

/** 雨伞：伞面 + 伞骨 + 弯柄。 */
internal fun DrawScope.drawUmbrella(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val tilt = sceneSway(phase, 0.2f) * 5f
    rotate(degrees = tilt, pivot = center) {
        val canopy = sceneFill(Color(0xFF8FA9C4), p, muted)
        drawArc(canopy, startAngle = 180f, sweepAngle = 180f, useCenter = true, topLeft = Offset(center.x - s, center.y - s * 0.55f), size = Size(s * 2f, s * 1.1f))
        val edge = sceneEdge(p, 0.45f)
        for (dx in listOf(-0.6f, -0.2f, 0.2f, 0.6f)) {
            drawLine(edge, Offset(center.x, center.y - s * 0.55f), Offset(center.x + s * dx, center.y), s * 0.025f, StrokeCap.Round)
        }
        drawLine(edge, Offset(center.x, center.y), Offset(center.x, center.y + s * 0.75f), s * 0.045f, StrokeCap.Round)
        drawArc(edge, startAngle = 0f, sweepAngle = 180f, useCenter = false, topLeft = Offset(center.x - s * 0.20f, center.y + s * 0.60f), size = Size(s * 0.40f, s * 0.30f), style = Stroke(s * 0.045f, cap = StrokeCap.Round))
    }
}

/** 白菜：层层包心的圆叶。 */
internal fun DrawScope.drawCabbage(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val bob = sceneSway(phase, 0.5f) * s * 0.015f
    val c = Offset(center.x, center.y + bob)
    drawOval(sceneFill(CabbageGreen, p, muted), Offset(c.x - s * 0.62f, c.y - s * 0.55f), Size(s * 1.24f, s * 1.10f))
    drawOval(sceneFill(CabbageLight, p, muted), Offset(c.x - s * 0.38f, c.y - s * 0.40f), Size(s * 0.76f, s * 0.80f))
    drawOval(sceneFill(CabbageGreen, p, muted).copy(alpha = 0.8f), Offset(c.x - s * 0.18f, c.y - s * 0.26f), Size(s * 0.36f, s * 0.52f))
}

/** 胡萝卜：橙色锥形 + 绿缨。 */
internal fun DrawScope.drawCarrot(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val root = Path().apply {
        moveTo(center.x - s * 0.22f, center.y - s * 0.45f)
        lineTo(center.x + s * 0.22f, center.y - s * 0.45f)
        lineTo(center.x, center.y + s * 0.55f)
        close()
    }
    drawPath(root, sceneFill(CarrotOrange, p, muted))
    for (dx in listOf(-0.12f, 0f, 0.12f)) {
        drawLine(sceneFill(TomatoLeaf, p, muted), Offset(center.x + s * dx, center.y - s * 0.45f), Offset(center.x + s * dx * 1.4f, center.y - s * 0.82f), s * 0.05f, StrokeCap.Round)
    }
}

/** 番茄：茎叶 + 几颗红果。 */
internal fun DrawScope.drawTomatoPlant(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val green = sceneFill(TomatoLeaf, p, muted)
    drawLine(green, Offset(center.x, center.y), Offset(center.x, center.y - s * 0.95f), s * 0.045f, StrokeCap.Round)
    for (dir in listOf(-1f, 1f)) {
        drawOval(green, Offset(center.x + dir * s * 0.12f - s * 0.20f, center.y - s * 0.55f), Size(s * 0.40f, s * 0.18f))
        drawCircle(sceneFill(TomatoRed, p, muted), s * 0.16f, Offset(center.x + dir * s * 0.26f, center.y - s * 0.48f))
    }
    drawCircle(sceneFill(TomatoRed, p, muted), s * 0.14f, Offset(center.x, center.y - s * 0.78f))
}

/** 稻草人：十字架 + 衣服 + 草帽 + 笑脸。 */
internal fun DrawScope.drawScarecrow(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val sway = sceneSway(phase, 0.4f) * s * 0.02f
    val c = Offset(center.x + sway, center.y)
    val wood = sceneFill(TrunkBrown, p, muted)
    drawLine(wood, Offset(c.x, c.y), Offset(c.x, c.y - s * 1.30f), s * 0.06f, StrokeCap.Round)
    drawLine(wood, Offset(c.x - s * 0.55f, c.y - s * 0.92f), Offset(c.x + s * 0.55f, c.y - s * 0.92f), s * 0.05f, StrokeCap.Round)
    drawRoundRect(sceneFill(ScarecrowShirt, p, muted), Offset(c.x - s * 0.32f, c.y - s * 1.00f), Size(s * 0.64f, s * 0.55f), CornerRadius(s * 0.08f))
    drawCircle(sceneFill(StrawYellow, p, muted), s * 0.22f, Offset(c.x, c.y - s * 1.16f))
    drawRect(sceneFill(StrawYellow, p, muted), Offset(c.x - s * 0.34f, c.y - s * 1.34f), Size(s * 0.68f, s * 0.08f))
    drawRect(sceneFill(StrawYellow, p, muted), Offset(c.x - s * 0.18f, c.y - s * 1.52f), Size(s * 0.36f, s * 0.20f))
    for (dx in listOf(-0.08f, 0.08f)) drawCircle(sceneEdge(p, 0.65f), s * 0.03f, Offset(c.x + s * dx, c.y - s * 1.18f))
    drawArc(sceneEdge(p, 0.55f), startAngle = 20f, sweepAngle = 140f, useCenter = false, topLeft = Offset(c.x - s * 0.08f, c.y - s * 1.14f), size = Size(s * 0.16f, s * 0.12f), style = Stroke(s * 0.025f, cap = StrokeCap.Round))
}
