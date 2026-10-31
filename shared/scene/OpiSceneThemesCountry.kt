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
            0f to sceneFill(CountrySky, p, (muted * 0.4f).coerceAtLeast(0.20f)),
            0.62f to p.base,
            1f to sceneFill(CountryHill, p, (muted * 0.28f).coerceAtLeast(0.16f)),
        ),
        size = size,
    )
    drawSun(p, Offset(w * 0.82f, h * 0.16f), u * 0.09f, phase, muted)
    drawCloud(p, Offset(w * 0.20f, h * 0.14f), u * 0.18f, phase, muted)
    drawCloud(p, Offset(w * 0.58f, h * 0.09f), u * 0.12f, phase + 0.4f, muted)
    // 远景树线。
    drawTree(p, Offset(w * 0.06f, h * 0.72f), u * 0.30f, phase, muted, BotanyLeafDark, BotanyLeafDark)
    drawTree(p, Offset(w * 0.94f, h * 0.74f), u * 0.28f, phase + 0.3f, muted, BotanyLeafDark, BotanyLeafDark)
    drawOval(sceneFill(CountryHill, p, 0.48f), Offset(-w * 0.1f, h * 0.66f), Size(w * 0.75f, h * 0.42f))
    drawOval(sceneFill(CountryHillDark, p, 0.55f), Offset(w * 0.40f, h * 0.72f), Size(w * 0.8f, h * 0.40f))
    drawFarmhouse(p, Offset(w * 0.16f, h * 0.84f), u * 0.22f, phase, muted)
    drawWindmill(p, Offset(w * 0.78f, h * 0.88f), u * 0.22f, phase, muted)
    drawCropRows(p, 0f, w, h * 0.90f, h * 0.12f, phase, muted)
    drawFence(p, 0f, h * 0.70f, w, h * 0.11f, muted)
    // 花枝/花藤：农舍旁与栅栏边都补上。
    drawFloweringBranch(
        p, Offset(w * 0.02f, h * 0.62f), Offset(w * 0.22f, h * 0.48f),
        u * 0.34f, phase, muted, BlossomPink, seed = 0.2f,
    )
    drawClimbingVine(
        p, Offset(w * 0.30f, h * 0.98f), Offset(w * 0.28f, h * 0.62f),
        u * 0.34f, phase, muted, BlossomWhite, flowers = 2, seed = 0.5f,
    )
    for ((i, x) in listOf(0.08f, 0.42f, 0.64f, 0.92f).withIndex()) {
        drawFlowerBed(
            p, Offset(w * x, h * 0.99f), u * 0.18f, u * 0.11f,
            phase + i * 0.2f, muted, listOf(BlossomWhite, BlossomYellow, BlossomPink), count = 4,
        )
    }
    val personC = Offset(w * 0.50f, h * 0.86f)
    drawContactShadow(p, Offset(personC.x, personC.y + u * 0.13f), u * 0.22f, muted)
    drawContactShadow(p, Offset(w * 0.36f, h * 0.95f + u * 0.10f), u * 0.18f, muted)
    drawContactShadow(p, Offset(w * 0.62f, h * 0.96f + u * 0.08f), u * 0.16f, muted)
    drawPersonFigure(p, personC, u * 0.18f, phase + 0.2f, muted, detail = 0.5f)
    drawCatFigure(p, Offset(w * 0.36f, h * 0.95f), u * 0.15f, phase + 0.4f, muted, detail = 0.5f)
    drawDogFigure(p, Offset(w * 0.62f, h * 0.96f), u * 0.13f, phase + 0.3f, muted, fillColor = DogCream, floppy = true)
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
            0f to sceneFill(ParkSky, p, (muted * 0.4f).coerceAtLeast(0.20f)),
            0.62f to p.base,
            1f to sceneFill(ParkHill, p, (muted * 0.28f).coerceAtLeast(0.16f)),
        ),
        size = size,
    )
    drawSun(p, Offset(w * 0.82f, h * 0.15f), u * 0.09f, phase, muted)
    drawCloud(p, Offset(w * 0.22f, h * 0.14f), u * 0.17f, phase, muted)
    drawCloud(p, Offset(w * 0.62f, h * 0.09f), u * 0.12f, phase + 0.4f, muted)
    drawOval(sceneFill(ParkHill, p, 0.48f), Offset(-w * 0.1f, h * 0.68f), Size(w * 0.85f, h * 0.42f))
    drawOval(sceneFill(ParkHill, p, 0.40f), Offset(w * 0.45f, h * 0.74f), Size(w * 0.8f, h * 0.38f))
    drawTree(p, Offset(w * 0.10f, h * 0.88f), u * 0.34f, phase, muted, BotanyLeaf, BotanyLeafDark, BlossomPink)
    drawTree(p, Offset(w * 0.90f, h * 0.86f), u * 0.30f, phase + 0.3f, muted, BotanyLeaf, BotanyLeafDark, BlossomWhite)
    drawFence(p, 0f, h * 0.72f, w, h * 0.12f, muted)
    // 花枝/花藤：公园的树与栅栏上都开花。
    drawFloweringBranch(
        p, Offset(w * 0.02f, h * 0.60f), Offset(w * 0.22f, h * 0.44f),
        u * 0.32f, phase, muted, BlossomPink, seed = 0.1f,
    )
    drawClimbingVine(
        p, Offset(w * 0.70f, h * 0.98f), Offset(w * 0.68f, h * 0.70f),
        u * 0.30f, phase, muted, BlossomWhite, flowers = 2, seed = 0.6f,
    )
    for ((i, x) in listOf(0.20f, 0.46f, 0.70f, 0.94f).withIndex()) {
        drawFlowerBed(
            p, Offset(w * x, h * 0.99f), u * 0.18f, u * 0.10f,
            phase + i * 0.2f, muted, listOf(BlossomYellow, BlossomPink, BlossomWhite), count = 4,
        )
    }
    // 狗群与球：全部踩在草地上。
    for ((i, spec) in listOf(
        Triple(0.14f, 0.17f, DogBrown), Triple(0.34f, 0.15f, DogCream),
        Triple(0.56f, 0.14f, DogGray), Triple(0.78f, 0.16f, DogDark),
    ).withIndex()) {
        drawContactShadow(p, Offset(w * spec.first, h * 0.93f + u * 0.10f), u * 0.20f, muted)
        drawDogFigure(
            p, Offset(w * spec.first, h * 0.94f), u * spec.second,
            phase + i * 0.2f, muted, fillColor = spec.third, floppy = i % 2 == 0,
        )
    }
    drawBall(p, Offset(w * 0.46f, h * 0.98f), u * 0.04f, phase, muted)
    val personC = Offset(w * 0.66f, h * 0.84f)
    drawPersonFigure(p, personC, u * 0.18f, phase + 0.3f, muted, detail = 0.5f)
    drawContactShadow(p, Offset(personC.x, personC.y + u * 0.12f), u * 0.22f, muted)
    drawBench(p, Offset(w * 0.86f, h * 0.90f), u * 0.14f, phase, muted)
    drawContactShadow(p, Offset(w * 0.86f, h * 0.96f), u * 0.20f, muted)
    drawBird(p, Offset(w * 0.36f, h * 0.24f), u * 0.024f, phase, muted)
    drawBird(p, Offset(w * 0.46f, h * 0.19f), u * 0.019f, phase + 0.3f, muted)
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
