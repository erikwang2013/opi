// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 秋色与冬雪主题。秋色 = 暖橙树冠/南瓜/落叶/蘑菇；冬雪 = 雪地/雪人/枯树/
//! 松树/雪花。小人/猫/花草仍是同一批角色。

package xyz.erik.opi.scene

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import kotlin.math.min

private val AutumnSky = Color(0xFFF6D9A8)
private val AutumnGround = Color(0xFFB58A5A)
private val AutumnLeaf = Color(0xFFE8824E)
private val WinterSky = Color(0xFFDCEAF5)
private val WinterGround = Color(0xFFF2F7FC)
private val WinterShadow = Color(0xFFDDE8F2)

internal fun DrawScope.drawAutumnBackdrop(p: ScenePalette, phase: Float, muted: Float) {
    val w = size.width
    val h = size.height
    if (w < 2f || h < 2f) return
    val u = min(w, h)
    drawRect(
        brush = Brush.verticalGradient(
            0f to sceneFill(AutumnSky, p, (muted * 0.45f).coerceAtLeast(0.25f)),
            0.62f to p.base,
            1f to sceneFill(AutumnGround, p, (muted * 0.35f).coerceAtLeast(0.22f)),
        ),
        size = size,
    )
    drawSun(p, Offset(w * 0.18f, h * 0.16f), u * 0.09f, phase, muted)
    drawCloud(p, Offset(w * 0.74f, h * 0.13f), u * 0.14f, phase, muted)
    drawOval(sceneFill(AutumnGround, p, 0.5f), Offset(-w * 0.1f, h * 0.68f), Size(w * 0.9f, h * 0.45f))
    drawOval(sceneFill(AutumnGround, p, 0.42f), Offset(w * 0.42f, h * 0.74f), Size(w * 0.9f, h * 0.42f))
    drawRoundTree(p, Offset(w * 0.10f, h * 0.86f), u * 0.30f, AutumnLeaf, phase, muted)
    drawRoundTree(p, Offset(w * 0.88f, h * 0.84f), u * 0.26f, AutumnLeaf, phase + 0.4f, muted)
    drawRoundTree(p, Offset(w * 0.27f, h * 0.76f), u * 0.18f, AutumnLeaf, phase + 0.2f, muted)
    drawRoundTree(p, Offset(w * 0.70f, h * 0.78f), u * 0.20f, AutumnLeaf, phase + 0.5f, muted)
    drawPumpkin(p, Offset(w * 0.17f, h * 0.96f), u * 0.075f, phase, muted)
    drawPumpkin(p, Offset(w * 0.24f, h * 0.98f), u * 0.055f, phase + 0.3f, muted)
    drawPumpkin(p, Offset(w * 0.82f, h * 0.97f), u * 0.065f, phase + 0.5f, muted)
    drawMushroom(p, Offset(w * 0.40f, h * 0.97f), u * 0.06f, phase, muted)
    drawMushroom(p, Offset(w * 0.56f, h * 0.99f), u * 0.05f, phase + 0.4f, muted)
    drawPersonFigure(p, Offset(w * 0.50f, h * 0.82f), u * 0.17f, phase + 0.2f, muted, detail = 0.45f)
    drawCatFigure(p, Offset(w * 0.62f, h * 0.90f), u * 0.14f, phase + 0.4f, muted, detail = 0.4f)
    drawFlowerCluster(p, Offset(w * 0.94f, h * 0.99f), u * 0.08f, phase + 0.2f, muted)
    drawFallingLeaves(p, w, h, phase, muted)
}

internal fun DrawScope.drawAutumnHalo(
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
    drawRoundTree(p, Offset(topXs[0], barTop + u * 0.32f), u * 0.62f, AutumnLeaf, phase, muted)
    drawPumpkin(p, Offset(topXs[1], barTop - u * 0.12f), u * 0.46f, phase + 0.3f, muted)
    drawPersonFigure(p, Offset(topXs[2], barTop - u * 0.20f), u * 0.62f, phase + 0.2f, muted, detail = 1f)
    drawFallingLeaves(p, size.width, barTop, phase, muted)
    drawPumpkin(p, Offset(bottomXs[0], barBottom + u * 0.46f), u * 0.42f, phase + 0.5f, muted)
    drawMushroom(p, Offset(bottomXs[1], barBottom + u * 0.42f), u * 0.42f, phase, muted)
    drawPumpkin(p, Offset(bottomXs[2], barBottom + u * 0.48f), u * 0.36f, phase + 0.7f, muted)
}

internal fun DrawScope.drawWinterBackdrop(p: ScenePalette, phase: Float, muted: Float) {
    val w = size.width
    val h = size.height
    if (w < 2f || h < 2f) return
    val u = min(w, h)
    drawRect(
        brush = Brush.verticalGradient(
            0f to sceneFill(WinterSky, p, (muted * 0.35f).coerceAtLeast(0.2f)),
            0.62f to p.base,
            1f to sceneFill(WinterGround, p, (muted * 0.25f).coerceAtLeast(0.15f)),
        ),
        size = size,
    )
    drawCloud(p, Offset(w * 0.70f, h * 0.14f), u * 0.14f, phase, muted)
    drawOval(sceneFill(WinterGround, p, 0.20f), Offset(-w * 0.1f, h * 0.70f), Size(w * 1.2f, h * 0.45f))
    drawOval(sceneFill(WinterShadow, p, 0.30f), Offset(w * 0.30f, h * 0.82f), Size(w * 0.8f, h * 0.28f))
    drawBareTree(p, Offset(w * 0.10f, h * 0.90f), u * 0.34f, phase, muted)
    drawBareTree(p, Offset(w * 0.90f, h * 0.88f), u * 0.30f, phase + 0.4f, muted)
    drawPine(p, Offset(w * 0.20f, h * 0.94f), u * 0.24f, phase + 0.2f, muted)
    drawPine(p, Offset(w * 0.80f, h * 0.92f), u * 0.22f, phase + 0.5f, muted)
    drawSnowman(p, Offset(w * 0.32f, h * 0.88f), u * 0.12f, phase, muted)
    drawSnowman(p, Offset(w * 0.72f, h * 0.90f), u * 0.09f, phase + 0.3f, muted)
    drawPersonFigure(p, Offset(w * 0.52f, h * 0.84f), u * 0.17f, phase + 0.2f, muted, detail = 0.45f)
    drawCatFigure(p, Offset(w * 0.62f, h * 0.92f), u * 0.13f, phase + 0.4f, muted, detail = 0.4f)
    drawSnowflakes(p, w, h, phase, muted)
}

internal fun DrawScope.drawWinterHalo(
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
    drawSnowman(p, Offset(topXs[0], barTop + u * 0.30f), u * 0.58f, phase, muted)
    drawBareTree(p, Offset(topXs[1], barTop + u * 0.30f), u * 0.82f, phase + 0.4f, muted)
    drawPersonFigure(p, Offset(topXs[2], barTop - u * 0.20f), u * 0.62f, phase + 0.2f, muted, detail = 1f)
    drawSnowflakes(p, size.width, barTop, phase, muted)
    drawOval(sceneFill(WinterGround, p, 0.20f), Offset(bottomXs[0] - u, barBottom + u * 0.40f), Size((bottomXs[2] - bottomXs[0]) + u * 2f, u * 0.50f))
    drawSnowman(p, Offset(bottomXs[0], barBottom + u * 0.56f), u * 0.34f, phase + 0.3f, muted)
    drawPine(p, Offset(bottomXs[1], barBottom + u * 0.62f), u * 0.42f, phase, muted)
    drawBareTree(p, Offset(bottomXs[2], barBottom + u * 0.62f), u * 0.40f, phase + 0.5f, muted)
}
