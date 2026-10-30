// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 背景主题的**共用道具**：松树、蘑菇、萤火虫、蜜蜂、向日葵、浇水壶、月亮、
//! 星星、灯笼、栅栏。与角色/花草同一套调色（[sceneFill]/[sceneEdge]）和相位
//! （[sceneSway]），只在主题编排里被调用。

package xyz.erik.opi.scene

import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import kotlin.math.abs
import kotlin.math.sin

// 色相种子（最终都混进皮肤底色；见 OpiScene.kt 的 sceneFill）。
private val PineGreen = Color(0xFF3F6B4F)
private val PineDark = Color(0xFF2F523D)
private val PineTrunk = Color(0xFF6B4A3A)
private val MushroomCap = Color(0xFFD96B5F)
private val MushroomStem = Color(0xFFFFE9C7)
private val FireflyGlow = Color(0xFFFFE07A)
private val BeeYellow = Color(0xFFF2C14E)
private val SunflowerPetal = Color(0xFFF2C14E)
private val SunflowerCenter = Color(0xFF8A5A2B)
private val WateringCanBlue = Color(0xFF7FB7C9)
private val MoonYellow = Color(0xFFFFE9A8)
private val StarWhite = Color(0xFFFFF3C4)
private val LanternOrange = Color(0xFFF2A65A)
private val FenceBrown = Color(0xFF8A6A4F)

/** 松树：树干 + 三层三角树冠。[s] ≈ 树高。 */
internal fun DrawScope.drawPine(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val sway = sceneSway(phase, 0.22f) * s * 0.02f
    val cx = center.x + sway
    val base = center.y
    drawRoundRect(
        sceneFill(PineTrunk, p, muted),
        topLeft = Offset(center.x - s * 0.055f, base - s * 0.28f),
        size = Size(s * 0.11f, s * 0.30f),
        cornerRadius = CornerRadius(s * 0.03f),
    )
    for (i in 0..2) {
        val w = s * (0.56f - i * 0.13f)
        val h = s * 0.34f
        val y = base - s * 0.20f - i * s * 0.20f
        val tree = Path().apply {
            moveTo(cx, y - h)
            lineTo(cx - w / 2f, y)
            lineTo(cx + w / 2f, y)
            close()
        }
        drawPath(tree, sceneFill(if (i % 2 == 0) PineGreen else PineDark, p, muted))
        drawPath(tree, sceneEdge(p, 0.22f), style = Stroke(s * 0.018f, join = StrokeJoin.Round))
    }
}

/** 蘑菇：奶油色菌柄 + 红伞 + 白点。 */
internal fun DrawScope.drawMushroom(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val bob = sceneSway(phase, 0.6f) * s * 0.02f
    val c = Offset(center.x, center.y + bob)
    drawRoundRect(
        sceneFill(MushroomStem, p, muted),
        topLeft = Offset(c.x - s * 0.18f, c.y - s * 0.10f),
        size = Size(s * 0.36f, s * 0.55f),
        cornerRadius = CornerRadius(s * 0.16f),
    )
    drawPath(
        Path().apply {
            moveTo(c.x - s * 0.52f, c.y - s * 0.05f)
            quadraticTo(c.x - s * 0.50f, c.y - s * 0.52f, c.x, c.y - s * 0.52f)
            quadraticTo(c.x + s * 0.50f, c.y - s * 0.52f, c.x + s * 0.52f, c.y - s * 0.05f)
            close()
        },
        sceneFill(MushroomCap, p, muted),
    )
    for ((dx, dy, r) in listOf(
        Triple(-0.22f, -0.30f, 0.07f),
        Triple(0.16f, -0.38f, 0.055f),
        Triple(0.30f, -0.16f, 0.04f),
    )) {
        drawCircle(sceneFill(Color.White, p, (muted + 0.2f).coerceAtMost(0.9f)), s * r, Offset(c.x + s * dx, c.y + s * dy))
    }
}

/** 萤火虫：暖光点 + 一对小翅膀；[twinkle] 由相位决定。 */
internal fun DrawScope.drawFirefly(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val twinkle = 0.35f + 0.65f * abs(sin((phase + 0.13f) * SCENE_TAU * 2f))
    drawCircle(sceneFill(FireflyGlow, p, muted).copy(alpha = 0.20f * twinkle), s * 1.9f, center)
    drawCircle(sceneFill(FireflyGlow, p, muted).copy(alpha = 0.85f * twinkle), s * 0.45f, center)
    drawCircle(Color.White.copy(alpha = 0.75f * twinkle), s * 0.18f, center)
    for (dir in listOf(-1f, 1f)) {
        drawOval(
            Color.White.copy(alpha = 0.35f * twinkle),
            Offset(center.x + dir * s * 0.18f - s * 0.22f, center.y - s * 0.35f),
            Size(s * 0.44f, s * 0.26f),
        )
    }
}

/** 蜜蜂：黄黑条纹身体 + 透明翅膀；[phase] 控制翅膀开合。 */
internal fun DrawScope.drawBee(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val flap = 0.45f + 0.55f * abs(sin(phase * SCENE_TAU * 4f))
    val body = sceneFill(BeeYellow, p, muted)
    val edge = sceneEdge(p, 0.55f)
    drawOval(Color.White.copy(alpha = 0.45f * flap), Offset(center.x - s * 0.55f, center.y - s * 0.62f), Size(s * 0.62f, s * 0.40f))
    drawOval(Color.White.copy(alpha = 0.45f * flap), Offset(center.x - s * 0.05f, center.y - s * 0.62f), Size(s * 0.62f, s * 0.40f))
    drawOval(body, Offset(center.x - s * 0.42f, center.y - s * 0.25f), Size(s * 0.84f, s * 0.50f))
    for (dx in listOf(-0.18f, 0.08f)) {
        drawLine(edge.copy(alpha = 0.55f), Offset(center.x + s * dx, center.y - s * 0.22f), Offset(center.x + s * dx, center.y + s * 0.20f), s * 0.07f, StrokeCap.Round)
    }
    drawCircle(edge, s * 0.16f, Offset(center.x - s * 0.42f, center.y))
    drawLine(edge, Offset(center.x - s * 0.50f, center.y - s * 0.10f), Offset(center.x - s * 0.66f, center.y - s * 0.30f), s * 0.025f, StrokeCap.Round)
}

/** 向日葵：大花盘 + 茎叶；[center] 是花盘中心。 */
internal fun DrawScope.drawSunflower(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val sway = sceneSway(phase, 0.35f) * s * 0.05f
    val c = Offset(center.x + sway, center.y)
    // 茎（向下，画在花盘后面）。
    drawLine(sceneFill(PineGreen, p, muted), Offset(c.x, c.y + s * 0.35f), Offset(center.x, center.y + s * 1.8f), s * 0.06f, StrokeCap.Round)
    for (dir in listOf(-1f, 1f)) {
        rotate(degrees = dir * 35f, pivot = Offset(c.x + dir * s * 0.18f, c.y + s * 1.0f)) {
            drawOval(sceneFill(PineGreen, p, muted), Offset(c.x + dir * s * 0.1f, c.y + s * 0.85f), Size(s * 0.45f, s * 0.20f))
        }
    }
    for (i in 0 until 10) {
        val a = i * SCENE_TAU / 10f
        drawCircle(
            sceneFill(SunflowerPetal, p, muted),
            s * 0.30f,
            Offset(c.x + sin(a) * s * 0.42f, c.y - kotlin.math.cos(a) * s * 0.42f),
        )
    }
    drawCircle(sceneFill(SunflowerCenter, p, muted), s * 0.34f, c)
    drawCircle(sceneEdge(p, 0.25f), s * 0.34f, c, style = Stroke(s * 0.03f))
}

/** 浇水壶：圆身 + 壶嘴 + 提手，微微上下点头。 */
internal fun DrawScope.drawWateringCan(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val tilt = sceneSway(phase, 0.5f) * 5f
    rotate(degrees = tilt, pivot = center) {
        val body = sceneFill(WateringCanBlue, p, muted)
        val edge = sceneEdge(p, 0.45f)
        drawRoundRect(
            body,
            topLeft = Offset(center.x - s * 0.45f, center.y - s * 0.20f),
            size = Size(s * 0.75f, s * 0.55f),
            cornerRadius = CornerRadius(s * 0.15f),
        )
        drawRoundRect(edge, topLeft = Offset(center.x - s * 0.45f, center.y - s * 0.20f), size = Size(s * 0.75f, s * 0.55f), cornerRadius = CornerRadius(s * 0.15f), style = Stroke(s * 0.035f))
        drawLine(body, Offset(center.x + s * 0.28f, center.y), Offset(center.x + s * 0.62f, center.y - s * 0.28f), s * 0.12f, StrokeCap.Round)
        drawLine(edge, Offset(center.x + s * 0.28f, center.y), Offset(center.x + s * 0.62f, center.y - s * 0.28f), s * 0.035f, StrokeCap.Round)
        drawPath(
            Path().apply {
                moveTo(center.x - s * 0.05f, center.y - s * 0.20f)
                quadraticTo(center.x + s * 0.10f, center.y - s * 0.62f, center.x + s * 0.28f, center.y - s * 0.20f)
            },
            edge,
            style = Stroke(s * 0.04f, cap = StrokeCap.Round),
        )
    }
}

/** 月亮：暖色圆盘 + 柔光 + 几个环形山（满月，避免裁切背景色）。 */
internal fun DrawScope.drawMoon(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val glow = 0.55f + 0.15f * sceneSway(phase, 0.2f)
    drawCircle(sceneFill(MoonYellow, p, muted).copy(alpha = 0.12f * glow), s * 1.45f, center)
    drawCircle(sceneFill(MoonYellow, p, muted), s, center)
    val crater = sceneEdge(p, 0.10f)
    drawCircle(crater, s * 0.16f, Offset(center.x - s * 0.30f, center.y - s * 0.18f))
    drawCircle(crater, s * 0.11f, Offset(center.x + s * 0.24f, center.y + s * 0.20f))
    drawCircle(crater, s * 0.07f, Offset(center.x + s * 0.10f, center.y - s * 0.44f))
}

/** 星星：固定分布的十字/圆点，亮度随相位闪。 */
internal fun DrawScope.drawStars(
    p: ScenePalette,
    w: Float,
    h: Float,
    phase: Float,
    muted: Float,
) {
    val spots = listOf(
        0.10f to 0.12f, 0.22f to 0.26f, 0.36f to 0.08f, 0.48f to 0.20f,
        0.58f to 0.10f, 0.67f to 0.30f, 0.76f to 0.14f, 0.88f to 0.24f,
        0.93f to 0.08f, 0.30f to 0.36f, 0.53f to 0.40f, 0.82f to 0.38f,
    )
    val star = sceneFill(StarWhite, p, muted)
    for ((i, spot) in spots.withIndex()) {
        val twinkle = 0.30f + 0.70f * abs(sin((phase + i * 0.11f) * SCENE_TAU * 1.5f))
        val c = Offset(spot.first * w, spot.second * h)
        val r = (0.008f + 0.004f * (i % 3)) * minOf(w, h)
        drawCircle(star.copy(alpha = 0.85f * twinkle), r, c)
        if (i % 3 == 0) {
            drawLine(star.copy(alpha = 0.45f * twinkle), Offset(c.x - r * 2.5f, c.y), Offset(c.x + r * 2.5f, c.y), r * 0.5f, StrokeCap.Round)
            drawLine(star.copy(alpha = 0.45f * twinkle), Offset(c.x, c.y - r * 2.5f), Offset(c.x, c.y + r * 2.5f), r * 0.5f, StrokeCap.Round)
        }
    }
}

/** 灯笼：暖橙灯身 + 上下盖 + 提手 + 光晕。 */
internal fun DrawScope.drawLantern(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val glow = 0.7f + 0.3f * abs(sceneSway(phase, 0.4f))
    drawCircle(sceneFill(LanternOrange, p, muted).copy(alpha = 0.16f * glow), s * 1.5f, center)
    val body = sceneFill(LanternOrange, p, muted)
    val edge = sceneEdge(p, 0.45f)
    drawRoundRect(body, Offset(center.x - s * 0.38f, center.y - s * 0.45f), Size(s * 0.76f, s * 0.90f), CornerRadius(s * 0.18f))
    drawRoundRect(edge, Offset(center.x - s * 0.38f, center.y - s * 0.45f), Size(s * 0.76f, s * 0.90f), CornerRadius(s * 0.18f), style = Stroke(s * 0.035f))
    drawRect(edge, Offset(center.x - s * 0.30f, center.y - s * 0.55f), Size(s * 0.60f, s * 0.10f))
    drawRect(edge, Offset(center.x - s * 0.30f, center.y + s * 0.45f), Size(s * 0.60f, s * 0.10f))
    drawLine(edge, Offset(center.x, center.y - s * 0.55f), Offset(center.x, center.y - s * 0.85f), s * 0.035f, StrokeCap.Round)
    drawCircle(Color.White.copy(alpha = 0.55f * glow), s * 0.16f, Offset(center.x, center.y))
}

/** 栅栏：两根横杆 + 一排竖桩（夜色里给猫坐）。 */
internal fun DrawScope.drawFence(
    p: ScenePalette,
    left: Float,
    top: Float,
    width: Float,
    height: Float,
    muted: Float,
) {
    val wood = sceneFill(FenceBrown, p, muted)
    val edge = sceneEdge(p, 0.20f)
    drawLine(wood, Offset(left, top + height * 0.35f), Offset(left + width, top + height * 0.35f), height * 0.09f, StrokeCap.Round)
    drawLine(wood, Offset(left, top + height * 0.72f), Offset(left + width, top + height * 0.72f), height * 0.09f, StrokeCap.Round)
    var x = left
    while (x <= left + width) {
        drawLine(wood, Offset(x, top), Offset(x, top + height), height * 0.10f, StrokeCap.Round)
        drawLine(edge.copy(alpha = 0.24f), Offset(x, top), Offset(x, top + height), height * 0.10f, StrokeCap.Round)
        x += width / 6f
    }
}
