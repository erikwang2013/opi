// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 乡村与狗群主题。乡村=农舍/风车/庄稼垄/栅栏/农夫;狗群=多只不同毛色的狗 +
//! 球 + 长椅 + 公园。

package xyz.erik.opi.scene

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import kotlin.math.min

private val CountrySky = Color(0xFFB7D3E8)
private val CountryHill = Color(0xFF8FBF7A)
private val CountryHillDark = Color(0xFF6FA05A)
private val ParkSky = Color(0xFFCDE8F2)
private val ParkHill = Color(0xFF9CCF8A)
private val DogBrown = Color(0xFFB58A5A)
private val DogCream = Color(0xFFF2E3C8)
private val DogGray = Color(0xFFB8AEA6)
private val DogDark = Color(0xFF6B6468)

internal fun DrawScope.drawCountrysideBackdrop(p: ScenePalette, phase: Float, muted: Float) {
    val w = size.width
    val h = size.height
    if (w < 2f || h < 2f) return
    val u = min(w, h)
    drawRect(
        brush = Brush.verticalGradient(
            0f to sceneFill(CountrySky, p, (muted * 0.4f).coerceAtLeast(0.22f)),
            0.62f to p.base,
            1f to sceneFill(CountryHill, p, (muted * 0.3f).coerceAtLeast(0.18f)),
        ),
        size = size,
    )
    drawSun(p, Offset(w * 0.82f, h * 0.16f), u * 0.09f, phase, muted)
    drawCloud(p, Offset(w * 0.20f, h * 0.14f), u * 0.16f, phase, muted)
    drawOval(sceneFill(CountryHill, p, 0.48f), Offset(-w * 0.1f, h * 0.66f), Size(w * 0.75f, h * 0.42f))
    drawOval(sceneFill(CountryHillDark, p, 0.55f), Offset(w * 0.40f, h * 0.72f), Size(w * 0.8f, h * 0.40f))
    drawFarmhouse(p, Offset(w * 0.16f, h * 0.82f), u * 0.20f, phase, muted)
    drawWindmill(p, Offset(w * 0.78f, h * 0.86f), u * 0.20f, phase, muted)
    drawCropRows(p, 0f, w, h * 0.90f, h * 0.12f, phase, muted)
    drawFence(p, 0f, h * 0.70f, w, h * 0.11f, muted)
    drawPersonFigure(p, Offset(w * 0.50f, h * 0.84f), u * 0.16f, phase + 0.2f, muted, detail = 0.45f)
    drawCatFigure(p, Offset(w * 0.36f, h * 0.92f), u * 0.13f, phase + 0.4f, muted, detail = 0.4f)
    drawDogFigure(p, Offset(w * 0.62f, h * 0.94f), u * 0.11f, phase + 0.3f, muted, fillColor = DogCream, floppy = true)
    drawBird(p, Offset(w * 0.32f, h * 0.26f), u * 0.026f, phase, muted)
    drawBird(p, Offset(w * 0.42f, h * 0.21f), u * 0.020f, phase + 0.3f, muted)
}

internal fun DrawScope.drawCountrysideHalo(
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
    drawWindmill(p, Offset(topXs[0], barTop + u * 0.30f), u * 0.55f, phase, muted)
    drawFarmhouse(p, Offset(topXs[1], barTop + u * 0.30f), u * 0.50f, phase, muted)
    drawPersonFigure(p, Offset(topXs[2], barTop - u * 0.20f), u * 0.62f, phase + 0.2f, muted, detail = 1f)
    drawDogFigure(p, Offset(topXs[3], barTop - u * 0.10f), u * 0.52f, phase + 0.3f, muted, fillColor = DogCream, floppy = true)
    drawCropRows(p, bottomXs[0] - u, bottomXs[2] + u, barBottom + u * 0.28f, u * 0.48f, phase, muted)
    drawFence(p, bottomXs[0] - u, barBottom + u * 0.40f, (bottomXs[2] - bottomXs[0]) + u * 2f, u * 0.50f, muted)
    drawCatFigure(p, Offset(bottomXs[1], barBottom + u * 0.48f), u * 0.46f, phase + 0.4f, muted, detail = 1f)
    drawDogFigure(p, Offset(bottomXs[2], barBottom + u * 0.52f), u * 0.40f, phase + 0.5f, muted, fillColor = DogGray, floppy = false)
}

internal fun DrawScope.drawDogsBackdrop(p: ScenePalette, phase: Float, muted: Float) {
    val w = size.width
    val h = size.height
    if (w < 2f || h < 2f) return
    val u = min(w, h)
    drawRect(
        brush = Brush.verticalGradient(
            0f to sceneFill(ParkSky, p, (muted * 0.4f).coerceAtLeast(0.22f)),
            0.62f to p.base,
            1f to sceneFill(ParkHill, p, (muted * 0.3f).coerceAtLeast(0.18f)),
        ),
        size = size,
    )
    drawSun(p, Offset(w * 0.82f, h * 0.15f), u * 0.09f, phase, muted)
    drawCloud(p, Offset(w * 0.22f, h * 0.14f), u * 0.15f, phase, muted)
    drawOval(sceneFill(ParkHill, p, 0.48f), Offset(-w * 0.1f, h * 0.68f), Size(w * 0.85f, h * 0.42f))
    drawOval(sceneFill(ParkHill, p, 0.40f), Offset(w * 0.45f, h * 0.74f), Size(w * 0.8f, h * 0.38f))
    drawRoundTree(p, Offset(w * 0.10f, h * 0.84f), u * 0.24f, Color(0xFF8FBF7A), phase, muted)
    drawRoundTree(p, Offset(w * 0.90f, h * 0.82f), u * 0.22f, Color(0xFF8FBF7A), phase + 0.3f, muted)
    drawFence(p, 0f, h * 0.72f, w, h * 0.12f, muted)
    drawDogFigure(p, Offset(w * 0.14f, h * 0.90f), u * 0.14f, phase, muted, fillColor = DogBrown, floppy = true)
    drawDogFigure(p, Offset(w * 0.34f, h * 0.92f), u * 0.12f, phase + 0.2f, muted, fillColor = DogCream, floppy = true)
    drawDogFigure(p, Offset(w * 0.56f, h * 0.94f), u * 0.11f, phase + 0.4f, muted, fillColor = DogGray, floppy = false)
    drawDogFigure(p, Offset(w * 0.78f, h * 0.90f), u * 0.13f, phase + 0.6f, muted, fillColor = DogDark, floppy = true)
    drawBall(p, Offset(w * 0.46f, h * 0.96f), u * 0.035f, phase, muted)
    drawPersonFigure(p, Offset(w * 0.64f, h * 0.82f), u * 0.16f, phase + 0.3f, muted, detail = 0.45f)
    drawBench(p, Offset(w * 0.86f, h * 0.88f), u * 0.12f, phase, muted)
    drawFlowerCluster(p, Offset(w * 0.04f, h * 0.99f), u * 0.07f, phase + 0.4f, muted)
}

internal fun DrawScope.drawDogsHalo(
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
    drawDogFigure(p, Offset(topXs[0], barTop - u * 0.10f), u * 0.54f, phase, muted, fillColor = DogBrown, floppy = true)
    drawDogFigure(p, Offset(topXs[1], barTop - u * 0.10f), u * 0.48f, phase + 0.3f, muted, fillColor = DogCream, floppy = false)
    drawPersonFigure(p, Offset(topXs[2], barTop - u * 0.20f), u * 0.62f, phase + 0.2f, muted, detail = 1f)
    drawBall(p, Offset(topXs[3], barTop - u * 0.42f), u * 0.34f, phase, muted)
    drawDogFigure(p, Offset(bottomXs[0], barBottom + u * 0.48f), u * 0.46f, phase + 0.4f, muted, fillColor = DogGray, floppy = true)
    drawBall(p, Offset(bottomXs[1], barBottom + u * 0.46f), u * 0.32f, phase + 0.2f, muted)
    drawFlowerCluster(p, Offset(bottomXs[2], barBottom + u * 0.42f), u * 0.34f, phase + 0.5f, muted, hanging = true)
}
