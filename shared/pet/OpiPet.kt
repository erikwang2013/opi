package io.opi.input.pet

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
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
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * 项目宠物「小欧」—— 一枚键帽精灵（docs/opi-pet.svg 的 Compose 复刻）。
 *
 * 身体是键盘键帽（输入法的本体），正面是笑脸且嘴为字母 O（Open），头顶是拼音
 * 声调符号天线（朱砂色，全图唯一暖色），侧壁刻 OPI，底部有按键涟漪。
 *
 * 坐标沿用 SVG 的 240x250 设计空间，按目标尺寸等比缩放 —— 改宠物只需同步
 * docs/opi-pet.svg 与这里的常量，二者共享同一套几何。
 */

/** 宠物情绪。由输入状态推导，不自己维护状态。 */
enum class PetMood { IDLE, WAITING, PUZZLED, SLEEPY }

/**
 * 由当前输入状态推导情绪（纯函数，无 Android 依赖，JVM 单测覆盖）。
 *
 * - 学习关闭 → [PetMood.SLEEPY] 睡着了，不记事
 * - 缓冲为空 → [PetMood.IDLE] 安静待命（英文/数字模式直传时同样是空缓冲）
 * - 有候选 → [PetMood.WAITING] 竖起天线等你挑
 * - 有缓冲无候选 → [PetMood.PUZZLED] 乱码拼音，它也没辙
 */
fun petMood(buffer: String, candidateCount: Int, learnerEnabled: Boolean): PetMood = when {
    !learnerEnabled -> PetMood.SLEEPY
    buffer.isEmpty() -> PetMood.IDLE
    candidateCount > 0 -> PetMood.WAITING
    else -> PetMood.PUZZLED
}

/**
 * 小欧的配色。默认值即 [Default]（浅色底，与 docs/opi-pet.svg 一致）；
 * 深色底宿主传 [Dark]，否则青色描边会糊进深色背景、键帽棱线消失。
 */
data class PetPalette(
    /** 青：结构描边 */
    val primary: Color = Color(0xFF2E8B80),
    /** 深青：眼睛与刻字 */
    val shade: Color = Color(0xFF1E6B63),
    /** 朱砂：声调天线（全图唯一暖色） */
    val accent: Color = Color(0xFFD9553F),
    /** 键帽侧壁 */
    val skirt: Color = Color(0xFFE9F3F1),
    /** 键帽正面 */
    val plate: Color = Color.White,
) {
    companion object {
        /** 浅色底（Android 设置页 / 候选栏）。 */
        val Default = PetPalette()

        /**
         * 深色底（Windows 候选窗 #2D2D2D）：描边与天线提亮，键帽正面压到近白但
         * 不纯白，避免在深色窗里过曝成一块死白。
         */
        val Dark = PetPalette(
            primary = Color(0xFF57C9BB),
            shade = Color(0xFF1E6B63),
            accent = Color(0xFFE8705A),
            skirt = Color(0xFFD6EAE7),
            plate = Color(0xFFF6FAFA),
        )
    }
}

/** 设计空间（与 SVG viewBox 相同）。 */
private const val CANVAS_W = 240f
private const val CANVAS_H = 250f

/**
 * 画一只小欧。
 *
 * @param mood 情绪，决定眼睛、嘴与天线的形态；用 [petMood] 从输入状态推导。
 * @param palette 配色；深色底宿主传 [PetPalette.Dark]。
 * @param size 键帽宽度（高度按 240:250 自动推导）。
 * @param showLegend 是否在侧壁刻 OPI —— 小于 64dp 时刻字只剩几像素高，会糊成
 *   一团脏点，默认按尺寸自动关闭（候选栏里的小欧就不带刻字）。
 */
@Composable
fun OpiPet(
    mood: PetMood = PetMood.IDLE,
    modifier: Modifier = Modifier,
    palette: PetPalette = PetPalette.Default,
    size: Dp = 72.dp,
    showLegend: Boolean = size >= 64.dp,
) {
    val measurer = rememberTextMeasurer()
    Canvas(modifier.size(width = size, height = size * CANVAS_H / CANVAS_W)) {
        val u = this.size.width / CANVAS_W // 设计单位 → 像素
        drawPet(mood, u, measurer, showLegend, palette)
    }
}

private fun DrawScope.drawPet(
    mood: PetMood,
    u: Float,
    measurer: TextMeasurer,
    legend: Boolean,
    palette: PetPalette,
) {
    val primary = palette.primary
    val shade = palette.shade
    val accent = palette.accent
    val skirt = palette.skirt
    val plate = palette.plate
    val stroke = 5f * u
    val sleepy = mood == PetMood.SLEEPY
    val puzzled = mood == PetMood.PUZZLED
    val waiting = mood == PetMood.WAITING

    // 天线：声调符号 ˉ（一声）。睡着时垂下来。
    rotate(degrees = if (sleepy) 24f else 0f, pivot = Offset(120f * u, 48f * u)) {
        drawLine(
            color = primary,
            start = Offset(120f * u, 52f * u),
            end = Offset(120f * u, if (waiting) 24f * u else 28f * u),
            strokeWidth = stroke,
            cap = StrokeCap.Round,
        )
        drawLine(
            color = accent,
            start = Offset(103f * u, if (waiting) 14f * u else 20f * u),
            end = Offset(137f * u, if (waiting) 14f * u else 20f * u),
            strokeWidth = 7f * u,
            cap = StrokeCap.Round,
        )
    }

    // 手臂：先画，接缝交给键帽正面压住。
    for (dir in listOf(-1f, 1f)) {
        val inner = Offset((120f + dir * 62f) * u, 118f * u)
        val outer = Offset((120f + dir * 84f) * u, 128f * u)
        drawLine(primary, inner, outer, strokeWidth = 17f * u, cap = StrokeCap.Round)
        drawLine(plate, inner, outer, strokeWidth = 7f * u, cap = StrokeCap.Round)
    }

    // 键帽侧壁（上沿被正面盖住），刻 OPI。
    val skirtPath = Path().apply {
        moveTo(54f * u, 140f * u)
        lineTo(44f * u, 184f * u)
        quadraticTo(42f * u, 202f * u, 62f * u, 202f * u)
        lineTo(178f * u, 202f * u)
        quadraticTo(198f * u, 202f * u, 196f * u, 184f * u)
        lineTo(186f * u, 140f * u)
        close()
    }
    drawPath(skirtPath, skirt)
    drawPath(skirtPath, primary, style = Stroke(stroke, join = StrokeJoin.Round))
    if (legend) {
        val layout = measurer.measure(
            AnnotatedString("OPI"),
            TextStyle(
                color = shade,
                // u 已是「像素 / 设计单位」，toSp() 抵消 density×fontScale，
                // 使刻字最终恰好落在 21*u 像素上——宠物是图形，不该跟随系统字号缩放。
                fontSize = (21f * u).toSp(),
                fontWeight = FontWeight.Bold,
                fontFamily = FontFamily.Monospace,
                letterSpacing = (4.5f * u).toSp(),
            ),
        )
        val top = Offset(
            (120f * u - layout.size.width / 2f),
            (172f * u - layout.size.height / 2f),
        )
        drawText(layout, topLeft = top)
    }

    // 键帽正面。
    drawRoundRect(
        color = plate,
        topLeft = Offset(52f * u, 46f * u),
        size = Size(136f * u, 104f * u),
        cornerRadius = CornerRadius(20f * u),
    )
    drawRoundRect(
        color = primary,
        topLeft = Offset(52f * u, 46f * u),
        size = Size(136f * u, 104f * u),
        cornerRadius = CornerRadius(20f * u),
        style = Stroke(stroke),
    )

    // 眼睛：睡着时闭成一条线；困惑时左右不一样大。
    if (sleepy) {
        for (cx in listOf(93f, 147f)) {
            drawLine(
                shade,
                Offset((cx - 10f) * u, 96f * u),
                Offset((cx + 10f) * u, 96f * u),
                strokeWidth = stroke,
                cap = StrokeCap.Round,
            )
        }
    } else {
        val leftR = if (puzzled) 8f else if (waiting) 12f else 11f
        val rightR = if (puzzled) 12f else leftR
        drawCircle(shade, leftR * u, Offset(93f * u, 95f * u))
        drawCircle(shade, rightR * u, Offset(147f * u, 95f * u))
        if (!puzzled) {
            drawCircle(plate, 3.6f * u, Offset(89.2f * u, 91.2f * u))
            drawCircle(plate, 3.6f * u, Offset(143.2f * u, 91.2f * u))
        }
    }

    // 嘴：圆环 = 字母 O。困惑时缩小并歪向一边。
    val mouthRx = if (puzzled) 7f else 12f
    val mouthRy = if (puzzled) 6f else 10f
    val mouthCx = if (puzzled) 132f else 120f
    drawOval(
        color = shade,
        topLeft = Offset((mouthCx - mouthRx) * u, (128f - mouthRy) * u),
        size = Size(mouthRx * 2f * u, mouthRy * 2f * u),
        style = Stroke(stroke),
    )
}
