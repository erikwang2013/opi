// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 动态背景里的**自然元素**：云、藤蔓、花丛、蝴蝶、飘落的花瓣。
//! 与 `OpiSceneFigures.kt` 同一套调色（sceneFill/sceneEdge）和相位（sceneSway）。

package xyz.erik.opi.scene

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.sin

internal val LeafGreen = Color(0xFF7DB98A)
internal val LeafDark = Color(0xFF4E8B66)
private val PetalCoral = Color(0xFFEE7D6B)
private val PetalYellow = Color(0xFFF6C85F)
private val PetalPink = Color(0xFFF3A7B2)
private val WingBlue = Color(0xFF8FC7D9)
private val GrassGreen = Color(0xFF6FAF7A)

private fun sceneGrass(p: ScenePalette, muted: Float): Color = sceneFill(GrassGreen, p, muted)
private fun sceneLeaf(p: ScenePalette, muted: Float, dark: Boolean = false): Color =
    sceneFill(if (dark) LeafDark else LeafGreen, p, muted)

/** 云：三团重叠的椭圆，压到几乎只剩明度差。 */
internal fun DrawScope.drawCloud(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val fill = sceneFill(Color.White, p, (muted * 0.5f + 0.15f).coerceAtMost(0.6f))
    val dx = sin(phase * SCENE_TAU + s) * s * 0.12f
    drawOval(fill, Offset(center.x - s + dx, center.y - s * 0.25f), Size(s * 1.05f, s * 0.65f))
    drawOval(fill, Offset(center.x - s * 0.25f + dx, center.y - s * 0.55f), Size(s * 1.0f, s * 0.85f))
    drawOval(fill, Offset(center.x + s * 0.45f + dx, center.y - s * 0.2f), Size(s * 0.8f, s * 0.55f))
}

/** 垂下的藤蔓：一条弯茎 + 三片叶子。 */
internal fun DrawScope.drawVine(
    p: ScenePalette,
    start: Offset,
    length: Float,
    phase: Float,
    muted: Float,
    flip: Boolean,
) {
    val dir = if (flip) -1f else 1f
    val swayX = sceneSway(phase, if (flip) 0.7f else 0.2f) * length * 0.04f
    val stem = Path().apply {
        moveTo(start.x, start.y)
        quadraticTo(
            start.x + dir * length * 0.16f + swayX,
            start.y + length * 0.45f,
            start.x + swayX * 1.4f,
            start.y + length,
        )
    }
    drawPath(stem, sceneLeaf(p, muted, dark = true), style = Stroke(length * 0.035f, cap = StrokeCap.Round))
    for (t in listOf(0.22f, 0.48f, 0.74f)) {
        val lx = start.x + dir * length * 0.10f * (1f - t) + swayX * t
        val ly = start.y + length * t
        val leafW = length * 0.16f
        val leafH = length * 0.09f
        rotate(degrees = dir * (25f + t * 20f), pivot = Offset(lx, ly)) {
            drawOval(sceneLeaf(p, muted), Offset(lx - leafW / 2f, ly - leafH / 2f), Size(leafW, leafH))
        }
    }
}

/** 一丛花：两朵花 + 草叶；[hanging] = 从条下沿垂下来（桌面状态栏）。 */
internal fun DrawScope.drawFlowerCluster(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
    hanging: Boolean = false,
) {
    val down = if (hanging) 1f else -1f
    val baseY = center.y
    // 草叶先画，花茎压在上面。
    for (i in 0..2) {
        val gx = center.x + (i - 1) * s * 0.30f
        val lean = sceneSway(phase, 0.1f * i) * s * 0.08f
        drawPath(
            Path().apply {
                moveTo(gx, baseY)
                quadraticTo(gx + lean, baseY + down * s * 0.34f, gx + lean * 1.8f, baseY + down * s * 0.50f)
            },
            sceneGrass(p, muted),
            style = Stroke(s * 0.035f, cap = StrokeCap.Round),
        )
    }
    val heads = listOf(
        Offset(center.x - s * 0.24f, baseY + down * s * 0.66f),
        Offset(center.x + s * 0.28f, baseY + down * s * 0.82f),
    )
    // 花茎（先于花头，免得压在花瓣上）。
    for ((fx, fy) in heads) {
        drawLine(
            sceneLeaf(p, muted, dark = true),
            Offset(fx, fy + down * s * 0.12f),
            Offset(center.x + (fx - center.x) * 0.35f, baseY),
            strokeWidth = s * 0.035f,
            cap = StrokeCap.Round,
        )
    }
    flowerHead(p, heads[0], s * 0.42f, phase, muted, PetalCoral)
    flowerHead(p, heads[1], s * 0.34f, phase + 0.3f, muted, PetalYellow)
}

/** 一朵五瓣花：花瓣用圆，花心用强调色。 */
private fun DrawScope.flowerHead(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
    petal: Color,
) {
    val tilt = sceneSway(phase, 0.4f) * 5f
    rotate(degrees = tilt, pivot = center) {
        val petalR = s * 0.34f
        for (i in 0 until 5) {
            val a = i * SCENE_TAU / 5f - PI.toFloat() / 2f
            drawCircle(
                sceneFill(petal, p, muted),
                radius = petalR,
                center = Offset(center.x + sin(a) * s * 0.34f, center.y - cos(a) * s * 0.34f),
            )
        }
        drawCircle(sceneFill(PetalYellow, p, muted), radius = s * 0.20f, center = center)
        drawCircle(sceneEdge(p, 0.16f), radius = s * 0.20f, center = center, style = Stroke(s * 0.03f))
    }
}

/** 蝴蝶：翅膀开合 + 缓慢漂移由调用方给中心点。 */
internal fun DrawScope.drawButterfly(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val flap = 0.55f + 0.45f * abs(sin(phase * SCENE_TAU * 2f))
    val fill = sceneFill(WingBlue, p, muted)
    val edge = sceneEdge(p, 0.45f)
    val body = sceneFill(Color(0xFF5B4038), p, muted)
    for (dir in listOf(-1f, 1f)) {
        rotate(degrees = dir * 22f, pivot = center) {
            drawOval(
                fill,
                Offset(center.x + dir * s * 0.10f - s * 0.26f, center.y - s * 0.20f),
                Size(s * 0.44f * flap + s * 0.10f, s * 0.38f),
            )
            drawOval(
                fill.copy(alpha = 0.75f),
                Offset(center.x + dir * s * 0.10f - s * 0.18f, center.y + s * 0.02f),
                Size(s * 0.36f * flap + s * 0.08f, s * 0.30f),
            )
        }
    }
    drawLine(body, Offset(center.x, center.y - s * 0.28f), Offset(center.x, center.y + s * 0.30f), s * 0.09f, StrokeCap.Round)
    drawLine(edge, Offset(center.x, center.y - s * 0.26f), Offset(center.x - s * 0.16f, center.y - s * 0.44f), s * 0.025f, StrokeCap.Round)
    drawLine(edge, Offset(center.x, center.y - s * 0.26f), Offset(center.x + s * 0.16f, center.y - s * 0.44f), s * 0.025f, StrokeCap.Round)
}

/** 飘落的花瓣：位置只由相位决定，任何一帧都稳定。 */
internal fun DrawScope.drawPetals(
    p: ScenePalette,
    w: Float,
    h: Float,
    phase: Float,
    muted: Float,
) {
    val colors = listOf(PetalPink, PetalCoral, PetalYellow)
    for (i in 0 until 7) {
        val t = (phase + i * 0.137f) % 1f
        val x = ((i * 0.173f + t * 0.22f) % 1f) * w
        val y = ((i * 0.291f + t * 0.42f) % 1f) * h
        val s = (0.018f + 0.006f * (i % 3)) * minOf(w, h)
        rotate(degrees = t * 180f + i * 40f, pivot = Offset(x, y)) {
            drawOval(
                sceneFill(colors[i % colors.size], p, muted).copy(alpha = 0.55f),
                Offset(x - s, y - s * 0.45f),
                Size(s * 2f, s * 0.9f),
            )
        }
    }
}
