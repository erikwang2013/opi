// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 城市与菜地主题。城市是楼群/路灯/小车；菜地是白菜/胡萝卜/番茄/稻草人 +
//! 园丁。角色仍是同一批（猫、小人、兔），只换场景和道具。

package xyz.erik.opi.scene

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import kotlin.math.min

private val CitySky = Color(0xFFF4D6C8)
private val CityGround = Color(0xFF9AA3B5)
private val VegSky = Color(0xFFF5E7C8)
private val VegHill = Color(0xFFA8C98A)
private val VegHillDark = Color(0xFF86A96B)

internal fun DrawScope.drawCityBackdrop(p: ScenePalette, phase: Float, muted: Float) {
    val w = size.width
    val h = size.height
    if (w < 2f || h < 2f) return
    val u = min(w, h)
    drawRect(
        brush = Brush.verticalGradient(
            0f to sceneFill(CitySky, p, (muted * 0.5f).coerceAtLeast(0.28f)),
            0.62f to p.base,
            1f to sceneFill(CityGround, p, (muted * 0.4f).coerceAtLeast(0.22f)),
        ),
        size = size,
    )
    drawOval(sceneFill(CityGround, p, 0.52f), Offset(-w * 0.1f, h * 0.78f), Size(w * 1.2f, h * 0.42f))
    drawBuildingRow(p, 0f, h * 0.86f, w, h * 0.42f, phase, muted)
    drawCloud(p, Offset(w * 0.18f, h * 0.14f), u * 0.16f, phase, muted)
    drawCloud(p, Offset(w * 0.74f, h * 0.10f), u * 0.13f, phase + 0.4f, muted)
    drawStreetLamp(p, Offset(w * 0.09f, h * 0.92f), u * 0.24f, phase, muted)
    drawCar(p, Offset(w * 0.57f, h * 0.93f), u * 0.14f, phase, muted)
    drawCatFigure(p, Offset(w * 0.80f, h * 0.88f), u * 0.16f, phase + 0.3f, muted, detail = 0.4f)
    drawPersonFigure(p, Offset(w * 0.38f, h * 0.82f), u * 0.18f, phase, muted, detail = 0.45f)
    drawBalloon(p, Offset(w * 0.45f, h * 0.38f), u * 0.05f, phase, muted)
    drawBird(p, Offset(w * 0.30f, h * 0.22f), u * 0.028f, phase, muted)
    drawBird(p, Offset(w * 0.42f, h * 0.17f), u * 0.022f, phase + 0.3f, muted)
}

internal fun DrawScope.drawCityHalo(
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
    drawBuildingRow(p, topXs[0] - u * 0.80f, barTop + u * 0.35f, u * 1.6f, u * 0.90f, phase, muted)
    drawStreetLamp(p, Offset(topXs[1], barTop + u * 0.30f), u * 0.55f, phase, muted)
    drawPersonFigure(p, Offset(topXs[2], barTop - u * 0.20f), u * 0.62f, phase, muted, detail = 1f)
    drawBalloon(p, Offset(topXs[3], barTop - u * 0.50f), u * 0.40f, phase, muted)
    drawCar(p, Offset(bottomXs[0], barBottom + u * 0.45f), u * 0.38f, phase, muted)
    drawFlowerCluster(p, Offset(bottomXs[1], barBottom + u * 0.42f), u * 0.34f, phase + 0.2f, muted, hanging = true)
    drawCatFigure(p, Offset(bottomXs[2], barBottom + u * 0.48f), u * 0.48f, phase + 0.3f, muted, detail = 1f)
}

internal fun DrawScope.drawVegetableBackdrop(p: ScenePalette, phase: Float, muted: Float) {
    val w = size.width
    val h = size.height
    if (w < 2f || h < 2f) return
    val u = min(w, h)
    drawRect(
        brush = Brush.verticalGradient(
            0f to sceneFill(VegSky, p, (muted * 0.5f).coerceAtLeast(0.28f)),
            0.62f to p.base,
            1f to sceneFill(VegHill, p, (muted * 0.35f).coerceAtLeast(0.22f)),
        ),
        size = size,
    )
    drawSun(p, Offset(w * 0.82f, h * 0.16f), u * 0.09f, phase, muted)
    drawCloud(p, Offset(w * 0.20f, h * 0.13f), u * 0.16f, phase, muted)
    drawOval(sceneFill(VegHill, p, 0.5f), Offset(-w * 0.12f, h * 0.68f), Size(w * 0.85f, h * 0.45f))
    drawOval(sceneFill(VegHillDark, p, 0.55f), Offset(w * 0.35f, h * 0.74f), Size(w * 0.85f, h * 0.42f))
    drawFence(p, 0f, h * 0.66f, w, h * 0.12f, muted)
    // 菜垄：白菜/胡萝卜/番茄。
    for ((x, s) in listOf(0.14f to 0.085f, 0.30f to 0.07f, 0.46f to 0.09f, 0.62f to 0.07f, 0.90f to 0.08f)) {
        drawCabbage(p, Offset(w * x, h * 0.96f), u * s, phase + x, muted)
    }
    for ((x, s) in listOf(0.22f to 0.07f, 0.38f to 0.06f, 0.70f to 0.065f, 0.82f to 0.06f)) {
        drawCarrot(p, Offset(w * x, h * 0.95f), u * s, phase + x, muted)
    }
    drawTomatoPlant(p, Offset(w * 0.54f, h * 0.92f), u * 0.10f, phase + 0.3f, muted)
    drawScarecrow(p, Offset(w * 0.50f, h * 0.78f), u * 0.20f, phase, muted)
    drawPersonFigure(p, Offset(w * 0.68f, h * 0.82f), u * 0.16f, phase + 0.2f, muted, detail = 0.45f)
    drawWateringCan(p, Offset(w * 0.75f, h * 0.86f), u * 0.06f, phase + 0.4f, muted)
    drawCatFigure(p, Offset(w * 0.10f, h * 0.92f), u * 0.14f, phase + 0.5f, muted, detail = 0.4f)
}

internal fun DrawScope.drawVegetableHalo(
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
    drawTomatoPlant(p, Offset(topXs[0], barTop + u * 0.10f), u * 0.55f, phase + 0.3f, muted)
    drawScarecrow(p, Offset(topXs[1], barTop + u * 0.25f), u * 0.55f, phase, muted)
    drawPersonFigure(p, Offset(topXs[2], barTop - u * 0.20f), u * 0.62f, phase + 0.2f, muted, detail = 1f)
    drawWateringCan(p, Offset(topXs[3], barTop - u * 0.35f), u * 0.50f, phase + 0.4f, muted)
    drawCabbage(p, Offset(bottomXs[0], barBottom + u * 0.45f), u * 0.42f, phase + 0.1f, muted)
    drawCarrot(p, Offset(bottomXs[1], barBottom + u * 0.38f), u * 0.42f, phase + 0.5f, muted)
    drawCabbage(p, Offset(bottomXs[2], barBottom + u * 0.48f), u * 0.38f, phase + 0.7f, muted)
}
