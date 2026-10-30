// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 书房/灶台/猫群主题。书房=书架/窗/书桌/台灯/摊开的书；灶台=灶眼/锅/蒸汽/
//! 罐子；猫群=多只不同毛色的猫 + 毛线球/纸箱/坐垫。

package xyz.erik.opi.scene

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import kotlin.math.min

private val WallCream = Color(0xFFF2E8D8)
private val FloorWood = Color(0xFFB58A5A)
private val KitchenTile = Color(0xFFDCE8E8)
private val KitchenCounter = Color(0xFF9FBBC4)
private val CatGray = Color(0xFFB8AEA6)
private val CatBlack = Color(0xFF6B6468)
private val CatWhite = Color(0xFFF4F0E6)
private val CatCream = Color(0xFFF2D9B8)
private val RugPurple = Color(0xFFB98BC9)

internal fun DrawScope.drawStudyBackdrop(p: ScenePalette, phase: Float, muted: Float) {
    val w = size.width
    val h = size.height
    if (w < 2f || h < 2f) return
    val u = min(w, h)
    drawRect(
        brush = Brush.verticalGradient(
            0f to sceneFill(WallCream, p, (muted * 0.4f).coerceAtLeast(0.22f)),
            0.78f to p.base,
            1f to sceneFill(FloorWood, p, (muted * 0.3f).coerceAtLeast(0.18f)),
        ),
        size = size,
    )
    drawBookshelf(p, w * 0.04f, h * 0.76f, w * 0.30f, h * 0.50f, phase, muted)
    drawWindow(p, Offset(w * 0.82f, h * 0.32f), u * 0.12f, phase, muted, night = true)
    drawDesk(p, Offset(w * 0.54f, h * 0.80f), u * 0.24f, phase, muted)
    drawOpenBook(p, Offset(w * 0.50f, h * 0.78f), u * 0.07f, phase, muted)
    drawDeskLamp(p, Offset(w * 0.68f, h * 0.77f), u * 0.08f, phase, muted)
    drawCatFigure(p, Offset(w * 0.72f, h * 0.76f), u * 0.10f, phase + 0.3f, muted, detail = 0.4f, fillColor = CatGray)
    drawPersonFigure(p, Offset(w * 0.42f, h * 0.76f), u * 0.16f, phase, muted, detail = 0.45f)
    drawFlowerCluster(p, Offset(w * 0.95f, h * 0.77f), u * 0.07f, phase + 0.3f, muted)
}

internal fun DrawScope.drawStudyHalo(
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
    drawBookshelf(p, topXs[0] - u * 0.80f, barTop + u * 0.30f, u * 1.6f, u * 0.90f, phase, muted)
    drawWindow(p, Offset(topXs[1], barTop - u * 0.42f), u * 0.34f, phase, muted, night = true)
    drawPersonFigure(p, Offset(topXs[2], barTop - u * 0.20f), u * 0.62f, phase, muted, detail = 1f)
    drawDeskLamp(p, Offset(topXs[3], barTop - u * 0.05f), u * 0.48f, phase, muted)
    drawOpenBook(p, Offset(bottomXs[0], barBottom + u * 0.44f), u * 0.40f, phase, muted)
    drawCatFigure(p, Offset(bottomXs[1], barBottom + u * 0.48f), u * 0.46f, phase + 0.3f, muted, detail = 1f, fillColor = CatGray)
    drawFlowerCluster(p, Offset(bottomXs[2], barBottom + u * 0.42f), u * 0.34f, phase + 0.3f, muted, hanging = true)
}

internal fun DrawScope.drawKitchenBackdrop(p: ScenePalette, phase: Float, muted: Float) {
    val w = size.width
    val h = size.height
    if (w < 2f || h < 2f) return
    val u = min(w, h)
    drawRect(
        brush = Brush.verticalGradient(
            0f to sceneFill(KitchenTile, p, (muted * 0.35f).coerceAtLeast(0.2f)),
            0.78f to p.base,
            1f to sceneFill(KitchenCounter, p, (muted * 0.3f).coerceAtLeast(0.18f)),
        ),
        size = size,
    )
    drawRect(sceneFill(KitchenCounter, p, (muted * 0.35f).coerceAtLeast(0.2f)), Offset(0f, h * 0.76f), Size(w, h * 0.24f))
    drawStove(p, Offset(w * 0.46f, h * 0.82f), u * 0.20f, phase, muted)
    drawPot(p, Offset(w * 0.46f, h * 0.72f), u * 0.085f, phase, muted)
    drawSteam(p, Offset(w * 0.46f, h * 0.66f), u * 0.075f, phase, muted)
    drawJar(p, Offset(w * 0.12f, h * 0.72f), u * 0.055f, phase, muted)
    drawJar(p, Offset(w * 0.20f, h * 0.71f), u * 0.045f, phase + 0.2f, muted)
    drawCarrot(p, Offset(w * 0.62f, h * 0.82f), u * 0.055f, phase, muted)
    drawTomatoPlant(p, Offset(w * 0.70f, h * 0.80f), u * 0.07f, phase + 0.4f, muted)
    drawPersonFigure(p, Offset(w * 0.28f, h * 0.84f), u * 0.16f, phase + 0.2f, muted, detail = 0.45f)
    drawCatFigure(p, Offset(w * 0.84f, h * 0.86f), u * 0.14f, phase + 0.4f, muted, detail = 0.4f)
}

internal fun DrawScope.drawKitchenHalo(
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
    drawPot(p, Offset(topXs[0], barTop - u * 0.10f), u * 0.52f, phase, muted)
    drawSteam(p, Offset(topXs[0], barTop - u * 0.48f), u * 0.38f, phase, muted)
    drawStove(p, Offset(topXs[1], barTop + u * 0.30f), u * 0.55f, phase, muted)
    drawPersonFigure(p, Offset(topXs[2], barTop - u * 0.20f), u * 0.62f, phase + 0.2f, muted, detail = 1f)
    drawJar(p, Offset(topXs[3], barTop - u * 0.12f), u * 0.42f, phase + 0.2f, muted)
    drawCatFigure(p, Offset(bottomXs[0], barBottom + u * 0.48f), u * 0.46f, phase + 0.4f, muted, detail = 1f)
    drawCarrot(p, Offset(bottomXs[1], barBottom + u * 0.42f), u * 0.40f, phase, muted)
    drawJar(p, Offset(bottomXs[2], barBottom + u * 0.46f), u * 0.34f, phase + 0.5f, muted)
}

internal fun DrawScope.drawCatsBackdrop(p: ScenePalette, phase: Float, muted: Float) {
    val w = size.width
    val h = size.height
    if (w < 2f || h < 2f) return
    val u = min(w, h)
    drawRect(
        brush = Brush.verticalGradient(
            0f to sceneFill(WallCream, p, (muted * 0.4f).coerceAtLeast(0.22f)),
            0.76f to p.base,
            1f to sceneFill(FloorWood, p, (muted * 0.3f).coerceAtLeast(0.18f)),
        ),
        size = size,
    )
    drawOval(sceneFill(RugPurple, p, 0.45f), Offset(w * 0.08f, h * 0.82f), Size(w * 0.84f, h * 0.24f))
    drawWindow(p, Offset(w * 0.84f, h * 0.30f), u * 0.10f, phase, muted, night = false)
    drawCushion(p, Offset(w * 0.22f, h * 0.88f), u * 0.14f, phase, muted)
    drawBox(p, Offset(w * 0.66f, h * 0.90f), u * 0.10f, phase, muted)
    drawCatFigure(p, Offset(w * 0.12f, h * 0.84f), u * 0.18f, phase, muted, detail = 0.45f, fillColor = CatGray)
    drawCatFigure(p, Offset(w * 0.40f, h * 0.90f), u * 0.20f, phase + 0.3f, muted, detail = 0.45f, fillColor = CatCream)
    drawCatFigure(p, Offset(w * 0.88f, h * 0.90f), u * 0.15f, phase + 0.5f, muted, detail = 0.4f, fillColor = CatWhite)
    drawKitten(p, Offset(w * 0.58f, h * 0.86f), u * 0.13f, phase + 0.2f, muted, fillColor = CatBlack)
    drawYarnBall(p, Offset(w * 0.30f, h * 0.96f), u * 0.055f, phase, muted)
    drawFlowerCluster(p, Offset(w * 0.05f, h * 0.99f), u * 0.07f, phase + 0.4f, muted)
}

internal fun DrawScope.drawCatsHalo(
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
    drawCatFigure(p, Offset(topXs[0], barTop - u * 0.30f), u * 0.66f, phase, muted, detail = 1f, fillColor = CatGray)
    drawKitten(p, Offset(topXs[1], barTop - u * 0.28f), u * 0.55f, phase + 0.3f, muted, fillColor = CatBlack)
    drawYarnBall(p, Offset(topXs[2], barTop - u * 0.42f), u * 0.32f, phase, muted)
    drawCatFigure(p, Offset(topXs[3], barTop - u * 0.28f), u * 0.62f, phase + 0.5f, muted, detail = 1f, fillColor = CatWhite)
    drawCatFigure(p, Offset(bottomXs[0], barBottom + u * 0.48f), u * 0.46f, phase + 0.2f, muted, detail = 1f, fillColor = CatCream)
    drawBox(p, Offset(bottomXs[1], barBottom + u * 0.52f), u * 0.34f, phase, muted)
    drawYarnBall(p, Offset(bottomXs[2], barBottom + u * 0.44f), u * 0.32f, phase + 0.4f, muted)
}
