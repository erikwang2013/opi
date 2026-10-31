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
            0f to sceneFill(CitySky, p, (muted * 0.5f).coerceAtLeast(0.26f)),
            0.62f to p.base,
            1f to sceneFill(CityGround, p, (muted * 0.4f).coerceAtLeast(0.20f)),
        ),
        size = size,
    )
    drawSun(p, Offset(w * 0.82f, h * 0.14f), u * 0.07f, phase, muted)
    drawCloud(p, Offset(w * 0.18f, h * 0.14f), u * 0.18f, phase, muted)
    drawCloud(p, Offset(w * 0.56f, h * 0.08f), u * 0.12f, phase + 0.4f, muted)
    drawOval(sceneFill(CityGround, p, 0.52f), Offset(-w * 0.1f, h * 0.78f), Size(w * 1.2f, h * 0.42f))
    drawBuildingRow(p, 0f, h * 0.86f, w, h * 0.42f, phase, muted)
    // 人行道树与花箱：城市也有花枝。
    drawTree(p, Offset(w * 0.16f, h * 0.93f), u * 0.34f, phase, muted, BotanyLeaf, BotanyLeafDark, BlossomPink)
    drawTree(p, Offset(w * 0.86f, h * 0.94f), u * 0.30f, phase + 0.4f, muted, BotanyLeafDark, BotanyLeafDark, BlossomWhite)
    drawFlowerBox(p, Offset(w * 0.29f, h * 0.86f), u * 0.10f, phase + 0.2f, muted, BlossomPink)
    drawFlowerBox(p, Offset(w * 0.72f, h * 0.86f), u * 0.09f, phase + 0.5f, muted, BlossomWhite)
    drawStreetLamp(p, Offset(w * 0.09f, h * 0.94f), u * 0.26f, phase, muted)
    // 花藤爬上灯杆，根在灯座。
    drawClimbingVine(
        p, Offset(w * 0.09f, h * 0.94f), Offset(w * 0.09f - u * 0.02f, h * 0.94f - u * 0.30f),
        u * 0.30f, phase, muted, BlossomPink, flowers = 2, seed = 0.2f,
    )
    drawCar(p, Offset(w * 0.56f, h * 0.94f), u * 0.16f, phase, muted)
    // 前景花圃。
    for ((i, x) in listOf(0.34f, 0.52f, 0.94f).withIndex()) {
        drawFlowerBed(
            p, Offset(w * x, h * 0.99f), u * 0.18f, u * 0.11f,
            phase + i * 0.2f, muted, listOf(BlossomPink, BlossomYellow, BlossomWhite), count = 4,
        )
    }
    val personC = Offset(w * 0.40f, h * 0.84f)
    val personS = u * 0.20f
    drawContactShadow(p, Offset(personC.x, personC.y + u * 0.13f), u * 0.24f, muted)
    drawContactShadow(p, Offset(w * 0.80f, h * 0.90f + u * 0.12f), u * 0.22f, muted)
    drawCatFigure(p, Offset(w * 0.80f, h * 0.90f), u * 0.18f, phase + 0.3f, muted, detail = 0.5f)
    drawPersonFigure(p, personC, personS, phase, muted, detail = 0.5f)
    drawBalloon(
        p, Offset(w * 0.46f, h * 0.38f), u * 0.06f, phase, muted,
        stringEnd = Offset(personC.x + personS * 0.23f, personC.y - personS * 0.28f),
    )
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
            0f to sceneFill(VegSky, p, (muted * 0.5f).coerceAtLeast(0.26f)),
            0.62f to p.base,
            1f to sceneFill(VegHill, p, (muted * 0.32f).coerceAtLeast(0.20f)),
        ),
        size = size,
    )
    drawSun(p, Offset(w * 0.82f, h * 0.16f), u * 0.09f, phase, muted)
    drawCloud(p, Offset(w * 0.20f, h * 0.13f), u * 0.18f, phase, muted)
    drawCloud(p, Offset(w * 0.58f, h * 0.09f), u * 0.12f, phase + 0.4f, muted)
    // 远景树线与草坡。
    drawTree(p, Offset(w * 0.10f, h * 0.76f), u * 0.34f, phase, muted, BotanyLeaf, BotanyLeafDark)
    drawTree(p, Offset(w * 0.90f, h * 0.78f), u * 0.30f, phase + 0.3f, muted, BotanyLeaf, BotanyLeafDark)
    drawOval(sceneFill(VegHill, p, 0.5f), Offset(-w * 0.12f, h * 0.68f), Size(w * 0.85f, h * 0.45f))
    drawOval(sceneFill(VegHillDark, p, 0.55f), Offset(w * 0.35f, h * 0.74f), Size(w * 0.85f, h * 0.42f))
    drawFence(p, 0f, h * 0.66f, w, h * 0.12f, muted)
    // 花藤爬上栅栏，补足花枝/花藤。
    drawClimbingVine(
        p, Offset(w * 0.06f, h * 0.80f), Offset(w * 0.08f, h * 0.56f),
        u * 0.34f, phase, muted, BlossomPink, flowers = 2, seed = 0.2f,
    )
    drawClimbingVine(
        p, Offset(w * 0.94f, h * 0.80f), Offset(w * 0.92f, h * 0.58f),
        u * 0.30f, phase + 0.4f, muted, BlossomWhite, flowers = 2, seed = 0.7f,
    )
    // 菜垄：先铺土，再种白菜/胡萝卜/番茄，根都落在土里。
    for ((x, r) in listOf(0.14f to 0.09f, 0.30f to 0.08f, 0.46f to 0.10f, 0.62f to 0.08f, 0.82f to 0.09f)) {
        drawOval(
            sceneFill(Color(0xFF9A7B55), p, (muted + 0.30f).coerceAtMost(0.8f)).copy(alpha = 0.55f),
            Offset(w * x - u * r * 1.4f, h * 0.955f),
            Size(u * r * 2.8f, u * r * 0.35f),
        )
    }
    for ((x, s) in listOf(0.14f to 0.10f, 0.30f to 0.08f, 0.46f to 0.10f, 0.62f to 0.08f, 0.90f to 0.09f)) {
        drawCabbage(p, Offset(w * x, h * 0.96f), u * s, phase + x, muted)
    }
    for ((x, s) in listOf(0.22f to 0.08f, 0.38f to 0.07f, 0.70f to 0.075f, 0.82f to 0.07f)) {
        drawCarrot(p, Offset(w * x, h * 0.95f), u * s, phase + x, muted)
    }
    drawTomatoPlant(p, Offset(w * 0.54f, h * 0.92f), u * 0.12f, phase + 0.3f, muted)
    drawScarecrow(p, Offset(w * 0.50f, h * 0.78f), u * 0.22f, phase, muted)
    drawFlowerBed(
        p, Offset(w * 0.84f, h * 0.99f), u * 0.18f, u * 0.11f,
        phase + 0.3f, muted, listOf(BlossomYellow, BlossomPink, BlossomWhite), count = 4,
    )
    val personC = Offset(w * 0.68f, h * 0.84f)
    drawContactShadow(p, Offset(personC.x, personC.y + u * 0.12f), u * 0.22f, muted)
    drawContactShadow(p, Offset(w * 0.10f, h * 0.94f + u * 0.10f), u * 0.20f, muted)
    drawPersonFigure(p, personC, u * 0.18f, phase + 0.2f, muted, detail = 0.5f)
    drawWateringCan(p, Offset(w * 0.75f, h * 0.88f), u * 0.07f, phase + 0.4f, muted)
    drawCatFigure(p, Offset(w * 0.10f, h * 0.94f), u * 0.16f, phase + 0.5f, muted, detail = 0.5f)
    drawButterfly(p, Offset(w * 0.34f, h * 0.34f), u * 0.045f, phase + 0.2f, muted)
    drawBird(p, Offset(w * 0.42f, h * 0.22f), u * 0.022f, phase, muted)
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
