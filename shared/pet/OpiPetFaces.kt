// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.pet

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke

/**
 * 小欧的**脸**：眼睛 / 腮红 / 情绪符号 / 嘴 / 捂脸的手。
 *
 * 从 `OpiPet.kt` 拆出（500 行门禁；几何真源仍是 docs/opi-pet.svg）。调用点在
 * `drawPet` 画完键帽正面之后 —— 这里画的每一样都在正面之上，捂脸的手最后画。
 */

/** 六形态的情绪点缀色（2026-10-17）：腮红（笑 / 欢乐）、泪（哭）、冷汗（惊恐）、怒红。 */
private val SMILE_PINK = Color(0xFFF5A6B4)
private val JOY_PINK = Color(0xFFF27D9D)
private val TEAR_BLUE = Color(0xFF4E97E6)
private val SWEAT_BLUE = Color(0xFF6FB2F0)
private val ANGER_RED = Color(0xFFDD5138)

/** 2026-10-18 追加六形态的点缀色（与 docs/opi-pet-forms.svg 同值）。 */
private val COQUETTE_PINK = Color(0xFFF46F9B)
private val CUTE_GOLD = Color(0xFFF2B84B)
private val PLEASING_MINT = Color(0xFF5FBF9F)
private val KNOCK_AMBER = Color(0xFFE8A13C)
private val PALM_RED = Color(0xFFE06A4E)
private val TONGUE_PINK = Color(0xFFEF5D8F)

/** 在键帽正面上画表情。参数是 `drawPet` 已算好的缩放与配色。 */
internal fun DrawScope.drawPetFace(
    mood: PetMood,
    u: Float,
    stroke: Float,
    face: Color,
    plate: Color,
    primary: Color,
    shade: Color,
) {
    val sleepy = mood == PetMood.SLEEPY
    val puzzled = mood == PetMood.PUZZLED
    val waiting = mood == PetMood.WAITING
    val crying = mood == PetMood.CRYING
    val terrified = mood == PetMood.TERRIFIED
    val angry = mood == PetMood.ANGRY
    val coquettish = mood == PetMood.COQUETTISH
    val cute = mood == PetMood.CUTE
    val pleasing = mood == PetMood.PLEASING
    val knocking = mood == PetMood.KNOCKING
    val facePalm = mood == PetMood.FACE_PALM
    val tongueOut = mood == PetMood.TONGUE_OUT

    // 眼睛（六形态）：睡着闭线、哭是挤紧的「><」、惊恐瞪圆、愤怒细眼配倒八字眉、
    // 欢乐弯弯笑眼、其余圆眼（沮丧左右不一样大）。
    when {
        sleepy -> {
            for (cx in listOf(93f, 147f)) {
                drawLine(
                    shade,
                    Offset((cx - 10f) * u, 96f * u),
                    Offset((cx + 10f) * u, 96f * u),
                    strokeWidth = stroke,
                    cap = StrokeCap.Round,
                )
            }
        }
        crying -> {
            for (dir in listOf(-1f, 1f)) {
                // 左眼 =「>」、右眼 =「<」：眼角朝脸中挤紧。
                val outer = (120f + dir * 36f) * u
                val tip = (120f + dir * 20f) * u
                drawLine(shade, Offset(outer, 87f * u), Offset(tip, 95f * u), strokeWidth = stroke, cap = StrokeCap.Round)
                drawLine(shade, Offset(outer, 103f * u), Offset(tip, 95f * u), strokeWidth = stroke, cap = StrokeCap.Round)
            }
        }
        terrified -> {
            // 瞪圆：描边大眼圈 + 小瞳孔。
            for (cx in listOf(93f, 147f)) {
                drawCircle(shade, 13f * u, Offset(cx * u, 95f * u), style = Stroke(4.5f * u))
                drawCircle(shade, 5.5f * u, Offset(cx * u, 95f * u))
            }
        }
        angry -> {
            drawCircle(shade, 9f * u, Offset(93f * u, 97f * u))
            drawCircle(shade, 9f * u, Offset(147f * u, 97f * u))
            // 倒八字眉：外高内低。
            drawLine(shade, Offset(80f * u, 78f * u), Offset(102f * u, 88f * u), strokeWidth = stroke, cap = StrokeCap.Round)
            drawLine(shade, Offset(160f * u, 78f * u), Offset(138f * u, 88f * u), strokeWidth = stroke, cap = StrokeCap.Round)
        }
        coquettish -> {
            // 撒娇：左眼眨成弯弧，右眼圆睁带高光。
            drawArc(
                color = shade,
                startAngle = 200f,
                sweepAngle = 140f,
                useCenter = false,
                topLeft = Offset(83f * u, 88f * u),
                size = Size(20f * u, 14f * u),
                style = Stroke(stroke, cap = StrokeCap.Round),
            )
            drawCircle(shade, 11f * u, Offset(147f * u, 95f * u))
            drawCircle(face, 3.6f * u, Offset(143.2f * u, 91.2f * u))
        }
        cute -> {
            // 卖萌：更大的圆眼 + 更大的高光，水汪汪。
            for (cx in listOf(93f, 147f)) {
                drawCircle(shade, 12f * u, Offset(cx * u, 95f * u))
                drawCircle(face, 5f * u, Offset((cx - 4f) * u, 90.5f * u))
                drawCircle(face, 1.8f * u, Offset((cx + 3.5f) * u, 100f * u))
            }
        }
        pleasing -> {
            // 讨好：白眼球 + 上抬的瞳孔（向上看），配薄荷色小菱形。
            for (cx in listOf(93f, 147f)) {
                drawCircle(plate, 12f * u, Offset(cx * u, 95f * u))
                drawCircle(shade, 8.5f * u, Offset(cx * u, 93f * u))
                drawCircle(plate.copy(alpha = 0.9f), 3f * u, Offset((cx - 3f) * u, 89.5f * u))
            }
        }
        knocking -> {
            // 敲打：专注眯眼，两条外高内低的斜线。
            drawLine(shade, Offset(82f * u, 96f * u), Offset(102f * u, 92f * u), strokeWidth = stroke, cap = StrokeCap.Round)
            drawLine(shade, Offset(138f * u, 92f * u), Offset(158f * u, 96f * u), strokeWidth = stroke, cap = StrokeCap.Round)
        }
        facePalm -> {
            // 捂脸：右脸被手盖住，只露一只挤紧的「>」眼。
            drawLine(shade, Offset(83f * u, 87f * u), Offset(100f * u, 95f * u), strokeWidth = stroke, cap = StrokeCap.Round)
            drawLine(shade, Offset(83f * u, 103f * u), Offset(100f * u, 95f * u), strokeWidth = stroke, cap = StrokeCap.Round)
        }
        tongueOut -> {
            // 吐舌头：左眼圆睁，右眼眨成弯弧（与撒娇左右相反）。
            drawCircle(shade, 11f * u, Offset(93f * u, 95f * u))
            drawCircle(face, 3.6f * u, Offset(89.2f * u, 91.2f * u))
            drawArc(
                color = shade,
                startAngle = 200f,
                sweepAngle = 140f,
                useCenter = false,
                topLeft = Offset(137f * u, 88f * u),
                size = Size(20f * u, 14f * u),
                style = Stroke(stroke, cap = StrokeCap.Round),
            )
        }
        waiting -> {
            // 欢乐：闭眼笑弧 ∩。
            for (cx in listOf(93f, 147f)) {
                drawArc(
                    color = shade,
                    startAngle = 200f,
                    sweepAngle = 140f,
                    useCenter = false,
                    topLeft = Offset((cx - 10f) * u, 88f * u),
                    size = Size(20f * u, 14f * u),
                    style = Stroke(stroke, cap = StrokeCap.Round),
                )
            }
        }
        else -> {
            val leftR = if (puzzled) 8f else 11f
            val rightR = if (puzzled) 12f else leftR
            drawCircle(shade, leftR * u, Offset(93f * u, 95f * u))
            drawCircle(shade, rightR * u, Offset(147f * u, 95f * u))
            if (!puzzled) {
                drawCircle(face, 3.6f * u, Offset(89.2f * u, 91.2f * u))
                drawCircle(face, 3.6f * u, Offset(143.2f * u, 91.2f * u))
            }
        }
    }

    // 沮丧：八字愁眉（内端高、外端低），把「困惑」的歪眼读成垂头丧气。
    if (puzzled) {
        drawLine(shade, Offset(84f * u, 84f * u), Offset(102f * u, 77f * u), strokeWidth = 4f * u, cap = StrokeCap.Round)
        drawLine(shade, Offset(156f * u, 84f * u), Offset(138f * u, 77f * u), strokeWidth = 4f * u, cap = StrokeCap.Round)
    }

    // 腮红（笑 / 欢乐 / 愤怒 + 新六形态）：情绪色之一。
    val blush = when {
        angry -> ANGER_RED
        waiting -> JOY_PINK
        mood == PetMood.IDLE -> SMILE_PINK
        coquettish -> COQUETTE_PINK
        cute -> SMILE_PINK
        pleasing -> PLEASING_MINT
        knocking -> KNOCK_AMBER
        facePalm -> PALM_RED
        tongueOut -> TONGUE_PINK
        else -> null
    }
    if (blush != null) {
        val alpha = if (angry) 0.30f else 0.38f
        drawOval(blush.copy(alpha = alpha), topLeft = Offset(66f * u, 112f * u), size = Size(22f * u, 11f * u))
        drawOval(blush.copy(alpha = alpha), topLeft = Offset(152f * u, 112f * u), size = Size(22f * u, 11f * u))
    }

    // 哭：外眼角下方蓝色泪珠 + 泪痕。
    if (crying) {
        for (dir in listOf(-1f, 1f)) {
            val x = (120f + dir * 36f) * u
            drawCircle(TEAR_BLUE, 3.2f * u, Offset(x, 108f * u))
            drawOval(
                TEAR_BLUE.copy(alpha = 0.9f),
                topLeft = Offset(x - 2.2f * u, 114f * u),
                size = Size(4.4f * u, 10f * u),
            )
        }
    }

    // 惊恐：右额角一滴冷汗（泪滴形 + 高光）。
    if (terrified) {
        val drop = Path().apply {
            moveTo(174f * u, 52f * u)
            cubicTo(182f * u, 61f * u, 182f * u, 72f * u, 174f * u, 72f * u)
            cubicTo(166f * u, 72f * u, 166f * u, 61f * u, 174f * u, 52f * u)
            close()
        }
        drawPath(drop, SWEAT_BLUE)
        drawCircle(Color.White.copy(alpha = 0.85f), 1.4f * u, Offset(171f * u, 64f * u))
    }

    // 愤怒：右额角的红色怒气符号（两笔交叉，「💢」的简写）。
    if (angry) {
        val cx = 170f * u
        val cy = 60f * u
        drawLine(ANGER_RED, Offset(cx - 8f * u, cy - 8f * u), Offset(cx + 8f * u, cy + 8f * u), strokeWidth = 4.5f * u, cap = StrokeCap.Round)
        drawLine(ANGER_RED, Offset(cx + 8f * u, cy - 8f * u), Offset(cx - 8f * u, cy + 8f * u), strokeWidth = 4.5f * u, cap = StrokeCap.Round)
    }

    // 撒娇：右脸颊旁一颗桃粉爱心（+ 一颗小爱心）。
    if (coquettish) {
        val hx = 166f * u
        val hy = 64f * u
        val heart = Path().apply {
            moveTo(hx, hy + 9f * u)
            cubicTo(hx - 14f * u, hy - 1f * u, hx - 10f * u, hy - 12f * u, hx, hy - 5f * u)
            cubicTo(hx + 10f * u, hy - 12f * u, hx + 14f * u, hy - 1f * u, hx, hy + 9f * u)
            close()
        }
        drawPath(heart, COQUETTE_PINK)
        drawCircle(COQUETTE_PINK.copy(alpha = 0.75f), 3f * u, Offset(184f * u, 50f * u))
    }

    // 卖萌：金色四角星（大小三颗）。
    if (cute) {
        for ((sx, sy, r) in listOf(Triple(170f, 60f, 8f), Triple(70f, 66f, 6f), Triple(182f, 86f, 4f))) {
            drawPath(fourPointStar(sx * u, sy * u, r * u), CUTE_GOLD)
        }
    }

    // 讨好：两颊外的薄荷色小菱形（与卖萌的金星在形状与颜色上都不同）。
    if (pleasing) {
        for ((sx, sy, r) in listOf(Triple(170f, 62f, 5f), Triple(70f, 62f, 4f))) {
            val diamond = Path().apply {
                moveTo(sx * u, (sy - r) * u)
                lineTo((sx + r) * u, sy * u)
                lineTo(sx * u, (sy + r) * u)
                lineTo((sx - r) * u, sy * u)
                close()
            }
            drawPath(diamond, PLEASING_MINT)
        }
    }

    // 敲打：右侧三道琥珀色动作线 + 一枚小十字星。
    if (knocking) {
        for (i in 0 until 3) {
            val ly = (70f + i * 12f) * u
            drawLine(KNOCK_AMBER, Offset(198f * u, ly), Offset(214f * u, ly), strokeWidth = 3.5f * u, cap = StrokeCap.Round)
        }
        drawLine(KNOCK_AMBER, Offset(207f * u, 100f * u), Offset(207f * u, 112f * u), strokeWidth = 3.5f * u, cap = StrokeCap.Round)
        drawLine(KNOCK_AMBER, Offset(201f * u, 106f * u), Offset(213f * u, 106f * u), strokeWidth = 3.5f * u, cap = StrokeCap.Round)
    }

    // 吐舌头：粉色舌头先画，嘴的 O 环压住舌尖接缝。
    if (tongueOut) {
        drawOval(TONGUE_PINK, topLeft = Offset(112f * u, 134f * u), size = Size(16f * u, 15f * u))
        drawLine(
            shade.copy(alpha = 0.6f),
            Offset(120f * u, 138f * u),
            Offset(120f * u, 146f * u),
            strokeWidth = 2f * u,
            cap = StrokeCap.Round,
        )
    }

    // 嘴：圆环 = 字母 O（品牌）。情绪只改大小与位置：哭竖着张大、惊恐收小、
    // 愤怒压扁、沮丧缩小偏下；其余保持 O。
    val mouthRx = when {
        terrified -> 6f
        puzzled -> 7f
        crying -> 8.5f
        angry -> 8f
        coquettish -> 8f
        cute -> 11f
        pleasing -> 8f
        knocking -> 9f
        facePalm -> 7f
        tongueOut -> 10f
        else -> 12f
    }
    val mouthRy = when {
        terrified -> 8f
        puzzled -> 6f
        crying -> 11f
        angry -> 6f
        coquettish -> 8f
        cute -> 9f
        pleasing -> 7f
        knocking -> 6f
        facePalm -> 8f
        tongueOut -> 9f
        else -> 10f
    }
    val mouthCy = if (puzzled || angry || knocking) 130f else 128f
    drawOval(
        color = shade,
        topLeft = Offset((120f - mouthRx) * u, (mouthCy - mouthRy) * u),
        size = Size(mouthRx * 2f * u, mouthRy * 2f * u),
        style = Stroke(stroke),
    )

    // 捂脸：手掌**最后画**，盖住右半张脸与半张嘴（先画会被嘴的 O 环穿透）。
    if (facePalm) {
        val palm = Path().apply {
            moveTo(124f * u, 146f * u)
            lineTo(124f * u, 82f * u)
            quadraticTo(124f * u, 64f * u, 144f * u, 64f * u)
            lineTo(172f * u, 64f * u)
            quadraticTo(186f * u, 64f * u, 186f * u, 82f * u)
            lineTo(186f * u, 132f * u)
            quadraticTo(186f * u, 148f * u, 168f * u, 148f * u)
            lineTo(142f * u, 148f * u)
            quadraticTo(124f * u, 148f * u, 124f * u, 146f * u)
            close()
        }
        drawPath(palm, face)
        drawPath(palm, primary, style = Stroke(stroke, join = StrokeJoin.Round))
        for (fx in listOf(142f, 156f, 170f)) {
            drawLine(
                primary.copy(alpha = 0.55f),
                Offset(fx * u, 68f * u),
                Offset(fx * u, 98f * u),
                strokeWidth = 2.5f * u,
                cap = StrokeCap.Round,
            )
        }
    }
}

/** 四角星（卖萌）：四个尖角 + 向内收的腰，纯路径无外部依赖。 */
private fun fourPointStar(cx: Float, cy: Float, r: Float): Path = Path().apply {
    val waist = r * 0.22f
    moveTo(cx, cy - r)
    quadraticTo(cx + waist, cy - waist, cx + r, cy)
    quadraticTo(cx + waist, cy + waist, cx, cy + r)
    quadraticTo(cx - waist, cy + waist, cx - r, cy)
    quadraticTo(cx - waist, cy - waist, cx, cy - r)
    close()
}
