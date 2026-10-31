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
            0f to sceneFill(AutumnSky, p, (muted * 0.45f).coerceAtLeast(0.22f)),
            0.62f to p.base,
            1f to sceneFill(AutumnGround, p, (muted * 0.32f).coerceAtLeast(0.20f)),
        ),
        size = size,
    )
    drawSun(p, Offset(w * 0.18f, h * 0.16f), u * 0.10f, phase, muted)
    drawCloud(p, Offset(w * 0.74f, h * 0.13f), u * 0.16f, phase, muted)
    drawCloud(p, Offset(w * 0.46f, h * 0.08f), u * 0.11f, phase + 0.4f, muted)
    drawOval(sceneFill(AutumnGround, p, 0.5f), Offset(-w * 0.1f, h * 0.68f), Size(w * 0.9f, h * 0.45f))
    drawOval(sceneFill(AutumnGround, p, 0.42f), Offset(w * 0.42f, h * 0.74f), Size(w * 0.9f, h * 0.42f))
    // 真正的树干 + 树冠，树不再是一个个悬空的圆。
    drawTree(p, Offset(w * 0.10f, h * 0.90f), u * 0.40f, phase, muted, AutumnLeaf, Color(0xFFC9673C))
    drawTree(p, Offset(w * 0.88f, h * 0.88f), u * 0.36f, phase + 0.4f, muted, AutumnLeaf, Color(0xFFC9673C))
    drawTree(p, Offset(w * 0.27f, h * 0.84f), u * 0.26f, phase + 0.2f, muted, Color(0xFFE9B25E), Color(0xFFC9673C))
    drawTree(p, Offset(w * 0.70f, h * 0.86f), u * 0.28f, phase + 0.5f, muted, Color(0xFFE9B25E), Color(0xFFC9673C))
    // 秋日花枝与花圃。
    drawFloweringBranch(
        p, Offset(-w * 0.02f, h * 0.10f), Offset(w * 0.22f, h * 0.24f),
        u * 0.34f, phase, muted, BlossomPink, seed = 0.2f,
    )
    drawFloweringBranch(
        p, Offset(w * 1.02f, h * 0.08f), Offset(w * 0.78f, h * 0.22f),
        u * 0.32f, phase + 0.4f, muted, BlossomWhite, seed = 0.7f,
    )
    for ((i, x) in listOf(0.34f, 0.58f, 0.94f).withIndex()) {
        drawFlowerBed(
            p, Offset(w * x, h * 0.99f), u * 0.16f, u * 0.09f,
            phase + i * 0.2f, muted, listOf(BlossomYellow, BlossomRed, BlossomWhite), count = 3,
        )
    }
    // 南瓜与蘑菇：都有接地阴影。
    for ((i, spec) in listOf(
        Triple(0.17f, 0.09f, 0.0f), Triple(0.24f, 0.07f, 0.3f), Triple(0.82f, 0.08f, 0.5f),
    ).withIndex()) {
        drawContactShadow(p, Offset(w * spec.first, h * 0.99f), u * 0.12f, muted)
        drawPumpkin(p, Offset(w * spec.first, h * 0.98f), u * spec.second, phase + spec.third, muted)
    }
    drawContactShadow(p, Offset(w * 0.40f, h * 0.99f), u * 0.08f, muted)
    drawContactShadow(p, Offset(w * 0.56f, h * 1.0f), u * 0.07f, muted)
    drawMushroom(p, Offset(w * 0.40f, h * 0.98f), u * 0.07f, phase, muted)
    drawMushroom(p, Offset(w * 0.56f, h * 0.99f), u * 0.06f, phase + 0.4f, muted)
    val personC = Offset(w * 0.50f, h * 0.86f)
    drawContactShadow(p, Offset(personC.x, personC.y + u * 0.12f), u * 0.22f, muted)
    drawContactShadow(p, Offset(w * 0.62f, h * 0.94f + u * 0.10f), u * 0.18f, muted)
    drawPersonFigure(p, personC, u * 0.18f, phase + 0.2f, muted, detail = 0.5f)
    drawCatFigure(p, Offset(w * 0.62f, h * 0.94f), u * 0.16f, phase + 0.4f, muted, detail = 0.5f)
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
            0f to sceneFill(WinterSky, p, (muted * 0.35f).coerceAtLeast(0.18f)),
            0.62f to p.base,
            1f to sceneFill(WinterGround, p, (muted * 0.22f).coerceAtLeast(0.14f)),
        ),
        size = size,
    )
    drawCloud(p, Offset(w * 0.70f, h * 0.14f), u * 0.16f, phase, muted)
    drawCloud(p, Offset(w * 0.28f, h * 0.09f), u * 0.11f, phase + 0.4f, muted)
    drawOval(sceneFill(WinterGround, p, 0.20f), Offset(-w * 0.1f, h * 0.70f), Size(w * 1.2f, h * 0.45f))
    drawOval(sceneFill(WinterShadow, p, 0.30f), Offset(w * 0.30f, h * 0.82f), Size(w * 0.8f, h * 0.28f))
    drawBareTree(p, Offset(w * 0.10f, h * 0.92f), u * 0.38f, phase, muted)
    drawBareTree(p, Offset(w * 0.90f, h * 0.90f), u * 0.34f, phase + 0.4f, muted)
    // 冬日花枝：枯枝上的梅花，画面立刻有生气。
    drawFloweringBranch(
        p, Offset(w * 0.02f, h * 0.10f), Offset(w * 0.22f, h * 0.24f),
        u * 0.32f, phase, muted, BlossomPink, seed = 0.2f,
    )
    drawFloweringBranch(
        p, Offset(w * 0.98f, h * 0.08f), Offset(w * 0.78f, h * 0.22f),
        u * 0.30f, phase + 0.4f, muted, BlossomWhite, seed = 0.7f,
    )
    drawPine(p, Offset(w * 0.20f, h * 0.96f), u * 0.28f, phase + 0.2f, muted)
    drawPine(p, Offset(w * 0.80f, h * 0.94f), u * 0.26f, phase + 0.5f, muted)
    drawContactShadow(p, Offset(w * 0.32f, h * 0.94f), u * 0.14f, muted)
    drawContactShadow(p, Offset(w * 0.72f, h * 0.96f), u * 0.12f, muted)
    drawSnowman(p, Offset(w * 0.32f, h * 0.92f), u * 0.14f, phase, muted)
    drawSnowman(p, Offset(w * 0.72f, h * 0.94f), u * 0.11f, phase + 0.3f, muted)
    drawFlowerBed(
        p, Offset(w * 0.50f, h * 0.99f), u * 0.16f, u * 0.09f,
        phase + 0.3f, muted, listOf(BlossomWhite, BlossomPink, BlossomYellow), count = 3,
    )
    val personC = Offset(w * 0.52f, h * 0.86f)
    drawContactShadow(p, Offset(personC.x, personC.y + u * 0.12f), u * 0.22f, muted)
    drawContactShadow(p, Offset(w * 0.62f, h * 0.94f + u * 0.09f), u * 0.18f, muted)
    drawPersonFigure(p, personC, u * 0.18f, phase + 0.2f, muted, detail = 0.5f)
    drawCatFigure(p, Offset(w * 0.62f, h * 0.94f), u * 0.15f, phase + 0.4f, muted, detail = 0.5f)
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
