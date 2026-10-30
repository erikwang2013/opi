// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 书房/灶台/猫群主题的道具：窗、书架、书桌、摊开的书、台灯、灶台、锅、蒸汽、
//! 罐子、小猫、毛线球、纸箱、坐垫。

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

private val WoodWarm = Color(0xFFB58A5A)
private val WoodDark = Color(0xFF8A6A4F)
private val PaperCream = Color(0xFFF7F1E3)
private val GlassDay = Color(0xFFCDE8F2)
private val GlassNight = Color(0xFF3A4468)
private val LampWarm = Color(0xFFFFE3A0)
private val Copper = Color(0xFFC97B4A)
private val TileBlue = Color(0xFF9FBBC4)
private val JarGlass = Color(0xFFDFF0F2)
private val BookColors = listOf(
    Color(0xFFD96B5F), Color(0xFF7FA8C9), Color(0xFFE8C87A),
    Color(0xFF8FBF7A), Color(0xFFB98BC9), Color(0xFFE8826A),
)
private val Cardboard = Color(0xFFC9A26B)
private val YarnPink = Color(0xFFF2A7B3)

/** 窗：木框 + 玻璃 + 十字棂 + 窗台；夜晚玻璃里有月亮/星星。 */
internal fun DrawScope.drawWindow(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
    night: Boolean = false,
) {
    val w = s * 1.4f
    val h = s * 1.6f
    val left = center.x - w / 2f
    val top = center.y - h / 2f
    drawRoundRect(sceneFill(WoodWarm, p, muted), Offset(left, top), Size(w, h), CornerRadius(s * 0.08f))
    drawRect(sceneFill(if (night) GlassNight else GlassDay, p, muted), Offset(left + s * 0.10f, top + s * 0.10f), Size(w - s * 0.20f, h - s * 0.20f))
    if (night) {
        drawCircle(sceneFill(LampWarm, p, muted), s * 0.14f, Offset(center.x + s * 0.28f, top + s * 0.38f))
        for ((dx, dy) in listOf(-0.30f to 0.30f, 0.02f to 0.16f, 0.34f to 0.62f)) {
            drawCircle(Color.White.copy(alpha = 0.75f), s * 0.025f, Offset(center.x + s * dx, top + s * dy))
        }
    } else {
        drawCircle(sceneFill(LampWarm, p, muted), s * 0.16f, Offset(left + w * 0.68f, top + h * 0.28f))
    }
    val frame = sceneFill(WoodDark, p, muted)
    drawLine(frame, Offset(center.x, top + s * 0.10f), Offset(center.x, top + h - s * 0.10f), s * 0.06f)
    drawLine(frame, Offset(left + s * 0.10f, center.y), Offset(left + w - s * 0.10f, center.y), s * 0.06f)
    drawRect(sceneFill(WoodDark, p, muted), Offset(left - s * 0.08f, top + h), Size(w + s * 0.16f, s * 0.10f))
    // 窗帘。
    val curtain = sceneFill(Color(0xFFE8C8C0), p, muted)
    drawPath(
        Path().apply {
            moveTo(left - s * 0.02f, top)
            quadraticTo(left + s * 0.18f, top + h * 0.55f, left - s * 0.02f, top + h)
            close()
        },
        curtain,
    )
    drawPath(
        Path().apply {
            moveTo(left + w + s * 0.02f, top)
            quadraticTo(left + w - s * 0.18f, top + h * 0.55f, left + w + s * 0.02f, top + h)
            close()
        },
        curtain,
    )
}

/** 书架：木框 + 三层书 + 顶上小盆栽。 */
internal fun DrawScope.drawBookshelf(
    p: ScenePalette,
    left: Float,
    baseY: Float,
    width: Float,
    height: Float,
    phase: Float,
    muted: Float,
) {
    val frame = sceneFill(WoodWarm, p, muted)
    val top = baseY - height
    drawRoundRect(frame, Offset(left, top), Size(width, height), CornerRadius(width * 0.03f))
    drawRect(sceneFill(WoodDark, p, muted), Offset(left + width * 0.05f, top + height * 0.05f), Size(width * 0.90f, height * 0.90f))
    val shelfH = height * 0.28f
    for (row in 0..2) {
        val sy = top + height * 0.08f + row * shelfH
        drawRect(frame, Offset(left + width * 0.05f, sy + shelfH * 0.88f), Size(width * 0.90f, height * 0.035f))
        var bx = left + width * 0.09f
        for (i in 0 until 5) {
            val bw = width * (0.10f + 0.02f * ((row + i) % 3))
            val bh = shelfH * (0.55f + 0.10f * ((i + row) % 3))
            drawRect(sceneFill(BookColors[(row * 5 + i) % BookColors.size], p, muted), Offset(bx, sy + shelfH * 0.88f - bh), Size(bw, bh))
            bx += bw + width * 0.025f
        }
    }
    drawOval(sceneFill(Color(0xFF8FBF7A), p, muted), Offset(left + width * 0.78f, top - height * 0.08f), Size(width * 0.16f, height * 0.10f))
}

/** 书桌：桌面 + 两条腿 + 抽屉。 */
internal fun DrawScope.drawDesk(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val top = sceneFill(WoodWarm, p, muted)
    drawRoundRect(top, Offset(center.x - s * 0.75f, center.y - s * 0.08f), Size(s * 1.5f, s * 0.12f), CornerRadius(s * 0.03f))
    for (dir in listOf(-1f, 1f)) {
        drawRect(sceneFill(WoodDark, p, muted), Offset(center.x + dir * s * 0.62f - s * 0.04f, center.y + s * 0.04f), Size(s * 0.08f, s * 0.55f))
    }
    drawRect(sceneFill(WoodDark, p, muted), Offset(center.x - s * 0.45f, center.y + s * 0.04f), Size(s * 0.90f, s * 0.20f))
    drawCircle(sceneFill(LampWarm, p, muted), s * 0.03f, Offset(center.x, center.y + s * 0.14f))
}

/** 摊开的书：两页 + 中缝 + 几行字。 */
internal fun DrawScope.drawOpenBook(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val page = sceneFill(PaperCream, p, muted)
    val edge = sceneEdge(p, 0.25f)
    drawPath(
        Path().apply {
            moveTo(center.x, center.y - s * 0.18f)
            quadraticTo(center.x - s * 0.35f, center.y - s * 0.28f, center.x - s * 0.55f, center.y - s * 0.12f)
            lineTo(center.x - s * 0.50f, center.y + s * 0.18f)
            quadraticTo(center.x - s * 0.28f, center.y + s * 0.05f, center.x, center.y + s * 0.12f)
            close()
        },
        page,
    )
    drawPath(
        Path().apply {
            moveTo(center.x, center.y - s * 0.18f)
            quadraticTo(center.x + s * 0.35f, center.y - s * 0.28f, center.x + s * 0.55f, center.y - s * 0.12f)
            lineTo(center.x + s * 0.50f, center.y + s * 0.18f)
            quadraticTo(center.x + s * 0.28f, center.y + s * 0.05f, center.x, center.y + s * 0.12f)
            close()
        },
        page,
    )
    drawLine(edge, Offset(center.x, center.y - s * 0.18f), Offset(center.x, center.y + s * 0.12f), s * 0.02f)
    for (i in 0..2) {
        val dy = center.y - s * 0.08f + i * s * 0.08f
        drawLine(edge.copy(alpha = 0.25f), Offset(center.x - s * 0.42f, dy), Offset(center.x - s * 0.10f, dy), s * 0.012f)
        drawLine(edge.copy(alpha = 0.25f), Offset(center.x + s * 0.10f, dy), Offset(center.x + s * 0.42f, dy), s * 0.012f)
    }
}

/** 台灯：底座 + 弯臂 + 灯罩 + 暖光。 */
internal fun DrawScope.drawDeskLamp(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val glow = 0.7f + 0.3f * sceneSway(phase, 0.3f)
    val dark = sceneFill(WoodDark, p, muted)
    drawOval(dark, Offset(center.x - s * 0.28f, center.y - s * 0.05f), Size(s * 0.56f, s * 0.16f))
    drawLine(dark, Offset(center.x, center.y), Offset(center.x + s * 0.20f, center.y - s * 0.60f), s * 0.05f, StrokeCap.Round)
    drawLine(dark, Offset(center.x + s * 0.20f, center.y - s * 0.60f), Offset(center.x + s * 0.55f, center.y - s * 0.50f), s * 0.05f, StrokeCap.Round)
    val shade = Path().apply {
        moveTo(center.x + s * 0.38f, center.y - s * 0.62f)
        lineTo(center.x + s * 0.78f, center.y - s * 0.50f)
        lineTo(center.x + s * 0.70f, center.y - s * 0.28f)
        lineTo(center.x + s * 0.46f, center.y - s * 0.38f)
        close()
    }
    drawPath(shade, sceneFill(LampWarm, p, muted))
    drawCircle(sceneFill(LampWarm, p, muted).copy(alpha = 0.16f * glow), s * 0.60f, Offset(center.x + s * 0.62f, center.y - s * 0.30f))
}

/** 灶台：台面 + 两个灶眼 + 旋钮。 */
internal fun DrawScope.drawStove(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    drawRoundRect(sceneFill(TileBlue, p, muted), Offset(center.x - s * 0.85f, center.y - s * 0.10f), Size(s * 1.7f, s * 0.70f), CornerRadius(s * 0.06f))
    drawRect(sceneFill(WoodDark, p, muted), Offset(center.x - s * 0.85f, center.y - s * 0.16f), Size(s * 1.7f, s * 0.10f))
    for (dx in listOf(-0.38f, 0.38f)) {
        drawCircle(sceneEdge(p, 0.35f), s * 0.20f, Offset(center.x + s * dx, center.y - s * 0.05f), style = Stroke(s * 0.025f))
        drawCircle(sceneFill(Color(0xFF4A5266), p, muted), s * 0.12f, Offset(center.x + s * dx, center.y - s * 0.05f))
    }
    for (dx in listOf(-0.55f, -0.35f, 0.35f, 0.55f)) {
        drawCircle(sceneFill(LampWarm, p, muted), s * 0.045f, Offset(center.x + s * dx, center.y + s * 0.40f))
    }
}

/** 锅：铜色锅身 + 锅盖 + 双耳。 */
internal fun DrawScope.drawPot(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val body = sceneFill(Copper, p, muted)
    drawRoundRect(body, Offset(center.x - s * 0.45f, center.y - s * 0.18f), Size(s * 0.90f, s * 0.60f), CornerRadius(s * 0.12f))
    drawOval(sceneFill(Color(0xFFE8A25A), p, muted), Offset(center.x - s * 0.42f, center.y - s * 0.28f), Size(s * 0.84f, s * 0.18f))
    drawCircle(sceneFill(WoodDark, p, muted), s * 0.05f, Offset(center.x, center.y - s * 0.36f))
    for (dir in listOf(-1f, 1f)) {
        drawOval(sceneEdge(p, 0.4f), Offset(center.x + dir * s * 0.48f - s * 0.08f, center.y - s * 0.02f), Size(s * 0.16f, s * 0.12f), style = Stroke(s * 0.035f))
    }
    drawLine(sceneEdge(p, 0.22f), Offset(center.x - s * 0.42f, center.y + s * 0.10f), Offset(center.x + s * 0.42f, center.y + s * 0.10f), s * 0.02f)
}

/** 蒸汽：三缕上升的弯线。 */
internal fun DrawScope.drawSteam(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val color = sceneFill(Color.White, p, (muted + 0.2f).coerceAtMost(0.85f)).copy(alpha = 0.55f)
    for (i in 0..2) {
        val dx = (i - 1) * s * 0.18f
        val path = Path().apply {
            moveTo(center.x + dx, center.y)
            var y = center.y
            var j = 0
            while (y > center.y - s * 1.0f) {
                val sway = sin((phase + i * 0.2f + j * 0.15f) * SCENE_TAU) * s * 0.08f
                quadraticTo(center.x + dx + sway, y - s * 0.16f, center.x + dx, y - s * 0.32f)
                y -= s * 0.32f
                j++
            }
        }
        drawPath(path, color, style = Stroke(s * 0.035f, cap = StrokeCap.Round))
    }
}

/** 罐子：玻璃身 + 木盖 + 内容物。 */
internal fun DrawScope.drawJar(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    drawRoundRect(sceneFill(JarGlass, p, (muted + 0.1f).coerceAtMost(0.8f)).copy(alpha = 0.85f), Offset(center.x - s * 0.28f, center.y - s * 0.35f), Size(s * 0.56f, s * 0.75f), CornerRadius(s * 0.10f))
    drawRoundRect(sceneFill(WoodDark, p, muted), Offset(center.x - s * 0.30f, center.y - s * 0.45f), Size(s * 0.60f, s * 0.14f), CornerRadius(s * 0.04f))
    for (i in 0..2) {
        drawCircle(sceneFill(BookColors[i % BookColors.size], p, muted), s * 0.07f, Offset(center.x - s * 0.12f + i * s * 0.12f, center.y + s * 0.10f))
    }
}

/** 小猫：蜷成一团，头和尾巴各在一侧。 */
internal fun DrawScope.drawKitten(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
    fillColor: Color = Color(0xFFB8AEA6),
) {
    val bob = sceneSway(phase, 0.5f) * s * 0.02f
    val c = Offset(center.x, center.y + bob)
    val fill = sceneFill(fillColor, p, muted)
    drawOval(fill, Offset(c.x - s * 0.55f, c.y - s * 0.30f), Size(s * 1.10f, s * 0.65f))
    drawCircle(fill, s * 0.30f, Offset(c.x - s * 0.40f, c.y - s * 0.42f))
    for (dir in listOf(-1f, 1f)) {
        val ear = Path().apply {
            moveTo(c.x - s * 0.40f + dir * s * 0.16f, c.y - s * 0.58f)
            lineTo(c.x - s * 0.40f + dir * s * 0.26f, c.y - s * 0.82f)
            lineTo(c.x - s * 0.40f + dir * s * 0.36f, c.y - s * 0.56f)
            close()
        }
        drawPath(ear, fill)
    }
    drawPath(
        Path().apply {
            moveTo(c.x + s * 0.48f, c.y - s * 0.12f)
            quadraticTo(c.x + s * 0.85f, c.y - s * 0.25f, c.x + s * 0.70f, c.y + s * 0.18f)
        },
        sceneEdge(p, 0.35f),
        style = Stroke(s * 0.09f, cap = StrokeCap.Round),
    )
    if (!sceneBlink(phase + 0.6f, 0.3f)) {
        for (dir in listOf(-1f, 1f)) drawCircle(sceneEdge(p, 0.55f), s * 0.035f, Offset(c.x - s * 0.40f + dir * s * 0.11f, c.y - s * 0.44f))
    }
}

/** 毛线球：圆 + 几道绕线 + 一根拖出来的线。 */
internal fun DrawScope.drawYarnBall(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val yarn = sceneFill(YarnPink, p, muted)
    drawCircle(yarn, s, center)
    for (i in 0..3) {
        val a = i * PI.toFloat() / 4f + 0.3f
        drawArc(
            sceneEdge(p, 0.22f),
            startAngle = a * 180f / PI.toFloat(),
            sweepAngle = 170f,
            useCenter = false,
            topLeft = Offset(center.x - s * (0.85f - i * 0.05f), center.y - s * 0.9f),
            size = Size(s * (1.7f - i * 0.1f), s * 1.8f),
            style = Stroke(s * 0.04f),
        )
    }
    drawPath(
        Path().apply {
            moveTo(center.x + s * 0.85f, center.y + s * 0.25f)
            quadraticTo(center.x + s * 1.6f, center.y + s * 0.55f, center.x + s * 1.9f, center.y + s * 0.20f)
        },
        sceneEdge(p, 0.3f),
        style = Stroke(s * 0.04f, cap = StrokeCap.Round),
    )
}

/** 纸箱：打开的纸板箱，猫可以坐进去。 */
internal fun DrawScope.drawBox(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val box = sceneFill(Cardboard, p, muted)
    val edge = sceneEdge(p, 0.3f)
    val left = center.x - s * 0.75f
    val top = center.y - s * 0.15f
    drawRect(box, Offset(left, top), Size(s * 1.5f, s * 0.75f))
    drawRect(edge, Offset(left, top), Size(s * 1.5f, s * 0.75f), style = Stroke(s * 0.03f))
    // 打开的四个翻盖。
    drawPath(Path().apply { moveTo(left, top); lineTo(left - s * 0.25f, top - s * 0.30f); lineTo(left + s * 0.30f, top - s * 0.12f); close() }, box)
    drawPath(Path().apply { moveTo(left + s * 1.5f, top); lineTo(left + s * 1.75f, top - s * 0.30f); lineTo(left + s * 1.20f, top - s * 0.12f); close() }, box)
    drawLine(edge.copy(alpha = 0.3f), Offset(left + s * 0.20f, top + s * 0.20f), Offset(left + s * 1.30f, top + s * 0.20f), s * 0.02f)
}

/** 坐垫：圆角垫 + 中心扣 + 四角小穗。 */
internal fun DrawScope.drawCushion(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val fill = sceneFill(Color(0xFFB98BC9), p, muted)
    drawRoundRect(fill, Offset(center.x - s * 0.70f, center.y - s * 0.35f), Size(s * 1.40f, s * 0.70f), CornerRadius(s * 0.22f))
    drawCircle(sceneEdge(p, 0.25f), s * 0.05f, center)
    for (dir in listOf(-1f, 1f)) {
        drawLine(sceneEdge(p, 0.2f), Offset(center.x + dir * s * 0.70f, center.y), Offset(center.x + dir * s * 0.82f, center.y), s * 0.03f, StrokeCap.Round)
    }
}
