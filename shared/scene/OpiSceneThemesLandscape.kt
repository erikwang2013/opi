// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 山川、大海、池塘、迷雾四套主题。山川=山脊/雪顶/松树/徒步客；大海=海浪/帆船/
//! 灯塔/贝壳；池塘=荷叶/荷花/芦苇/鸭子/蜻蜓；迷雾=雾带/枯树/路灯/雨伞/灯笼。

package xyz.erik.opi.scene

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import kotlin.math.min

private val MountainSky = Color(0xFFB7D3E8)
private val MountainGround = Color(0xFF7A9A78)
private val SeaSky = Color(0xFFBFE6EF)
private val SeaDeep = Color(0xFF3C7FA6)
private val SeaSand = Color(0xFFE8D3A8)
private val PondSky = Color(0xFFCFE8D5)
private val PondWater = Color(0xFF6FAFC9)
private val PondBank = Color(0xFF8FBF7A)
private val MistSky = Color(0xFFC4D2D8)
private val MistGround = Color(0xFF83948F)

// ---------- 山川 ----------

internal fun DrawScope.drawMountainBackdrop(p: ScenePalette, phase: Float, muted: Float) {
    val w = size.width
    val h = size.height
    if (w < 2f || h < 2f) return
    val u = min(w, h)
    drawRect(
        brush = Brush.verticalGradient(
            0f to sceneFill(MountainSky, p, (muted * 0.45f).coerceAtLeast(0.22f)),
            0.62f to p.base,
            1f to sceneFill(MountainGround, p, (muted * 0.30f).coerceAtLeast(0.18f)),
        ),
        size = size,
    )
    drawSun(p, Offset(w * 0.18f, h * 0.18f), u * 0.10f, phase, muted)
    drawCloud(p, Offset(w * 0.72f, h * 0.16f), u * 0.17f, phase, muted)
    drawCloud(p, Offset(w * 0.46f, h * 0.10f), u * 0.11f, phase + 0.4f, muted)
    // 远山与山脚树线。
    drawMountains(p, baseY = h * 0.80f, width = w, height = h * 0.52f, muted = muted)
    for ((i, x) in listOf(0.03f, 0.13f, 0.23f, 0.33f, 0.67f, 0.77f, 0.87f, 0.97f).withIndex()) {
        drawPine(p, Offset(w * x, h * 0.78f), u * (0.16f + 0.03f * (i % 3)), phase + i * 0.15f, muted + 0.16f)
    }
    drawPine(p, Offset(w * 0.07f, h * 0.96f), u * 0.46f, phase, muted)
    drawPine(p, Offset(w * 0.93f, h * 0.94f), u * 0.42f, phase + 0.4f, muted)
    drawPine(p, Offset(w * 0.78f, h * 0.88f), u * 0.30f, phase + 0.2f, muted)
    // 山花与花枝：高山草甸也要有前景。
    drawFloweringBranch(
        p, Offset(-w * 0.02f, h * 0.08f), Offset(w * 0.22f, h * 0.22f),
        u * 0.36f, phase, muted, BlossomWhite, seed = 0.2f,
    )
    drawFloweringBranch(
        p, Offset(w * 1.02f, h * 0.06f), Offset(w * 0.78f, h * 0.20f),
        u * 0.34f, phase + 0.4f, muted, BlossomPink, seed = 0.7f,
    )
    for ((i, x) in listOf(0.14f, 0.34f, 0.58f, 0.82f).withIndex()) {
        drawFlowerBed(
            p, Offset(w * x, h * 0.99f), u * 0.18f, u * 0.11f,
            phase + i * 0.2f, muted, listOf(BlossomYellow, BlossomWhite, BlossomPink), count = 4,
        )
    }
    val personC = Offset(w * 0.50f, h * 0.86f)
    drawContactShadow(p, Offset(personC.x, personC.y + u * 0.12f), u * 0.22f, muted)
    drawPersonFigure(p, personC, u * 0.18f, phase + 0.2f, muted, detail = 0.5f)
    drawBird(p, Offset(w * 0.36f, h * 0.28f), u * 0.026f, phase, muted)
    drawBird(p, Offset(w * 0.44f, h * 0.23f), u * 0.020f, phase + 0.3f, muted)
}

internal fun DrawScope.drawMountainHalo(
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
    drawMountains(p, baseY = barTop + u * 0.35f, width = u * 2.0f, height = u * 0.80f, muted = muted, left = topXs[0] - u)
    drawSun(p, Offset(topXs[1], barTop - u * 0.55f), u * 0.28f, phase, muted)
    drawPersonFigure(p, Offset(topXs[2], barTop - u * 0.20f), u * 0.62f, phase + 0.2f, muted, detail = 1f)
    drawPine(p, Offset(topXs[3], barTop + u * 0.25f), u * 0.90f, phase + 0.2f, muted)
    drawPine(p, Offset(bottomXs[0], barBottom + u * 0.55f), u * 0.55f, phase, muted)
    drawFlowerCluster(p, Offset(bottomXs[1], barBottom + u * 0.42f), u * 0.34f, phase + 0.3f, muted, hanging = true)
    drawOval(sceneFill(MountainGround, p, 0.55f), Offset(bottomXs[2] - u * 0.45f, barBottom + u * 0.45f), Size(u * 0.9f, u * 0.35f))
    drawBird(p, Offset(topXs[0] + u * 0.9f, barTop - u * 0.55f), u * 0.30f, phase, muted)
}

// ---------- 大海 ----------

internal fun DrawScope.drawSeaBackdrop(p: ScenePalette, phase: Float, muted: Float) {
    val w = size.width
    val h = size.height
    if (w < 2f || h < 2f) return
    val u = min(w, h)
    drawRect(
        brush = Brush.verticalGradient(
            0f to sceneFill(SeaSky, p, (muted * 0.45f).coerceAtLeast(0.22f)),
            0.55f to p.base,
            1f to sceneFill(SeaSky, p, (muted * 0.30f).coerceAtLeast(0.18f)),
        ),
        size = size,
    )
    drawSun(p, Offset(w * 0.82f, h * 0.16f), u * 0.10f, phase, muted)
    drawCloud(p, Offset(w * 0.20f, h * 0.14f), u * 0.18f, phase, muted)
    drawCloud(p, Offset(w * 0.66f, h * 0.10f), u * 0.13f, phase + 0.4f, muted)
    drawRect(sceneFill(SeaDeep, p, (muted * 0.30f).coerceAtLeast(0.18f)), Offset(0f, h * 0.60f), Size(w, h * 0.32f))
    drawWaves(p, -w * 0.1f, w * 1.1f, h * 0.62f, u * 0.12f, phase, muted)
    drawWaves(p, -w * 0.1f, w * 1.1f, h * 0.74f, u * 0.10f, phase + 0.4f, muted)
    drawRect(sceneFill(SeaSand, p, (muted * 0.35f).coerceAtLeast(0.20f)), Offset(0f, h * 0.90f), Size(w, h * 0.10f))
    drawSailboat(p, Offset(w * 0.52f, h * 0.68f), u * 0.20f, phase, muted)
    drawLighthouse(p, Offset(w * 0.12f, h * 0.84f), u * 0.18f, phase, muted)
    // 海岸花枝/花藤。
    drawFloweringBranch(
        p, Offset(-w * 0.02f, h * 0.08f), Offset(w * 0.24f, h * 0.22f),
        u * 0.36f, phase, muted, BlossomPink, seed = 0.2f,
    )
    drawClimbingVine(
        p, Offset(w * 0.04f, h * 1.02f), Offset(w * 0.06f, h * 0.60f),
        u * 0.34f, phase, muted, BlossomWhite, flowers = 2, seed = 0.6f,
    )
    // 沙滩花草与贝壳。
    for ((i, x) in listOf(0.20f, 0.38f, 0.62f, 0.88f).withIndex()) {
        drawFlowerBed(
            p, Offset(w * x, h * 0.995f), u * 0.16f, u * 0.09f,
            phase + i * 0.2f, muted, listOf(BlossomWhite, BlossomPink, BlossomYellow), count = 3,
        )
    }
    drawShell(p, Offset(w * 0.28f, h * 0.98f), u * 0.04f, phase, muted)
    drawShell(p, Offset(w * 0.34f, h * 0.99f), u * 0.03f, phase + 0.3f, muted)
    val personC = Offset(w * 0.72f, h * 0.88f)
    drawContactShadow(p, Offset(personC.x, personC.y + u * 0.12f), u * 0.22f, muted)
    drawContactShadow(p, Offset(w * 0.87f, h * 0.94f + u * 0.10f), u * 0.20f, muted)
    drawPersonFigure(p, personC, u * 0.18f, phase + 0.2f, muted, detail = 0.5f)
    drawCatFigure(p, Offset(w * 0.87f, h * 0.94f), u * 0.16f, phase + 0.4f, muted, detail = 0.5f)
    drawBird(p, Offset(w * 0.34f, h * 0.24f), u * 0.026f, phase, muted)
    drawBird(p, Offset(w * 0.43f, h * 0.19f), u * 0.020f, phase + 0.3f, muted)
}

internal fun DrawScope.drawSeaHalo(
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
    drawSailboat(p, Offset(topXs[0], barTop - u * 0.10f), u * 0.45f, phase, muted)
    drawLighthouse(p, Offset(topXs[1], barTop + u * 0.30f), u * 0.55f, phase, muted)
    drawBird(p, Offset(topXs[2], barTop - u * 0.55f), u * 0.32f, phase, muted)
    drawSun(p, Offset(topXs[3], barTop - u * 0.55f), u * 0.30f, phase, muted)
    // 下沿：一条浪 + 贝壳/沙滩猫。
    drawWaves(p, bottomXs[0] - u, bottomXs[2] + u, barBottom + u * 0.35f, u * 0.08f, phase, muted)
    drawShell(p, Offset(bottomXs[0], barBottom + u * 0.50f), u * 0.40f, phase + 0.2f, muted)
    drawCatFigure(p, Offset(bottomXs[1], barBottom + u * 0.48f), u * 0.46f, phase + 0.4f, muted, detail = 1f)
    drawShell(p, Offset(bottomXs[2], barBottom + u * 0.52f), u * 0.32f, phase + 0.5f, muted)
}

// ---------- 池塘 ----------

internal fun DrawScope.drawPondBackdrop(p: ScenePalette, phase: Float, muted: Float) {
    val w = size.width
    val h = size.height
    if (w < 2f || h < 2f) return
    val u = min(w, h)
    drawRect(
        brush = Brush.verticalGradient(
            0f to sceneFill(PondSky, p, (muted * 0.45f).coerceAtLeast(0.22f)),
            0.52f to p.base,
            1f to sceneFill(PondBank, p, (muted * 0.30f).coerceAtLeast(0.18f)),
        ),
        size = size,
    )
    drawSun(p, Offset(w * 0.16f, h * 0.16f), u * 0.09f, phase, muted)
    drawCloud(p, Offset(w * 0.72f, h * 0.14f), u * 0.16f, phase, muted)
    drawCloud(p, Offset(w * 0.40f, h * 0.09f), u * 0.11f, phase + 0.4f, muted)
    // 水塘：水面之上是天空，之下是前景草岸。
    drawOval(sceneFill(PondWater, p, (muted * 0.28f).coerceAtLeast(0.16f)), Offset(-w * 0.1f, h * 0.58f), Size(w * 1.2f, h * 0.38f))
    drawWaves(p, -w * 0.1f, w * 1.1f, h * 0.66f, u * 0.07f, phase, muted)
    drawRect(sceneFill(PondBank, p, (muted * 0.30f).coerceAtLeast(0.18f)), Offset(0f, h * 0.92f), Size(w, h * 0.08f))
    // 岸边柳树/花枝。
    drawTree(p, Offset(w * 0.94f, h * 0.96f), u * 0.34f, phase, muted, BotanyLeaf, BotanyLeafDark, BlossomWhite)
    drawFloweringBranch(
        p, Offset(w * 0.02f, h * 0.04f), Offset(w * 0.26f, h * 0.22f),
        u * 0.40f, phase, muted, BlossomPink, seed = 0.2f,
    )
    // 岸边芦苇与花圃，根都在草岸上。
    drawReed(p, Offset(w * 0.06f, h * 0.94f), u * 0.24f, phase, muted)
    drawReed(p, Offset(w * 0.88f, h * 0.95f), u * 0.20f, phase + 0.4f, muted)
    for ((i, x) in listOf(0.14f, 0.34f, 0.58f, 0.82f).withIndex()) {
        drawFlowerBed(
            p, Offset(w * x, h * 0.995f), u * 0.17f, u * 0.10f,
            phase + i * 0.2f, muted, listOf(BlossomPink, BlossomWhite, BlossomYellow), count = 4,
        )
    }
    // 荷花：花茎从水面长出来，荷叶贴水。
    drawLotus(p, Offset(w * 0.24f, h * 0.78f), u * 0.14f, phase, muted)
    drawLotus(p, Offset(w * 0.46f, h * 0.86f), u * 0.12f, phase + 0.3f, muted)
    drawLotus(p, Offset(w * 0.70f, h * 0.80f), u * 0.13f, phase + 0.6f, muted)
    drawDuck(p, Offset(w * 0.60f, h * 0.84f), u * 0.10f, phase + 0.2f, muted)
    drawDragonfly(p, Offset(w * 0.38f, h * 0.50f), u * 0.08f, phase, muted)
    // 岸边猫与花。
    drawContactShadow(p, Offset(w * 0.10f, h * 0.94f + u * 0.09f), u * 0.18f, muted)
    drawCatFigure(p, Offset(w * 0.10f, h * 0.94f), u * 0.17f, phase + 0.4f, muted, detail = 0.5f)
    drawButterfly(p, Offset(w * 0.52f, h * 0.46f), u * 0.045f, phase + 0.2f, muted)
}

internal fun DrawScope.drawPondHalo(
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
    drawLotus(p, Offset(topXs[0], barTop - u * 0.22f), u * 0.44f, phase, muted)
    drawDragonfly(p, Offset(topXs[1], barTop - u * 0.50f), u * 0.32f, phase, muted)
    drawCatFigure(p, Offset(topXs[2], barTop - u * 0.28f), u * 0.68f, phase + 0.4f, muted, detail = 1f)
    drawReed(p, Offset(topXs[3], barTop + u * 0.30f), u * 0.65f, phase + 0.4f, muted)
    // 下沿：水面/鸭/荷。
    drawOval(sceneFill(PondWater, p, 0.30f), Offset(bottomXs[0] - u, barBottom + u * 0.35f), Size(u * 4.2f, u * 0.55f))
    drawDuck(p, Offset(bottomXs[0], barBottom + u * 0.48f), u * 0.40f, phase + 0.2f, muted)
    drawLotus(p, Offset(bottomXs[1], barBottom + u * 0.46f), u * 0.34f, phase + 0.6f, muted)
    drawFirefly(p, Offset(bottomXs[2], barBottom + u * 0.38f), u * 0.35f, phase, muted)
}

// ---------- 迷雾 ----------

internal fun DrawScope.drawMistBackdrop(p: ScenePalette, phase: Float, muted: Float) {
    val w = size.width
    val h = size.height
    if (w < 2f || h < 2f) return
    val u = min(w, h)
    drawRect(
        brush = Brush.verticalGradient(
            0f to sceneFill(MistSky, p, (muted * 0.30f).coerceAtLeast(0.16f)),
            0.62f to p.base,
            1f to sceneFill(MistGround, p, (muted * 0.24f).coerceAtLeast(0.14f)),
        ),
        size = size,
    )
    drawOval(sceneFill(MistGround, p, 0.44f), Offset(-w * 0.15f, h * 0.68f), Size(w * 0.8f, h * 0.42f))
    drawOval(sceneFill(MistGround, p, 0.36f), Offset(w * 0.45f, h * 0.72f), Size(w * 0.8f, h * 0.40f))
    // 远处农舍 + 烟囱，雾里有个落脚点。
    drawFarmhouse(p, Offset(w * 0.84f, h * 0.82f), u * 0.16f, phase, muted)
    drawTree(p, Offset(w * 0.92f, h * 0.92f), u * 0.30f, phase + 0.3f, muted, BotanyLeafDark, Color(0xFF3F5A4E))
    drawBareTree(p, Offset(w * 0.12f, h * 0.90f), u * 0.38f, phase, muted)
    drawBareTree(p, Offset(w * 0.70f, h * 0.94f), u * 0.30f, phase + 0.4f, muted)
    drawShrub(p, Offset(w * 0.26f, h * 0.95f), u * 0.16f, phase, muted)
    drawShrub(p, Offset(w * 0.52f, h * 0.97f), u * 0.13f, phase + 0.3f, muted)
    drawFence(p, 0f, h * 0.72f, w, h * 0.12f, muted)
    drawStreetLamp(p, Offset(w * 0.34f, h * 0.90f), u * 0.22f, phase, muted)
    // 雾里的花藤与梅花，避免整幅灰成一片。
    drawFloweringBranch(
        p, Offset(w * 0.02f, h * 0.36f), Offset(w * 0.22f, h * 0.48f),
        u * 0.30f, phase, muted, BlossomWhite, seed = 0.3f,
    )
    drawClimbingVine(
        p, Offset(w * 0.06f, h * 0.84f), Offset(w * 0.08f, h * 0.58f),
        u * 0.32f, phase, muted, BlossomWhite, flowers = 2, seed = 0.3f,
    )
    drawClimbingVine(
        p, Offset(w * 0.82f, h * 0.84f), Offset(w * 0.80f, h * 0.60f),
        u * 0.30f, phase + 0.4f, muted, BlossomPink, flowers = 2, seed = 0.7f,
    )
    for ((i, x) in listOf(0.18f, 0.42f, 0.62f, 0.94f).withIndex()) {
        drawFlowerBed(
            p, Offset(w * x, h * 0.99f), u * 0.16f, u * 0.10f,
            phase + i * 0.2f, muted, listOf(BlossomWhite, BlossomYellow, BlossomPink), count = 3,
        )
    }
    val personC = Offset(w * 0.56f, h * 0.86f)
    val personS = u * 0.19f
    drawContactShadow(p, Offset(personC.x, personC.y + u * 0.12f), u * 0.22f, muted)
    drawPersonFigure(p, personC, personS, phase, muted, detail = 0.5f)
    drawUmbrella(p, Offset(w * 0.56f, h * 0.68f), u * 0.15f, phase, muted)
    val hand = Offset(personC.x + personS * 0.23f, personC.y - personS * 0.28f)
    val lanternC = Offset(hand.x + u * 0.02f, hand.y - u * 0.02f)
    drawLine(sceneEdge(p, 0.40f), hand, Offset(lanternC.x, lanternC.y - u * 0.06f), u * 0.005f, androidx.compose.ui.graphics.StrokeCap.Round)
    drawLantern(p, lanternC, u * 0.07f, phase, muted)
    drawFog(p, -w * 0.1f, w * 1.1f, h * 0.54f, h * 0.10f, phase, muted)
    drawFog(p, -w * 0.1f, w * 1.1f, h * 0.78f, h * 0.08f, phase + 0.4f, muted)
    drawFirefly(p, Offset(w * 0.44f, h * 0.62f), u * 0.03f, phase, muted)
    drawFirefly(p, Offset(w * 0.76f, h * 0.58f), u * 0.025f, phase + 0.3f, muted)
}

internal fun DrawScope.drawMistHalo(
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
    drawBareTree(p, Offset(topXs[0], barTop + u * 0.30f), u * 0.82f, phase, muted)
    drawStreetLamp(p, Offset(topXs[1], barTop + u * 0.30f), u * 0.55f, phase, muted)
    drawPersonFigure(p, Offset(topXs[2], barTop - u * 0.20f), u * 0.62f, phase, muted, detail = 1f)
    drawUmbrella(p, Offset(topXs[2], barTop - u * 0.62f), u * 0.35f, phase, muted)
    drawLantern(p, Offset(topXs[3], barTop - u * 0.45f), u * 0.46f, phase + 0.2f, muted)
    drawFence(p, bottomXs[0] - u, barBottom + u * 0.30f, (bottomXs[2] - bottomXs[0]) + u * 2f, u * 0.50f, muted)
    drawFirefly(p, Offset(bottomXs[1], barBottom + u * 0.40f), u * 0.34f, phase, muted)
    drawFog(p, bottomXs[2] - u, bottomXs[2] + u, barBottom + u * 0.20f, u * 0.45f, phase + 0.3f, muted)
    drawBareTree(p, Offset(bottomXs[2], barBottom + u * 0.60f), u * 0.45f, phase + 0.4f, muted)
}
