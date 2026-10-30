// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 落日与路灯主题。落日=暖色天幕/低垂的太阳/剪影山丘/栅栏；路灯=夜路/多盏
//! 路灯/长椅/行人/萤火虫。

package xyz.erik.opi.scene

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import kotlin.math.min

private val SunsetHigh = Color(0xFF6B4A7A)
private val SunsetMid = Color(0xFFE8826A)
private val SunsetLow = Color(0xFFF6B26A)
private val SunsetSea = Color(0xFF8A5A7A)
private val NightRoad = Color(0xFF1E2745)
private val RoadPath = Color(0xFF3A4468)
private val LampGlow = Color(0xFFFFE3A0)

internal fun DrawScope.drawSunsetBackdrop(p: ScenePalette, phase: Float, muted: Float) {
    val w = size.width
    val h = size.height
    if (w < 2f || h < 2f) return
    val u = min(w, h)
    drawRect(
        brush = Brush.verticalGradient(
            0f to sceneFill(SunsetHigh, p, (muted * 0.35f).coerceAtLeast(0.18f)),
            0.42f to sceneFill(SunsetMid, p, (muted * 0.30f).coerceAtLeast(0.18f)),
            0.72f to sceneFill(SunsetLow, p, (muted * 0.25f).coerceAtLeast(0.16f)),
            1f to p.base,
        ),
        size = size,
    )
    drawSun(p, Offset(w * 0.52f, h * 0.58f), u * 0.14f, phase, muted)
    drawCloud(p, Offset(w * 0.20f, h * 0.22f), u * 0.18f, phase, muted)
    drawCloud(p, Offset(w * 0.80f, h * 0.16f), u * 0.14f, phase + 0.4f, muted)
    // 远山剪影。
    drawOval(sceneFill(SunsetSea, p, 0.25f), Offset(-w * 0.15f, h * 0.66f), Size(w * 0.85f, h * 0.34f))
    drawOval(sceneFill(SunsetSea, p, 0.18f), Offset(w * 0.42f, h * 0.70f), Size(w * 0.85f, h * 0.30f))
    // 水面/地面光带。
    drawRect(sceneFill(SunsetSea, p, (muted * 0.30f).coerceAtLeast(0.18f)), Offset(0f, h * 0.80f), Size(w, h * 0.20f))
    drawOval(sceneFill(LampGlow, p, 0.20f).copy(alpha = 0.20f), Offset(w * 0.36f, h * 0.83f), Size(w * 0.30f, h * 0.06f))
    drawFence(p, 0f, h * 0.82f, w, h * 0.13f, muted)
    drawBird(p, Offset(w * 0.30f, h * 0.34f), u * 0.028f, phase, muted)
    drawBird(p, Offset(w * 0.40f, h * 0.29f), u * 0.022f, phase + 0.3f, muted)
    drawCatFigure(p, Offset(w * 0.18f, h * 0.92f), u * 0.15f, phase + 0.4f, muted, detail = 0.4f)
    drawPersonFigure(p, Offset(w * 0.72f, h * 0.86f), u * 0.17f, phase + 0.2f, muted, detail = 0.45f)
    drawFlowerCluster(p, Offset(w * 0.90f, h * 0.99f), u * 0.08f, phase + 0.5f, muted)
}

internal fun DrawScope.drawSunsetHalo(
    p: ScenePalette,
    phase: Float,
    halo: Float,
    barTop: Float,
    barBottom: Float,
    topXs: List<Float>,
    bottomXs: List<Float>,
    muted: Float,
) {
    val u = halo
    drawSun(p, Offset(topXs[0], barTop - u * 0.55f), u * 0.32f, phase, muted)
    drawCloud(p, Offset(topXs[1], barTop - u * 0.55f), u * 0.42f, phase, muted)
    drawPersonFigure(p, Offset(topXs[2], barTop - u * 0.20f), u * 0.62f, phase + 0.2f, muted, detail = 1f)
    drawBalloon(p, Offset(topXs[3], barTop - u * 0.50f), u * 0.40f, phase, muted)
    drawFence(p, bottomXs[0] - u, barBottom + u * 0.36f, (bottomXs[2] - bottomXs[0]) + u * 2f, u * 0.50f, muted)
    drawCatFigure(p, Offset(bottomXs[1], barBottom + u * 0.48f), u * 0.46f, phase + 0.4f, muted, detail = 1f)
    drawFlowerCluster(p, Offset(bottomXs[2], barBottom + u * 0.42f), u * 0.34f, phase + 0.5f, muted, hanging = true)
}

internal fun DrawScope.drawStreetlightBackdrop(p: ScenePalette, phase: Float, muted: Float) {
    val w = size.width
    val h = size.height
    if (w < 2f || h < 2f) return
    val u = min(w, h)
    drawRect(
        brush = Brush.verticalGradient(
            0f to sceneFill(NightRoad, p, (muted * 0.35f).coerceAtLeast(0.18f)),
            0.68f to p.base,
            1f to sceneFill(RoadPath, p, (muted * 0.30f).coerceAtLeast(0.18f)),
        ),
        size = size,
    )
    drawStars(p, w, h * 0.55f, phase, muted)
    drawMoon(p, Offset(w * 0.82f, h * 0.16f), u * 0.07f, phase, muted)
    drawOval(sceneFill(RoadPath, p, 0.30f), Offset(w * 0.02f, h * 0.82f), Size(w * 0.96f, h * 0.28f))
    drawBareTree(p, Offset(w * 0.30f, h * 0.86f), u * 0.32f, phase, muted)
    drawBareTree(p, Offset(w * 0.70f, h * 0.84f), u * 0.26f, phase + 0.4f, muted)
    drawStreetLamp(p, Offset(w * 0.12f, h * 0.92f), u * 0.26f, phase, muted)
    drawStreetLamp(p, Offset(w * 0.50f, h * 0.90f), u * 0.22f, phase + 0.2f, muted)
    drawStreetLamp(p, Offset(w * 0.88f, h * 0.92f), u * 0.24f, phase + 0.4f, muted)
    drawBench(p, Offset(w * 0.66f, h * 0.88f), u * 0.16f, phase, muted)
    drawPersonFigure(p, Offset(w * 0.38f, h * 0.86f), u * 0.16f, phase + 0.2f, muted, detail = 0.45f)
    drawCatFigure(p, Offset(w * 0.46f, h * 0.94f), u * 0.13f, phase + 0.4f, muted, detail = 0.4f)
    fireflies(p, w, h, u, phase, muted, listOf(0.24f to 0.55f, 0.58f to 0.48f, 0.78f to 0.58f))
}

internal fun DrawScope.drawStreetlightHalo(
    p: ScenePalette,
    phase: Float,
    halo: Float,
    barTop: Float,
    barBottom: Float,
    topXs: List<Float>,
    bottomXs: List<Float>,
    muted: Float,
) {
    val u = halo
    drawStreetLamp(p, Offset(topXs[0], barTop + u * 0.30f), u * 0.55f, phase, muted)
    drawBareTree(p, Offset(topXs[1], barTop + u * 0.30f), u * 0.78f, phase + 0.4f, muted)
    drawPersonFigure(p, Offset(topXs[2], barTop - u * 0.20f), u * 0.62f, phase + 0.2f, muted, detail = 1f)
    drawStreetLamp(p, Offset(topXs[3], barTop + u * 0.30f), u * 0.52f, phase + 0.4f, muted)
    drawBench(p, Offset(bottomXs[0], barBottom + u * 0.45f), u * 0.38f, phase, muted)
    drawCatFigure(p, Offset(bottomXs[1], barBottom + u * 0.48f), u * 0.46f, phase + 0.4f, muted, detail = 1f)
    drawFirefly(p, Offset(bottomXs[2], barBottom + u * 0.38f), u * 0.35f, phase, muted)
    drawStars(p, size.width, barTop, phase, muted)
}
