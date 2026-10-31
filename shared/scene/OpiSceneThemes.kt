// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 第一批四套背景里的另外三套（田园的原画在 `OpiScene.kt` 的`drawMeadow*`）：
//!
//! * 森林：松树、蘑菇、萤火虫、藤蔓，猫兔在林间，小人当徒步客；
//! * 花园：向日葵、花丛、蜜蜂、浇水壶，小人当园丁；
//! * 星夜：月亮、星星、灯笼、栅栏，猫坐栅栏、小人提灯。
//!
//! 每套都只**换道具、构图与色相种子**，角色/花草仍是同一批绘制函数 —— 所以叫
//! 「不同风格的类似皮肤」，不是互不相干的插画。

package xyz.erik.opi.scene

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.min
import kotlin.math.sin

private val ForestSky = Color(0xFFB9D8B4)
private val ForestGround = Color(0xFF7FA985)
private val ForestHill = Color(0xFF5B8768)
private val GardenSky = Color(0xFFF7DCE6)
private val GardenHill = Color(0xFFA9CF9E)
private val GardenHillDark = Color(0xFF86B77F)
private val NightSky = Color(0xFF2B3560)
private val NightHill = Color(0xFF1F2847)
private val NightHillFar = Color(0xFF273257)

// ---------- 森林 ----------

internal fun DrawScope.drawForestBackdrop(p: ScenePalette, phase: Float, muted: Float) {
    val w = size.width
    val h = size.height
    if (w < 2f || h < 2f) return
    val u = min(w, h)
    drawRect(
        brush = androidx.compose.ui.graphics.Brush.verticalGradient(
            0f to sceneFill(ForestSky, p, (muted * 0.55f).coerceAtLeast(0.28f)),
            0.6f to sceneFill(ForestGround, p, (muted * 0.4f).coerceAtLeast(0.22f)),
            1f to p.base,
        ),
        size = size,
    )
    // 林间漏下的光和云。
    drawSun(p, Offset(w * 0.18f, h * 0.16f), u * 0.06f, phase, muted)
    drawCloud(p, Offset(w * 0.55f, h * 0.12f), u * 0.14f, phase + 0.3f, muted)
    // 远景树线：一排压暗的小松树，森林才有纵深。
    for ((i, x) in listOf(0.04f, 0.14f, 0.26f, 0.38f, 0.50f, 0.62f, 0.74f, 0.86f, 0.96f).withIndex()) {
        drawPine(p, Offset(w * x, h * 0.72f), u * (0.18f + 0.03f * (i % 3)), phase + i * 0.17f, muted + 0.18f)
    }
    drawOval(sceneFill(ForestHill, p, 0.55f), Offset(-w * 0.1f, h * 0.70f), Size(w * 0.75f, h * 0.5f))
    drawOval(sceneFill(ForestHill, p, 0.45f), Offset(w * 0.45f, h * 0.74f), Size(w * 0.75f, h * 0.5f))
    // 林间小路：从画面底部弯向远处。
    drawPath(
        Path().apply {
            moveTo(w * 0.38f, h * 1.02f)
            quadraticTo(w * 0.44f, h * 0.84f, w * 0.56f, h * 0.72f)
            lineTo(w * 0.62f, h * 0.72f)
            quadraticTo(w * 0.52f, h * 0.86f, w * 0.50f, h * 1.02f)
            close()
        },
        sceneFill(Color(0xFFD8C9A8), p, (muted + 0.25f).coerceAtMost(0.75f)),
    )
    // 两侧近景松树。
    drawPine(p, Offset(w * 0.07f, h * 0.90f), u * 0.60f, phase, muted)
    drawPine(p, Offset(w * 0.93f, h * 0.88f), u * 0.54f, phase + 0.3f, muted)
    drawPine(p, Offset(w * 0.80f, h * 0.74f), u * 0.38f, phase + 0.6f, muted)
    drawPine(p, Offset(w * 0.20f, h * 0.72f), u * 0.34f, phase + 0.1f, muted)
    // 花枝与花藤：森林里也补上，且根/枝都落在实景上。
    drawFloweringBranch(
        p, Offset(-w * 0.02f, h * 0.06f), Offset(w * 0.26f, h * 0.20f),
        u * 0.40f, phase, muted, BlossomWhite, seed = 0.1f,
    )
    drawFloweringBranch(
        p, Offset(w * 1.02f, h * 0.05f), Offset(w * 0.74f, h * 0.18f),
        u * 0.38f, phase + 0.4f, muted, BlossomPink, seed = 0.6f,
    )
    drawClimbingVine(
        p, Offset(w * 0.03f, h * 1.02f), Offset(w * 0.05f, h * 0.22f),
        u * 0.48f, phase, muted, BlossomWhite, flowers = 3, seed = 0.2f,
    )
    drawClimbingVine(
        p, Offset(w * 0.97f, h * 1.02f), Offset(w * 0.95f, h * 0.24f),
        u * 0.44f, phase + 0.5f, muted, BlossomPink, flowers = 3, seed = 0.8f,
    )
    // 蘑菇丛 + 蕨叶 + 花圃；每朵都从地面长出来。
    drawContactShadow(p, Offset(w * 0.28f, h * 0.98f), u * 0.16f, muted)
    drawContactShadow(p, Offset(w * 0.72f, h * 0.97f), u * 0.14f, muted)
    drawMushroom(p, Offset(w * 0.28f, h * 0.96f), u * 0.12f, phase + 0.2f, muted)
    drawMushroom(p, Offset(w * 0.35f, h * 0.99f), u * 0.08f, phase + 0.5f, muted)
    drawMushroom(p, Offset(w * 0.72f, h * 0.97f), u * 0.10f, phase + 0.8f, muted)
    drawGrassTuft(p, Offset(w * 0.64f, h * 0.98f), u * 0.12f, phase + 0.2f, muted)
    drawFlowerBed(
        p, Offset(w * 0.47f, h * 0.99f), u * 0.20f, u * 0.12f,
        phase + 0.3f, muted, listOf(BlossomWhite, BlossomPink, BlossomYellow), count = 4,
    )
    // 角色：猫、徒步的小人、兔子，全部踩在地面上。
    val personC = Offset(w * 0.52f, h * 0.80f)
    drawContactShadow(p, Offset(w * 0.13f, h * 0.80f + u * 0.20f), u * 0.30f, muted)
    drawContactShadow(p, Offset(w * 0.87f, h * 0.82f + u * 0.16f), u * 0.26f, muted)
    drawContactShadow(p, Offset(personC.x, personC.y + u * 0.15f), u * 0.26f, muted)
    drawCatFigure(p, Offset(w * 0.13f, h * 0.80f), u * 0.28f, phase, muted, detail = 0.55f)
    drawPersonFigure(p, personC, u * 0.22f, phase + 0.2f, muted, detail = 0.55f)
    drawRabbitFigure(p, Offset(w * 0.87f, h * 0.82f), u * 0.22f, phase + 0.4f, muted, detail = 0.55f)
    // 萤火虫。
    fireflies(p, w, h, u, phase, muted, listOf(0.24f to 0.55f, 0.44f to 0.42f, 0.62f to 0.58f, 0.78f to 0.40f))
}

internal fun DrawScope.drawForestHalo(
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
    drawCatFigure(p, Offset(topXs[0], barTop - u * 0.30f), u * 0.72f, phase, muted, detail = 1f)
    drawPine(p, Offset(topXs[1], barTop + u * 0.18f), u * 0.95f, phase + 0.3f, muted)
    drawRabbitFigure(p, Offset(topXs[2], barTop - u * 0.24f), u * 0.60f, phase + 0.4f, muted, detail = 1f)
    drawMushroom(p, Offset(topXs[3], barTop - u * 0.05f), u * 0.58f, phase + 0.6f, muted)
    drawMushroom(p, Offset(bottomXs[0], barBottom + u * 0.45f), u * 0.52f, phase + 0.1f, muted)
    drawVine(p, Offset(bottomXs[1], barBottom - u * 0.05f), u * 0.78f, phase + 0.3f, muted, flip = false)
    drawMushroom(p, Offset(bottomXs[2], barBottom + u * 0.50f), u * 0.45f, phase + 0.7f, muted)
    fireflies(p, size.width, size.height, u, phase, muted, listOf(0.20f to 0.30f, 0.46f to 0.45f, 0.68f to 0.30f, 0.36f to 0.75f))
}

// ---------- 花园 ----------

internal fun DrawScope.drawGardenBackdrop(p: ScenePalette, phase: Float, muted: Float) {
    val w = size.width
    val h = size.height
    if (w < 2f || h < 2f) return
    val u = min(w, h)
    drawRect(
        brush = androidx.compose.ui.graphics.Brush.verticalGradient(
            0f to sceneFill(GardenSky, p, (muted * 0.5f).coerceAtLeast(0.28f)),
            0.60f to p.base,
            1f to sceneFill(GardenHill, p, (muted * 0.32f).coerceAtLeast(0.20f)),
        ),
        size = size,
    )
    drawSun(p, Offset(w * 0.86f, h * 0.13f), u * 0.075f, phase, muted)
    drawCloud(p, Offset(w * 0.17f, h * 0.15f), u * 0.20f, phase, muted)
    drawCloud(p, Offset(w * 0.62f, h * 0.09f), u * 0.13f, phase + 0.4f, muted)
    // 远处树篱：让花园有背景，不是一块空草地。
    for ((i, x) in listOf(0.08f, 0.24f, 0.47f, 0.66f, 0.86f).withIndex()) {
        drawShrub(p, Offset(w * x, h * 0.70f), u * (0.14f + 0.02f * (i % 2)), phase + i * 0.2f, muted + 0.12f)
    }
    drawOval(sceneFill(GardenHill, p, 0.45f), Offset(-w * 0.1f, h * 0.70f), Size(w * 0.8f, h * 0.48f))
    drawOval(sceneFill(GardenHillDark, p, 0.50f), Offset(w * 0.4f, h * 0.76f), Size(w * 0.8f, h * 0.46f))
    // 花枝从画面两角伸进来，顶住空旷的天空。
    drawFloweringBranch(
        p, Offset(-w * 0.03f, h * 0.06f), Offset(w * 0.27f, h * 0.22f),
        u * 0.52f, phase, muted, BlossomPink, seed = 0.2f,
    )
    drawFloweringBranch(
        p, Offset(w * 1.03f, h * 0.04f), Offset(w * 0.70f, h * 0.21f),
        u * 0.48f, phase + 0.4f, muted, BlossomWhite, seed = 0.6f,
    )
    // 两侧花藤：从地面爬到画面上沿，根与梢都在画面里。
    drawClimbingVine(
        p, Offset(w * 0.035f, h * 1.02f), Offset(w * 0.06f, h * 0.20f),
        u * 0.62f, phase, muted, BlossomPink, flowers = 4, seed = 0.1f,
    )
    drawClimbingVine(
        p, Offset(w * 0.965f, h * 1.02f), Offset(w * 0.94f, h * 0.24f),
        u * 0.58f, phase + 0.5f, muted, BlossomWhite, flowers = 4, seed = 0.7f,
    )
    // 向日葵：茎一直扎到地平线，花头在视平线上。
    drawSunflower(p, Offset(w * 0.10f, h * 0.50f), u * 0.18f, phase, muted)
    drawSunflower(p, Offset(w * 0.90f, h * 0.54f), u * 0.16f, phase + 0.5f, muted)
    // 前景花圃：五丛花全部从地面长出来。
    for ((i, x) in listOf(0.14f, 0.32f, 0.50f, 0.68f, 0.86f).withIndex()) {
        drawFlowerBed(
            p, Offset(w * x, h * 0.95f), u * 0.22f, u * 0.15f,
            phase + i * 0.17f, muted,
            listOf(BlossomPink, BlossomYellow, BlossomWhite, BlossomRed),
            count = 4,
        )
    }
    // 园丁 + 猫 + 兔：脚底有接地阴影。
    drawContactShadow(p, Offset(w * 0.50f, h * 0.80f + u * 0.15f), u * 0.26f, muted)
    drawContactShadow(p, Offset(w * 0.15f, h * 0.80f + u * 0.16f), u * 0.26f, muted)
    drawContactShadow(p, Offset(w * 0.86f, h * 0.80f + u * 0.14f), u * 0.24f, muted)
    drawPersonFigure(p, Offset(w * 0.50f, h * 0.80f), u * 0.24f, phase, muted, detail = 0.45f)
    drawWateringCan(p, Offset(w * 0.59f, h * 0.84f), u * 0.09f, phase + 0.2f, muted)
    drawCatFigure(p, Offset(w * 0.14f, h * 0.80f), u * 0.22f, phase + 0.5f, muted, detail = 0.4f)
    drawRabbitFigure(p, Offset(w * 0.86f, h * 0.80f), u * 0.20f, phase + 0.3f, muted, detail = 0.4f)
    // 蜜蜂 + 蝴蝶。
    drawBee(p, Offset(w * 0.30f + sin(phase * 2f * PI.toFloat()) * w * 0.03f, h * 0.34f), u * 0.06f, phase, muted)
    drawBee(p, Offset(w * 0.68f + cos(phase * 2f * PI.toFloat()) * w * 0.02f, h * 0.28f), u * 0.045f, phase + 0.5f, muted)
    drawButterfly(p, Offset(w * 0.42f, h * 0.26f), u * 0.05f, phase + 0.2f, muted)
    drawPetals(p, w, h, phase, muted)
}

internal fun DrawScope.drawGardenHalo(
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
    drawSunflower(p, Offset(topXs[0], barTop - u * 0.42f), u * 0.48f, phase, muted)
    drawBee(p, Offset(topXs[1], barTop - u * 0.48f), u * 0.38f, phase, muted)
    drawPersonFigure(p, Offset(topXs[2], barTop - u * 0.20f), u * 0.62f, phase, muted, detail = 1f)
    drawWateringCan(p, Offset(topXs[3], barTop - u * 0.38f), u * 0.50f, phase + 0.2f, muted)
    drawFlowerCluster(p, Offset(bottomXs[0], barBottom + u * 0.40f), u * 0.42f, phase + 0.1f, muted, hanging = true)
    drawFlowerCluster(p, Offset(bottomXs[1], barBottom + u * 0.36f), u * 0.36f, phase + 0.5f, muted, hanging = true)
    drawFlowerCluster(p, Offset(bottomXs[2], barBottom + u * 0.42f), u * 0.40f, phase + 0.3f, muted, hanging = true)
    drawButterfly(p, Offset(topXs[1], barTop - u * 0.75f), u * 0.30f, phase + 0.4f, muted)
}

// ---------- 星夜 ----------

internal fun DrawScope.drawNightBackdrop(p: ScenePalette, phase: Float, muted: Float) {
    val w = size.width
    val h = size.height
    if (w < 2f || h < 2f) return
    val u = min(w, h)
    drawRect(
        brush = androidx.compose.ui.graphics.Brush.verticalGradient(
            0f to sceneFill(NightSky, p, (muted * 0.45f).coerceAtLeast(0.22f)),
            0.55f to sceneFill(NightHillFar, p, (muted * 0.4f).coerceAtLeast(0.18f)),
            1f to p.base,
        ),
        size = size,
    )
    drawStars(p, w, h, phase, muted)
    drawMoon(p, Offset(w * 0.82f, h * 0.18f), u * 0.12f, phase, muted)
    // 远山与树影。
    drawOval(sceneFill(NightHillFar, p, 0.4f), Offset(-w * 0.1f, h * 0.70f), Size(w * 0.8f, h * 0.5f))
    drawOval(sceneFill(NightHill, p, 0.45f), Offset(w * 0.4f, h * 0.76f), Size(w * 0.8f, h * 0.48f))
    drawTree(p, Offset(w * 0.06f, h * 0.84f), u * 0.42f, phase, muted, BotanyLeafDark, Color(0xFF243B33), BlossomWhite)
    drawTree(p, Offset(w * 0.94f, h * 0.86f), u * 0.38f, phase + 0.4f, muted, BotanyLeafDark, Color(0xFF243B33), BlossomPink)
    // 栅栏 + 坐在上面的猫。
    val fenceTop = h * 0.72f
    drawFence(p, w * 0.04f, fenceTop, w * 0.92f, h * 0.14f, muted)
    drawCatFigure(p, Offset(w * 0.22f, fenceTop - u * 0.12f), u * 0.18f, phase, muted, detail = 0.6f)
    // 提灯的小人：灯笼挂在手上，不再悬空。
    val personC = Offset(w * 0.62f, h * 0.80f)
    val personS = u * 0.22f
    drawPersonFigure(p, personC, personS, phase + 0.3f, muted, detail = 0.55f)
    val hand = Offset(personC.x + personS * 0.23f, personC.y - personS * 0.28f)
    val lanternC = Offset(hand.x + u * 0.02f, hand.y - u * 0.03f)
    drawLine(
        sceneEdge(p, 0.45f),
        hand,
        Offset(lanternC.x, lanternC.y - u * 0.075f),
        u * 0.006f,
        StrokeCap.Round,
    )
    drawLantern(p, lanternC, u * 0.09f, phase + 0.2f, muted)
    drawContactShadow(p, Offset(personC.x, personC.y + u * 0.14f), u * 0.26f, muted)
    // 花枝与花藤：夜色里也把花接满。
    drawFloweringBranch(
        p, Offset(-w * 0.02f, h * 0.06f), Offset(w * 0.25f, h * 0.21f),
        u * 0.40f, phase, muted, BlossomWhite, seed = 0.2f,
    )
    drawFloweringBranch(
        p, Offset(w * 1.02f, h * 0.05f), Offset(w * 0.75f, h * 0.19f),
        u * 0.38f, phase + 0.4f, muted, BlossomPink, seed = 0.7f,
    )
    drawClimbingVine(
        p, Offset(w * 0.03f, h * 1.02f), Offset(w * 0.05f, h * 0.24f),
        u * 0.46f, phase, muted, BlossomWhite, flowers = 3, seed = 0.3f,
    )
    drawClimbingVine(
        p, Offset(w * 0.97f, h * 1.02f), Offset(w * 0.95f, h * 0.26f),
        u * 0.42f, phase + 0.5f, muted, BlossomPink, flowers = 3, seed = 0.9f,
    )
    // 前景花圃。
    for ((i, x) in listOf(0.12f, 0.46f, 0.88f).withIndex()) {
        drawFlowerBed(
            p, Offset(w * x, h * 0.98f), u * 0.22f, u * 0.13f,
            phase + i * 0.2f, muted, listOf(BlossomWhite, BlossomPink, BlossomYellow), count = 4,
        )
    }
    fireflies(p, w, h, u, phase, muted, listOf(0.30f to 0.55f, 0.55f to 0.45f, 0.76f to 0.58f, 0.16f to 0.68f))
}

internal fun DrawScope.drawNightHalo(
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
    drawMoon(p, Offset(topXs[0], barTop - u * 0.55f), u * 0.32f, phase, muted)
    drawCatFigure(p, Offset(topXs[1], barTop - u * 0.28f), u * 0.68f, phase, muted, detail = 1f)
    drawPersonFigure(p, Offset(topXs[2], barTop - u * 0.20f), u * 0.62f, phase + 0.3f, muted, detail = 1f)
    drawLantern(p, Offset(topXs[3], barTop - u * 0.45f), u * 0.48f, phase + 0.2f, muted)
    // 条上沿的几颗小星。
    for ((i, x) in listOf(0.18f, 0.42f, 0.66f, 0.90f).withIndex()) {
        val twinkle = 0.35f + 0.65f * abs(sin((phase + i * 0.17f) * SCENE_TAU * 1.5f))
        val c = Offset(size.width * x, u * 0.18f)
        drawCircle(sceneFill(Color(0xFFFFF3C4), p, muted).copy(alpha = 0.8f * twinkle), u * 0.035f, c)
    }
    drawFlowerCluster(p, Offset(bottomXs[0], barBottom + u * 0.40f), u * 0.38f, phase + 0.2f, 0.30f, hanging = true)
    drawVine(p, Offset(bottomXs[1], barBottom - u * 0.05f), u * 0.72f, phase + 0.4f, muted, flip = false)
    drawFlowerCluster(p, Offset(bottomXs[2], barBottom + u * 0.42f), u * 0.34f, phase + 0.6f, 0.30f, hanging = true)
    fireflies(p, size.width, size.height, u, phase, muted, listOf(0.16f to 0.32f, 0.44f to 0.28f, 0.72f to 0.34f, 0.34f to 0.72f))
}

// ---------- 共用 ----------

internal fun DrawScope.fireflies(
    p: ScenePalette,
    w: Float,
    h: Float,
    u: Float,
    phase: Float,
    muted: Float,
    spots: List<Pair<Float, Float>>,
) {
    for ((i, spot) in spots.withIndex()) {
        val drift = sin((phase + i * 0.21f) * SCENE_TAU)
        drawFirefly(
            p,
            Offset(spot.first * w + drift * u * 0.10f, spot.second * h + cos((phase + i * 0.33f) * SCENE_TAU) * u * 0.06f),
            u * 0.035f,
            phase + i * 0.13f,
            muted,
        )
    }
}
