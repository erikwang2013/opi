// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 城市主题的道具：楼群、路灯、小车、小鸟。颜色/相位与其它主题同一套
//! （sceneFill/sceneEdge/sceneSway），只在 `OpiSceneThemesCity.kt` 里编排。

package xyz.erik.opi.scene

import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import kotlin.math.sin

private val CityBuilding = Color(0xFF6B7A99)
private val CityBuildingDark = Color(0xFF55617D)
private val CityWindow = Color(0xFFFFD98A)
private val CityRoof = Color(0xFF46516A)
private val CityRoad = Color(0xFF4A5266)
private val LampPole = Color(0xFF46516A)
private val LampGlow = Color(0xFFFFE3A0)
private val CarBody = Color(0xFFE8826A)
private val CarDark = Color(0xFF4A5266)
private val BirdColor = Color(0xFF5B6678)

/** 一排楼：五栋高低错落，窗户按相位明灭；[baseY] = 地面。 */
internal fun DrawScope.drawBuildingRow(
    p: ScenePalette,
    left: Float,
    baseY: Float,
    width: Float,
    height: Float,
    phase: Float,
    muted: Float,
) {
    val heights = floatArrayOf(0.62f, 0.92f, 0.52f, 1.0f, 0.74f)
    val bw = width / heights.size
    for ((i, hf) in heights.withIndex()) {
        val bh = height * hf
        val x = left + i * bw + bw * 0.08f
        val w = bw * 0.84f
        val top = baseY - bh
        val fill = sceneFill(if (i % 2 == 0) CityBuilding else CityBuildingDark, p, muted)
        drawRect(fill, Offset(x, top), Size(w, bh))
        drawRect(sceneFill(CityRoof, p, muted), Offset(x, top - height * 0.035f), Size(w, height * 0.035f))
        // 窗户：三列，亮灯窗带闪烁。
        val cols = 3
        val rows = 4
        for (r in 0 until rows) for (c in 0 until cols) {
            val wx = x + w * (0.16f + c * 0.28f)
            val wy = top + bh * (0.12f + r * 0.20f)
            val lit = (i * 7 + r * 3 + c) % 5 != 0
            val alpha = if (lit) 0.65f + 0.35f * sin((phase + i * 0.13f + r * 0.07f) * SCENE_TAU).coerceAtLeast(0f) else 0.16f
            drawRect(
                sceneFill(CityWindow, p, muted).copy(alpha = alpha),
                Offset(wx, wy),
                Size(w * 0.14f, bh * 0.085f),
            )
        }
    }
    // 路面。
    drawLine(sceneFill(CityRoad, p, muted), Offset(left, baseY), Offset(left + width, baseY), height * 0.05f)
}

/** 路灯：弯臂 + 暖光。[center] 是灯杆底部。 */
internal fun DrawScope.drawStreetLamp(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val pole = sceneFill(LampPole, p, muted)
    val glow = 0.70f + 0.30f * sceneSway(phase, 0.4f)
    drawLine(pole, Offset(center.x, center.y), Offset(center.x, center.y - s * 1.15f), s * 0.055f, StrokeCap.Round)
    drawLine(pole, Offset(center.x, center.y - s * 1.15f), Offset(center.x + s * 0.28f, center.y - s * 1.15f), s * 0.045f, StrokeCap.Round)
    val lamp = Offset(center.x + s * 0.30f, center.y - s * 1.08f)
    drawCircle(sceneFill(LampGlow, p, muted).copy(alpha = 0.18f * glow), s * 0.55f, lamp)
    drawCircle(sceneFill(LampGlow, p, muted), s * 0.14f, lamp)
    drawLine(pole, Offset(lamp.x, lamp.y - s * 0.12f), Offset(lamp.x, lamp.y - s * 0.24f), s * 0.04f, StrokeCap.Round)
}

/** 小车：圆角车身 + 车顶 + 两个轮子。[center] 是车身中心。 */
internal fun DrawScope.drawCar(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val body = sceneFill(CarBody, p, muted)
    val dark = sceneFill(CarDark, p, muted)
    val edge = sceneEdge(p, 0.35f)
    val bob = sceneSway(phase, 0.7f) * s * 0.02f
    val c = Offset(center.x, center.y + bob)
    drawRoundRect(body, Offset(c.x - s * 0.55f, c.y - s * 0.16f), Size(s * 1.10f, s * 0.34f), CornerRadius(s * 0.10f))
    drawRoundRect(body, Offset(c.x - s * 0.28f, c.y - s * 0.40f), Size(s * 0.54f, s * 0.30f), CornerRadius(s * 0.08f))
    drawRoundRect(edge, Offset(c.x - s * 0.55f, c.y - s * 0.16f), Size(s * 1.10f, s * 0.34f), CornerRadius(s * 0.10f), style = Stroke(s * 0.025f))
    drawRect(sceneFill(Color(0xFFBFE4EF), p, (muted + 0.15f).coerceAtMost(0.8f)), Offset(c.x - s * 0.22f, c.y - s * 0.36f), Size(s * 0.42f, s * 0.20f))
    for (dir in listOf(-1f, 1f)) {
        drawCircle(dark, s * 0.13f, Offset(c.x + dir * s * 0.34f, c.y + s * 0.18f))
        drawCircle(sceneFill(Color.White, p, muted), s * 0.055f, Offset(c.x + dir * s * 0.34f, c.y + s * 0.18f))
    }
}

/** 小鸟：两笔 V 形翅膀，扑翼由相位控制；用于城市/山川/大海。 */
internal fun DrawScope.drawBird(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
) {
    val flap = 0.55f + 0.45f * sin(phase * SCENE_TAU * 2f + center.x * 0.01f)
    val color = sceneFill(BirdColor, p, muted)
    drawPath(
        Path().apply {
            moveTo(center.x - s, center.y + s * 0.25f * flap)
            quadraticTo(center.x - s * 0.45f, center.y - s * 0.35f * flap, center.x, center.y)
            quadraticTo(center.x + s * 0.45f, center.y - s * 0.35f * flap, center.x + s, center.y + s * 0.25f * flap)
        },
        color,
        style = Stroke(s * 0.12f, cap = StrokeCap.Round),
    )
}
