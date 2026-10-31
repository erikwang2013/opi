// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 插画里的**花草树木**：花茎、花丛、花枝、花藤、树冠、灌木、花箱与接地阴影。
//!
//! 2026-10-31 用户裁决：插画要「清晰、生动、有观感」，补上花藤/花枝，把不连贯的
//! 景物接起来。这里的花都从**根部的 base 点**长出来，枝从树干/画面边缘伸出，
//! 藤从地面爬到高处 —— 调用方只要给根点，不另外画悬空的花头。

package xyz.erik.opi.scene

import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin

internal val BotanyLeaf = Color(0xFF6FA85F)
internal val BotanyLeafLight = Color(0xFF9CCB7E)
internal val BotanyLeafDark = Color(0xFF3F7448)
internal val BotanyStem = Color(0xFF4C7A45)
internal val BotanyBranch = Color(0xFF7A5638)
internal val BotanyTrunk = Color(0xFF6B4A32)
internal val BlossomPink = Color(0xFFF39AAE)
internal val BlossomWhite = Color(0xFFFFF4E6)
internal val BlossomYellow = Color(0xFFF7C85C)
internal val BlossomRed = Color(0xFFE4675A)

/** 接地阴影：角色/花丛/道具脚下一小片压暗，画面才不是「浮在空中」。 */
internal fun DrawScope.drawContactShadow(
    p: ScenePalette,
    center: Offset,
    width: Float,
    muted: Float,
    alpha: Float = 0.16f,
) {
    if (width < 1f) return
    val color = if (p.base.luminance() < 0.3f) {
        sceneFill(Color.White, p, 0.9f).copy(alpha = alpha * 0.45f)
    } else {
        sceneFill(Color(0xFF3A2E22), p, 0.9f).copy(alpha = alpha)
    }
    drawOval(color, Offset(center.x - width * 0.5f, center.y - width * 0.07f), Size(width, width * 0.14f))
}

/** 草簇：从 [base] 向上散开的五根草叶。 */
internal fun DrawScope.drawGrassTuft(
    p: ScenePalette,
    base: Offset,
    s: Float,
    phase: Float,
    muted: Float,
    blades: Int = 5,
) {
    val dark = sceneFill(BotanyLeafDark, p, muted)
    val light = sceneFill(BotanyLeafLight, p, muted)
    for (i in 0 until blades) {
        val dx = (i - (blades - 1) / 2f) * s * 0.20f
        val lean = sceneSway(phase, i * 0.17f) * s * 0.10f + dx * 0.30f
        val hgt = s * (0.52f + 0.10f * (i % 3))
        drawPath(
            Path().apply {
                moveTo(base.x + dx, base.y)
                quadraticTo(base.x + dx + lean * 0.55f, base.y - hgt * 0.58f, base.x + dx + lean, base.y - hgt)
            },
            if (i % 2 == 0) dark else light,
            style = Stroke(s * 0.045f, cap = StrokeCap.Round),
        )
    }
}

/** 一片叶子：从 [at] 沿 [angle] 度方向伸出，带主脉与描边。 */
internal fun DrawScope.drawLeaf(
    p: ScenePalette,
    at: Offset,
    len: Float,
    angle: Float,
    phase: Float,
    muted: Float,
    color: Color = BotanyLeaf,
) {
    if (len < 0.5f) return
    rotate(degrees = angle + sceneSway(phase, 0.23f) * 3f, pivot = at) {
        val leaf = Path().apply {
            moveTo(at.x, at.y)
            quadraticTo(at.x + len * 0.42f, at.y - len * 0.36f, at.x + len, at.y)
            quadraticTo(at.x + len * 0.42f, at.y + len * 0.36f, at.x, at.y)
            close()
        }
        drawPath(leaf, sceneFill(color, p, muted))
        drawPath(leaf, sceneEdge(p, 0.32f), style = Stroke(len * 0.055f, join = StrokeJoin.Round))
        drawLine(
            sceneEdge(p, 0.18f),
            Offset(at.x + len * 0.06f, at.y),
            Offset(at.x + len * 0.82f, at.y),
            len * 0.035f,
            StrokeCap.Round,
        )
    }
}

/** 一朵花：花瓣 + 花心 + 描边。 */
internal fun DrawScope.drawBlossom(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
    petal: Color,
    petals: Int = 5,
) {
    if (s < 0.5f) return
    val edge = sceneEdge(p, 0.42f)
    val line = s * 0.045f
    rotate(degrees = sceneSway(phase, 0.4f) * 6f, pivot = center) {
        for (i in 0 until petals) {
            val a = i * SCENE_TAU / petals - PI.toFloat() / 2f
            val c = Offset(center.x + sin(a) * s * 0.44f, center.y - cos(a) * s * 0.44f)
            drawCircle(sceneFill(petal, p, muted), s * 0.37f, c)
            drawCircle(edge, s * 0.37f, c, style = Stroke(line))
        }
        drawCircle(sceneFill(BlossomYellow, p, muted), s * 0.27f, center)
        drawCircle(edge, s * 0.27f, center, style = Stroke(line))
        drawCircle(
            Color.White.copy(alpha = 0.55f),
            s * 0.08f,
            Offset(center.x - s * 0.08f, center.y - s * 0.08f),
        )
    }
}

/** 一根花茎：从 [base] 长到 [top]，沿途两片叶，顶端一朵花。 */
internal fun DrawScope.drawFlowerStalk(
    p: ScenePalette,
    base: Offset,
    top: Offset,
    s: Float,
    phase: Float,
    muted: Float,
    petal: Color,
    leaves: Int = 2,
    seed: Float = 0f,
) {
    if (s < 0.5f) return
    val sway = sceneSway(phase, seed) * s * 0.10f
    val tip = Offset(top.x + sway, top.y)
    val mid = Offset(base.x + (tip.x - base.x) * 0.30f + sway * 0.6f, (base.y + tip.y) * 0.5f)
    val stem = Path().apply {
        moveTo(base.x, base.y)
        quadraticTo(mid.x, mid.y, tip.x, tip.y + s * 0.22f)
    }
    drawPath(stem, sceneEdge(p, 0.35f), style = Stroke(s * 0.105f, cap = StrokeCap.Round))
    drawPath(stem, sceneFill(BotanyStem, p, muted), style = Stroke(s * 0.070f, cap = StrokeCap.Round))
    for (i in 0 until leaves) {
        val t = 0.32f + i * 0.26f
        val x = (1 - t) * (1 - t) * base.x + 2 * (1 - t) * t * mid.x + t * t * tip.x
        val y = (1 - t) * (1 - t) * base.y + 2 * (1 - t) * t * mid.y + t * t * (tip.y + s * 0.22f)
        val side = if (i % 2 == 0) -1f else 1f
        drawLeaf(
            p,
            Offset(x, y),
            s * 0.42f,
            if (side < 0) 205f else -25f,
            phase + seed + i * 0.1f,
            muted,
        )
    }
    drawBlossom(p, Offset(tip.x, tip.y), s * 0.55f, phase + seed, muted, petal)
}

/** 一丛花：土丘 + 草 + 五根花茎，全部从 [center] 这条地平线长出来。 */
internal fun DrawScope.drawFlowerBed(
    p: ScenePalette,
    center: Offset,
    width: Float,
    s: Float,
    phase: Float,
    muted: Float,
    petals: List<Color> = listOf(BlossomPink, BlossomYellow, BlossomWhite),
    count: Int = 5,
) {
    if (width < 1f || s < 0.5f) return
    drawOval(
        sceneFill(BotanyLeafDark, p, muted).copy(alpha = 0.45f),
        Offset(center.x - width * 0.55f, center.y - width * 0.045f),
        Size(width * 1.1f, width * 0.14f),
    )
    drawGrassTuft(p, Offset(center.x - width * 0.34f, center.y), s * 0.55f, phase, muted)
    drawGrassTuft(p, Offset(center.x + width * 0.33f, center.y), s * 0.45f, phase + 0.3f, muted)
    for (i in 0 until count) {
        val fx = center.x - width * 0.42f + width * 0.84f * i / (count - 1).coerceAtLeast(1)
        val hgt = s * (0.62f + 0.20f * ((i * 29) % 4) / 3f)
        drawFlowerStalk(
            p,
            Offset(fx, center.y),
            Offset(fx + (i - 2) * s * 0.05f, center.y - hgt),
            s * 0.52f,
            phase,
            muted,
            petals[i % petals.size],
            leaves = 2,
            seed = i * 0.13f,
        )
    }
}

/** 花枝：从 [start] 伸到 [end] 的曲枝，带叶与花；用于树冠/画面边缘/屋檐。 */
internal fun DrawScope.drawFloweringBranch(
    p: ScenePalette,
    start: Offset,
    end: Offset,
    s: Float,
    phase: Float,
    muted: Float,
    blossom: Color = BlossomPink,
    seed: Float = 0f,
) {
    if (s < 0.5f) return
    val mid = Offset(
        (start.x + end.x) * 0.5f + (end.y - start.y) * 0.12f,
        (start.y + end.y) * 0.5f - s * 0.35f,
    )
    val branch = Path().apply {
        moveTo(start.x, start.y)
        quadraticTo(mid.x, mid.y, end.x, end.y)
    }
    drawPath(branch, sceneEdge(p, 0.38f), style = Stroke(s * 0.12f, cap = StrokeCap.Round))
    drawPath(branch, sceneFill(BotanyBranch, p, muted), style = Stroke(s * 0.075f, cap = StrokeCap.Round))
    for ((i, t) in listOf(0.30f, 0.56f, 0.80f).withIndex()) {
        val x = (1 - t) * (1 - t) * start.x + 2 * (1 - t) * t * mid.x + t * t * end.x
        val y = (1 - t) * (1 - t) * start.y + 2 * (1 - t) * t * mid.y + t * t * end.y
        val side = if (i % 2 == 0) -1f else 1f
        drawLeaf(
            p,
            Offset(x, y),
            s * 0.28f,
            if (side < 0) 200f else -20f,
            phase + seed + i * 0.12f,
            muted,
        )
        drawBlossom(
            p,
            Offset(x + side * s * 0.18f, y - s * 0.20f),
            s * 0.14f,
            phase + seed + i * 0.2f,
            muted,
            blossom,
        )
    }
    drawBlossom(p, Offset(end.x, end.y), s * 0.17f, phase + seed, muted, blossom)
}

/** 花藤：从地面 [base] 爬到 [top]，沿途叶片、卷须与花；根与顶端都落在实景上。 */
internal fun DrawScope.drawClimbingVine(
    p: ScenePalette,
    base: Offset,
    top: Offset,
    s: Float,
    phase: Float,
    muted: Float,
    blossom: Color = BlossomWhite,
    flowers: Int = 3,
    seed: Float = 0f,
) {
    if (s < 0.5f) return
    val sway = sceneSway(phase, seed) * s * 0.10f
    val span = base.y - top.y
    val c1 = Offset(base.x - s * 0.35f + sway, base.y - span * 0.32f)
    val c2 = Offset(top.x + s * 0.40f + sway, top.y + span * 0.30f)
    val vine = Path().apply {
        moveTo(base.x, base.y)
        cubicTo(c1.x, c1.y, c2.x, c2.y, top.x, top.y)
    }
    drawPath(vine, sceneEdge(p, 0.35f), style = Stroke(s * 0.075f, cap = StrokeCap.Round))
    drawPath(vine, sceneFill(BotanyStem, p, muted), style = Stroke(s * 0.048f, cap = StrokeCap.Round))
    val leafTs = listOf(0.18f, 0.36f, 0.55f, 0.73f, 0.88f)
    for ((i, t) in leafTs.withIndex()) {
        val mt = 1 - t
        val x = mt * mt * mt * base.x + 3 * mt * mt * t * c1.x + 3 * mt * t * t * c2.x + t * t * t * top.x
        val y = mt * mt * mt * base.y + 3 * mt * mt * t * c1.y + 3 * mt * t * t * c2.y + t * t * t * top.y
        val side = if (i % 2 == 0) -1f else 1f
        drawLeaf(p, Offset(x, y), s * (0.20f + 0.03f * (i % 3)), if (side < 0) 195f else -15f, phase + seed + i * 0.1f, muted)
        if (i % 2 == 1 && flowers > 0) {
            drawBlossom(p, Offset(x + side * s * 0.08f, y - s * 0.05f), s * 0.13f, phase + seed + i * 0.2f, muted, blossom)
        }
    }
    for (i in 0 until flowers) {
        val t = 0.30f + 0.22f * i
        val mt = 1 - t
        val x = mt * mt * mt * base.x + 3 * mt * mt * t * c1.x + 3 * mt * t * t * c2.x + t * t * t * top.x
        val y = mt * mt * mt * base.y + 3 * mt * mt * t * c1.y + 3 * mt * t * t * c2.y + t * t * t * top.y
        drawBlossom(p, Offset(x, y), s * 0.15f, phase + seed + t, muted, blossom)
    }
    drawBlossom(p, top, s * 0.16f, phase + seed, muted, blossom)
}

/** 一棵树：树干 + 多球树冠 + 高光；[blossom] 非空时树冠上撒花。 */
internal fun DrawScope.drawTree(
    p: ScenePalette,
    base: Offset,
    s: Float,
    phase: Float,
    muted: Float,
    canopy: Color = BotanyLeaf,
    canopyDark: Color = BotanyLeafDark,
    blossom: Color? = null,
) {
    if (s < 1f) return
    val sway = sceneSway(phase, 0.2f) * s * 0.025f
    val trunkTop = Offset(base.x + sway * 0.4f, base.y - s * 0.55f)
    drawLine(sceneEdge(p, 0.38f), base, trunkTop, s * 0.16f, StrokeCap.Round)
    drawLine(sceneFill(BotanyTrunk, p, muted), base, trunkTop, s * 0.11f, StrokeCap.Round)
    for (dir in listOf(-1f, 1f)) {
        val end = Offset(trunkTop.x + dir * s * 0.34f + sway, trunkTop.y - s * 0.22f)
        drawLine(sceneEdge(p, 0.32f), trunkTop, end, s * 0.10f, StrokeCap.Round)
        drawLine(sceneFill(BotanyTrunk, p, muted), trunkTop, end, s * 0.06f, StrokeCap.Round)
    }
    val lobes = listOf(
        Triple(-0.38f, -0.78f, 0.34f),
        Triple(-0.05f, -0.98f, 0.40f),
        Triple(0.34f, -0.80f, 0.33f),
        Triple(-0.22f, -0.52f, 0.30f),
        Triple(0.20f, -0.54f, 0.30f),
    )
    for ((dx, dy, r) in lobes) {
        val c = Offset(base.x + dx * s + sway, base.y + dy * s)
        drawCircle(sceneEdge(p, 0.30f), s * (r + 0.025f), c)
        drawCircle(sceneFill(if (r > 0.35f) canopy else canopyDark, p, muted), s * r, c)
    }
    for ((dx, dy, r) in lobes.take(3)) {
        drawCircle(
            Color.White.copy(alpha = 0.20f),
            s * r * 0.45f,
            Offset(base.x + (dx - 0.10f) * s + sway, base.y + (dy - 0.12f) * s),
        )
    }
    if (blossom != null) {
        for ((i, spot) in listOf(
            -0.42f to -0.86f, -0.12f to -1.02f, 0.18f to -0.86f, 0.40f to -0.66f, -0.30f to -0.60f,
        ).withIndex()) {
            drawBlossom(
                p,
                Offset(base.x + spot.first * s + sway, base.y + spot.second * s),
                s * 0.12f,
                phase + i * 0.17f,
                muted,
                blossom,
            )
        }
    }
}

/** 灌木：三团高低错落的叶球 + 可选的浆果/小花。 */
internal fun DrawScope.drawShrub(
    p: ScenePalette,
    base: Offset,
    s: Float,
    phase: Float,
    muted: Float,
    flower: Color? = null,
) {
    if (s < 1f) return
    val sway = sceneSway(phase, 0.3f) * s * 0.02f
    for ((dx, dy, r) in listOf(Triple(-0.28f, -0.22f, 0.30f), Triple(0.02f, -0.34f, 0.36f), Triple(0.30f, -0.20f, 0.28f))) {
        val c = Offset(base.x + dx * s + sway, base.y + dy * s)
        drawCircle(sceneEdge(p, 0.30f), s * (r + 0.022f), c)
        drawCircle(sceneFill(BotanyLeaf, p, muted), s * r, c)
    }
    drawCircle(
        Color.White.copy(alpha = 0.18f),
        s * 0.16f,
        Offset(base.x - s * 0.04f + sway, base.y - s * 0.38f),
    )
    if (flower != null) {
        for ((i, spot) in listOf(-0.24f to -0.30f, 0.08f to -0.44f, 0.30f to -0.22f).withIndex()) {
            drawBlossom(
                p,
                Offset(base.x + spot.first * s + sway, base.y + spot.second * s),
                s * 0.12f,
                phase + i * 0.2f,
                muted,
                flower,
            )
        }
    }
}

/** 窗台/阳台花箱：陶盆 + 花叶 + 垂下的花藤；[center] 是花箱底部中心。 */
internal fun DrawScope.drawFlowerBox(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
    blossom: Color = BlossomPink,
) {
    if (s < 1f) return
    val boxW = s * 1.25f
    val boxH = s * 0.42f
    drawRoundRect(
        sceneFill(Color(0xFFB4644A), p, muted),
        Offset(center.x - boxW / 2f, center.y - boxH),
        Size(boxW, boxH),
        CornerRadius(s * 0.08f),
    )
    drawRoundRect(
        sceneEdge(p, 0.38f),
        Offset(center.x - boxW / 2f, center.y - boxH),
        Size(boxW, boxH),
        CornerRadius(s * 0.08f),
        style = Stroke(s * 0.035f),
    )
    for ((i, dx) in listOf(-0.38f, -0.10f, 0.20f, 0.44f).withIndex()) {
        drawGrassTuft(p, Offset(center.x + dx * s, center.y - boxH), s * 0.48f, phase + i * 0.2f, muted, blades = 4)
        drawBlossom(
            p,
            Offset(center.x + dx * s, center.y - boxH - s * (0.26f + 0.06f * (i % 2))),
            s * 0.14f,
            phase + i * 0.17f,
            muted,
            if (i % 2 == 0) blossom else BlossomWhite,
        )
    }
    for (dir in listOf(-1f, 1f)) {
        drawClimbingVine(
            p,
            Offset(center.x + dir * boxW * 0.34f, center.y - boxH * 0.65f),
            Offset(center.x + dir * boxW * 0.52f, center.y - boxH * 0.10f),
            s * 0.32f,
            phase,
            muted,
            blossom,
            flowers = 1,
            seed = if (dir < 0) 0.3f else 0.6f,
        )
    }
}
