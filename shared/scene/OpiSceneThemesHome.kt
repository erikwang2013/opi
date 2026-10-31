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
            0f to sceneFill(WallCream, p, (muted * 0.4f).coerceAtLeast(0.18f)),
            0.78f to p.base,
            1f to sceneFill(FloorWood, p, (muted * 0.26f).coerceAtLeast(0.15f)),
        ),
        size = size,
    )
    // 墙面挂画 + 吊兰，填住上半幅。
    for ((i, spec) in listOf(
        Triple(0.40f, 0.16f, BlossomPink), Triple(0.52f, 0.22f, BlossomYellow), Triple(0.31f, 0.26f, BlossomWhite),
    ).withIndex()) {
        val cx = w * spec.first
        val cy = h * spec.second
        val fw = u * 0.11f
        val fh = u * 0.14f
        drawRoundRect(sceneFill(Color(0xFFB58A5A), p, muted), Offset(cx - fw / 2, cy - fh / 2), Size(fw, fh), androidx.compose.ui.geometry.CornerRadius(u * 0.008f))
        drawRect(sceneFill(Color(0xFFFFF4E6), p, muted), Offset(cx - fw * 0.40f, cy - fh * 0.40f), Size(fw * 0.80f, fh * 0.80f))
        drawBlossom(p, Offset(cx, cy), u * 0.030f, phase + i * 0.2f, muted, spec.third)
    }
    // 大挂画：书房的视觉中心。
    drawRoundRect(sceneFill(Color(0xFFB58A5A), p, muted), Offset(w * 0.43f, h * 0.25f), Size(w * 0.14f, u * 0.16f), androidx.compose.ui.geometry.CornerRadius(u * 0.008f))
    drawRect(sceneFill(Color(0xFFBFE6EF), p, muted), Offset(w * 0.43f + u * 0.012f, h * 0.25f + u * 0.012f), Size(w * 0.14f - u * 0.024f, u * 0.16f - u * 0.024f))
    drawSun(p, Offset(w * 0.50f, h * 0.25f + u * 0.06f), u * 0.025f, phase, muted)
    drawFlowerBed(p, Offset(w * 0.50f, h * 0.25f + u * 0.14f), u * 0.13f, u * 0.05f, phase, muted, listOf(BlossomPink, BlossomYellow, BlossomWhite), count = 4)
    // 窗帘。
    for (dir in listOf(-1f, 1f)) {
        drawRoundRect(
            sceneFill(Color(0xFFB98BC9), p, (muted + 0.15f).coerceAtMost(0.7f)),
            Offset(w * 0.82f + dir * u * 0.20f - (if (dir < 0) u * 0.06f else 0f), h * 0.34f - u * 0.14f),
            Size(u * 0.06f, u * 0.28f),
            androidx.compose.ui.geometry.CornerRadius(u * 0.02f),
        )
    }
    // 落地灯。
    drawLine(sceneFill(Color(0xFF8A6A4F), p, muted), Offset(w * 0.90f, h * 0.96f), Offset(w * 0.90f, h * 0.60f), u * 0.012f, androidx.compose.ui.graphics.StrokeCap.Round)
    drawPath(
        androidx.compose.ui.graphics.Path().apply {
            moveTo(w * 0.90f - u * 0.09f, h * 0.60f)
            lineTo(w * 0.90f + u * 0.09f, h * 0.60f)
            lineTo(w * 0.90f + u * 0.06f, h * 0.54f)
            lineTo(w * 0.90f - u * 0.06f, h * 0.54f)
            close()
        },
        sceneFill(Color(0xFFF2C14E), p, muted),
    )
    drawOval(sceneFill(Color(0xFFF2C14E), p, muted).copy(alpha = 0.18f), Offset(w * 0.90f - u * 0.10f, h * 0.58f), Size(u * 0.20f, u * 0.08f))
    drawContactShadow(p, Offset(w * 0.90f, h * 0.97f), u * 0.12f, muted)
    // 吊兰。
    drawRoundRect(
        sceneFill(Color(0xFFB4644A), p, muted),
        Offset(w * 0.90f, h * 0.05f), Size(u * 0.08f, u * 0.07f),
        androidx.compose.ui.geometry.CornerRadius(u * 0.02f),
    )
    drawVine(p, Offset(w * 0.94f, h * 0.11f), h * 0.26f, phase, muted, flip = false)
    drawBlossom(p, Offset(w * 0.95f, h * 0.20f), u * 0.024f, phase + 0.3f, muted, BlossomPink)
    drawBlossom(p, Offset(w * 0.93f, h * 0.29f), u * 0.020f, phase + 0.6f, muted, BlossomWhite)
    // 地毯。
    drawOval(
        sceneFill(Color(0xFFB98BC9), p, (muted + 0.22f).coerceAtMost(0.8f)).copy(alpha = 0.35f),
        Offset(w * 0.28f, h * 0.86f), Size(w * 0.56f, h * 0.16f),
    )
    drawBookshelf(p, w * 0.04f, h * 0.78f, w * 0.30f, h * 0.50f, phase, muted)
    drawWindow(p, Offset(w * 0.82f, h * 0.34f), u * 0.14f, phase, muted, night = true)
    drawFlowerBox(p, Offset(w * 0.82f, h * 0.48f), u * 0.10f, phase + 0.2f, muted, BlossomPink)
    // 书桌墙上的小书架。
    drawRoundRect(sceneFill(Color(0xFFB58A5A), p, muted), Offset(w * 0.44f, h * 0.60f), Size(w * 0.20f, u * 0.02f), androidx.compose.ui.geometry.CornerRadius(u * 0.006f))
    for ((i, dx) in listOf(0f, 0.05f, 0.10f, 0.15f).withIndex()) {
        drawRect(
            sceneFill(listOf(Color(0xFFE8826A), Color(0xFF7FB7C9), Color(0xFFF2C14E), Color(0xFF9CCB7E))[i], p, muted),
            Offset(w * 0.44f + w * dx, h * 0.60f - u * (0.045f + 0.008f * (i % 2))),
            Size(w * 0.030f, u * (0.045f + 0.008f * (i % 2))),
        )
    }
    drawDesk(p, Offset(w * 0.54f, h * 0.80f), u * 0.24f, phase, muted)
    drawOpenBook(p, Offset(w * 0.50f, h * 0.76f), u * 0.07f, phase, muted)
    drawDeskLamp(p, Offset(w * 0.66f, h * 0.75f), u * 0.08f, phase, muted)
    drawRoundRect(
        sceneFill(Color(0xFF7FB7C9), p, muted),
        Offset(w * 0.59f - u * 0.018f, h * 0.72f), Size(u * 0.036f, u * 0.06f),
        androidx.compose.ui.geometry.CornerRadius(u * 0.012f),
    )
    drawFlowerStalk(p, Offset(w * 0.59f, h * 0.72f), Offset(w * 0.59f - u * 0.03f, h * 0.64f), u * 0.05f, phase, muted, BlossomPink, leaves = 1)
    drawFlowerStalk(p, Offset(w * 0.59f, h * 0.72f), Offset(w * 0.59f + u * 0.03f, h * 0.63f), u * 0.045f, phase + 0.3f, muted, BlossomWhite, leaves = 1)
    drawFlowerBox(p, Offset(w * 0.94f, h * 0.96f), u * 0.11f, phase + 0.4f, muted, BlossomYellow)
    val catS = u * 0.11f
    val deskTop = h * 0.80f - u * 0.0192f
    drawCatFigure(p, Offset(w * 0.74f, deskTop - catS * 0.72f), catS, phase + 0.3f, muted, detail = 0.5f, fillColor = CatGray)
    val personC = Offset(w * 0.40f, h * 0.84f)
    drawPersonFigure(p, personC, u * 0.18f, phase, muted, detail = 0.5f)
    drawContactShadow(p, Offset(personC.x, personC.y + u * 0.12f), u * 0.22f, muted)
    drawContactShadow(p, Offset(w * 0.04f, h * 0.78f), u * 0.32f, muted, alpha = 0.10f)
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
    drawRect(sceneFill(Color(0xFFF2E8D8), p, (muted * 0.35f).coerceAtLeast(0.18f)), size = size)
    // 台面以上的墙砖只铺到台面，别让整面墙变成方格纸。
    for (row in 0..2) for (col in 0..9) {
        val x = w * (col / 10f) + if (row % 2 == 0) 0f else w * 0.05f
        drawRect(
            sceneEdge(p, 0.12f),
            Offset(x, h * (0.50f + row * 0.09f)),
            Size(w * 0.10f, h * 0.075f),
            style = androidx.compose.ui.graphics.drawscope.Stroke(u * 0.004f),
        )
    }
    // 吊柜。
    for ((i, x) in listOf(0.03f, 0.35f).withIndex()) {
        val cw = w * 0.28f
        drawRoundRect(sceneFill(Color(0xFFC99A6A), p, muted), Offset(w * x, h * 0.07f), Size(cw, h * 0.22f), androidx.compose.ui.geometry.CornerRadius(u * 0.012f))
        drawRoundRect(sceneEdge(p, 0.30f), Offset(w * x, h * 0.07f), Size(cw, h * 0.22f), androidx.compose.ui.geometry.CornerRadius(u * 0.012f), style = androidx.compose.ui.graphics.drawscope.Stroke(u * 0.005f))
        drawLine(sceneEdge(p, 0.25f), Offset(w * x + cw / 2, h * 0.07f), Offset(w * x + cw / 2, h * 0.29f), u * 0.005f)
        drawCircle(sceneFill(Color(0xFFF2C14E), p, muted), u * 0.012f, Offset(w * x + cw * 0.42f, h * 0.20f))
        drawCircle(sceneFill(Color(0xFFF2C14E), p, muted), u * 0.012f, Offset(w * x + cw * 0.58f, h * 0.20f))
    }
    // 开放搁板 + 罐子 + 挂着的香草。
    drawRoundRect(sceneFill(Color(0xFFB58A5A), p, muted), Offset(w * 0.04f, h * 0.36f), Size(w * 0.24f, u * 0.018f), androidx.compose.ui.geometry.CornerRadius(u * 0.006f))
    drawJar(p, Offset(w * 0.09f, h * 0.36f), u * 0.045f, phase, muted)
    drawJar(p, Offset(w * 0.16f, h * 0.355f), u * 0.038f, phase + 0.2f, muted)
    drawJar(p, Offset(w * 0.23f, h * 0.36f), u * 0.042f, phase + 0.4f, muted)
    drawCircle(sceneFill(Color(0xFFFFF4E6), p, muted), u * 0.055f, Offset(w * 0.46f, h * 0.12f))
    drawCircle(sceneEdge(p, 0.30f), u * 0.055f, Offset(w * 0.46f, h * 0.12f), style = androidx.compose.ui.graphics.drawscope.Stroke(u * 0.004f))
    drawLine(sceneEdge(p, 0.4f), Offset(w * 0.46f, h * 0.12f), Offset(w * 0.46f, h * 0.12f - u * 0.03f), u * 0.004f)
    drawLine(sceneEdge(p, 0.4f), Offset(w * 0.46f, h * 0.12f), Offset(w * 0.46f + u * 0.02f, h * 0.12f), u * 0.004f)
    // 吊兰：从吊柜边上垂下来。
    drawRoundRect(sceneFill(Color(0xFFB4644A), p, muted), Offset(w * 0.68f, h * 0.08f), Size(u * 0.07f, u * 0.06f), androidx.compose.ui.geometry.CornerRadius(u * 0.015f))
    drawVine(p, Offset(w * 0.715f, h * 0.13f), h * 0.22f, phase, muted, flip = false)
    drawBlossom(p, Offset(w * 0.73f, h * 0.22f), u * 0.022f, phase + 0.3f, muted, BlossomWhite)
    drawBlossom(p, Offset(w * 0.70f, h * 0.30f), u * 0.019f, phase + 0.6f, muted, BlossomPink)
    drawWindow(p, Offset(w * 0.86f, h * 0.34f), u * 0.11f, phase, muted, night = false)
    drawFlowerBox(p, Offset(w * 0.86f, h * 0.46f), u * 0.09f, phase + 0.2f, muted, BlossomWhite)
    // 右侧搁板 + 挂勺。
    drawRoundRect(sceneFill(Color(0xFFB58A5A), p, muted), Offset(w * 0.64f, h * 0.46f), Size(w * 0.16f, u * 0.016f), androidx.compose.ui.geometry.CornerRadius(u * 0.005f))
    drawJar(p, Offset(w * 0.68f, h * 0.46f), u * 0.036f, phase, muted)
    drawJar(p, Offset(w * 0.74f, h * 0.455f), u * 0.030f, phase + 0.3f, muted)
    for (dx in listOf(0.66f, 0.70f, 0.74f)) {
        drawLine(sceneEdge(p, 0.35f), Offset(w * dx, h * 0.46f + u * 0.016f), Offset(w * dx, h * 0.52f), u * 0.004f)
        drawCircle(sceneFill(Color(0xFF9FBBC4), p, muted), u * 0.012f, Offset(w * dx, h * 0.525f))
    }
    // 台面碗/砧板。
    drawOval(sceneFill(Color(0xFFE8826A), p, muted), Offset(w * 0.34f - u * 0.04f, h * 0.74f), Size(u * 0.08f, u * 0.035f))
    drawRoundRect(sceneFill(Color(0xFFD8C9A8), p, muted), Offset(w * 0.55f, h * 0.74f), Size(u * 0.10f, u * 0.025f), androidx.compose.ui.geometry.CornerRadius(u * 0.006f))
    drawRect(sceneFill(KitchenCounter, p, (muted * 0.35f).coerceAtLeast(0.18f)), Offset(0f, h * 0.76f), Size(w, h * 0.24f))
    drawOval(
        sceneFill(Color(0xFFB98BC9), p, (muted + 0.22f).coerceAtMost(0.8f)).copy(alpha = 0.30f),
        Offset(w * 0.22f, h * 0.88f), Size(w * 0.56f, h * 0.14f),
    )
    drawStove(p, Offset(w * 0.46f, h * 0.84f), u * 0.24f, phase, muted)
    drawPot(p, Offset(w * 0.46f, h * 0.71f), u * 0.10f, phase, muted)
    drawSteam(p, Offset(w * 0.46f, h * 0.62f), u * 0.09f, phase, muted)
    drawFlowerBox(p, Offset(w * 0.14f, h * 0.79f), u * 0.11f, phase + 0.3f, muted, BlossomPink)
    drawFlowerBox(p, Offset(w * 0.70f, h * 0.79f), u * 0.10f, phase + 0.6f, muted, BlossomYellow)
    drawCarrot(p, Offset(w * 0.60f, h * 0.81f), u * 0.055f, phase, muted)
    drawTomatoPlant(p, Offset(w * 0.66f, h * 0.80f), u * 0.08f, phase + 0.4f, muted)
    val personC = Offset(w * 0.30f, h * 0.87f)
    drawContactShadow(p, Offset(personC.x, personC.y + u * 0.13f), u * 0.22f, muted)
    drawContactShadow(p, Offset(w * 0.86f, h * 0.89f + u * 0.10f), u * 0.20f, muted)
    drawPersonFigure(p, personC, u * 0.19f, phase + 0.2f, muted, detail = 0.5f)
    drawCatFigure(p, Offset(w * 0.86f, h * 0.89f), u * 0.17f, phase + 0.4f, muted, detail = 0.5f)
    drawButterfly(p, Offset(w * 0.55f, h * 0.40f), u * 0.04f, phase + 0.2f, muted)
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
            0f to sceneFill(WallCream, p, (muted * 0.4f).coerceAtLeast(0.18f)),
            0.76f to p.base,
            1f to sceneFill(FloorWood, p, (muted * 0.26f).coerceAtLeast(0.15f)),
        ),
        size = size,
    )
    // 墙面猫跳台：两层木搁板，最上面一只猫，画面有高低层次。
    for ((i, y) in listOf(0.34f, 0.52f).withIndex()) {
        drawRoundRect(
            sceneFill(Color(0xFFB58A5A), p, muted),
            Offset(w * (0.44f + i * 0.10f), h * y), Size(w * 0.22f, u * 0.018f),
            androidx.compose.ui.geometry.CornerRadius(u * 0.006f),
        )
        drawLine(sceneFill(Color(0xFF8A6A4F), p, muted), Offset(w * (0.48f + i * 0.10f), h * y + u * 0.018f), Offset(w * (0.47f + i * 0.10f), h * y + u * 0.09f), u * 0.010f)
        drawLine(sceneFill(Color(0xFF8A6A4F), p, muted), Offset(w * (0.62f + i * 0.10f), h * y + u * 0.018f), Offset(w * (0.63f + i * 0.10f), h * y + u * 0.09f), u * 0.010f)
    }
    drawCatFigure(p, Offset(w * 0.55f, h * 0.34f - u * 0.72f * 0.10f), u * 0.10f, phase + 0.2f, muted, detail = 0.5f, fillColor = CatWhite)
    drawCatFigure(p, Offset(w * 0.64f, h * 0.52f - u * 0.72f * 0.09f), u * 0.09f, phase + 0.4f, muted, detail = 0.5f, fillColor = CatCream)
    // 搁板上的小盆栽。
    drawRoundRect(sceneFill(Color(0xFFB4644A), p, muted), Offset(w * 0.47f, h * 0.52f - u * 0.045f), Size(u * 0.055f, u * 0.045f), androidx.compose.ui.geometry.CornerRadius(u * 0.010f))
    drawGrassTuft(p, Offset(w * 0.47f + u * 0.0275f, h * 0.52f - u * 0.045f), u * 0.06f, phase, muted, blades = 4)
    // 挂画与吊兰。
    drawRoundRect(sceneFill(Color(0xFFB58A5A), p, muted), Offset(w * 0.72f, h * 0.20f), Size(u * 0.11f, u * 0.14f), androidx.compose.ui.geometry.CornerRadius(u * 0.008f))
    drawRect(sceneFill(Color(0xFFFFF4E6), p, muted), Offset(w * 0.72f + u * 0.010f, h * 0.20f + u * 0.010f), Size(u * 0.09f, u * 0.12f))
    drawBlossom(p, Offset(w * 0.72f + u * 0.055f, h * 0.20f + u * 0.07f), u * 0.026f, phase, muted, BlossomPink)
    drawRoundRect(sceneFill(Color(0xFFB58A5A), p, muted), Offset(w * 0.30f, h * 0.14f), Size(u * 0.10f, u * 0.13f), androidx.compose.ui.geometry.CornerRadius(u * 0.008f))
    drawRect(sceneFill(Color(0xFFFFF4E6), p, muted), Offset(w * 0.30f + u * 0.010f, h * 0.14f + u * 0.010f), Size(u * 0.08f, u * 0.11f))
    drawBlossom(p, Offset(w * 0.30f + u * 0.050f, h * 0.14f + u * 0.065f), u * 0.024f, phase + 0.3f, muted, BlossomYellow)
    drawRoundRect(sceneFill(Color(0xFFB4644A), p, muted), Offset(w * 0.90f, h * 0.06f), Size(u * 0.07f, u * 0.06f), androidx.compose.ui.geometry.CornerRadius(u * 0.015f))
    drawVine(p, Offset(w * 0.935f, h * 0.11f), h * 0.22f, phase + 0.3f, muted, flip = true)
    drawOval(sceneFill(RugPurple, p, 0.40f), Offset(w * 0.06f, h * 0.80f), Size(w * 0.88f, h * 0.26f))
    drawWindow(p, Offset(w * 0.84f, h * 0.30f), u * 0.11f, phase, muted, night = false)
    drawFlowerBox(p, Offset(w * 0.84f, h * 0.40f), u * 0.09f, phase + 0.2f, muted, BlossomPink)
    // 猫爬架：立柱 + 三层平台。
    drawRoundRect(
        sceneFill(Color(0xFFB58A5A), p, muted),
        Offset(w * 0.03f, h * 0.44f), Size(u * 0.04f, h * 0.48f),
        androidx.compose.ui.geometry.CornerRadius(u * 0.015f),
    )
    for ((i, y) in listOf(0.46f, 0.62f, 0.78f).withIndex()) {
        drawOval(
            sceneFill(if (i % 2 == 0) Color(0xFFB98BC9) else CatCream, p, muted + 0.10f),
            Offset(w * 0.03f - u * 0.11f, h * y), Size(u * 0.26f, u * 0.05f),
        )
        drawCatFigure(p, Offset(w * 0.03f, h * y - u * 0.02f), u * (0.07f + i * 0.01f), phase + i * 0.2f, muted, detail = 0.4f, fillColor = listOf(CatGray, CatCream, CatBlack)[i])
    }
    drawCushion(p, Offset(w * 0.22f, h * 0.90f), u * 0.16f, phase, muted)
    drawBox(p, Offset(w * 0.66f, h * 0.93f), u * 0.12f, phase, muted)
    drawFlowerBox(p, Offset(w * 0.96f, h * 0.97f), u * 0.12f, phase + 0.4f, muted, BlossomPink)
    drawFloweringBranch(
        p, Offset(w * 0.02f, h * 0.42f), Offset(w * 0.20f, h * 0.54f),
        u * 0.26f, phase, muted, BlossomPink, seed = 0.3f,
    )
    // 地上的猫。
    for ((i, spec) in listOf(
        Triple(0.14f, 0.19f, CatGray), Triple(0.42f, 0.21f, CatCream),
        Triple(0.86f, 0.16f, CatWhite), Triple(0.58f, 0.14f, CatBlack),
    ).withIndex()) {
        drawContactShadow(p, Offset(w * spec.first, h * 0.93f + u * 0.10f), u * 0.22f, muted)
        drawCatFigure(p, Offset(w * spec.first, h * 0.94f), u * spec.second, phase + i * 0.2f, muted, detail = 0.5f, fillColor = spec.third)
    }
    drawYarnBall(p, Offset(w * 0.30f, h * 0.98f), u * 0.06f, phase, muted)
    drawFlowerBed(
        p, Offset(w * 0.52f, h * 0.99f), u * 0.18f, u * 0.10f,
        phase + 0.4f, muted, listOf(BlossomPink, BlossomYellow, BlossomWhite), count = 4,
    )
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
