// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import androidx.compose.animation.Crossfade
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import xyz.erik.opi.keyboard.LocalKeyboardPalette
import xyz.erik.opi.pet.OpiPet
import xyz.erik.opi.pet.PetMood
import xyz.erik.opi.pet.PetPalette
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.sin

/**
 * 顶部**宠物条**（2026-10-17 用户裁决：「默认页最上面的动态花纹不好看，移除，对应的
 * 功能也移除，改成项目宠物的多形态：笑、欢乐、哭、沮丧、惊恐、愤怒，要是动态的、
 * 有色彩感」）。
 *
 * 位置 = 拼音行那条 38dp 槽位**空闲时**的内容（输入中仍是拼音行 —— 共用槽位不变）。
 * 十二形态按用户点名的次序轮播（[PET_STRIP_FORMS]），每个停留 [PET_FORM_HOLD_MS] 后
 * 淡入下一形态；小欧本体另有随情绪变化的动效（笑轻晃 / 欢乐蹦跳 / 哭发抖 / 沮丧
 * 低头摆 / 惊恐急颤 / 愤怒横抖 / 撒娇轻摇 / 卖萌弹跳 / 讨好点头 / 敲打快敲 /
 * 捂脸摇头 / 吐舌晃动），条底洗色与粒子跟着当前形态换色。
 *
 * 纯装饰：读屏跳过（`clearAndSetSemantics`）。条底走键盘皮肤（`barBg`），情绪色只做
 * 低透明叠加 —— 深色皮肤下不会变成一块亮斑。
 */
@Composable
fun PetStrip(modifier: Modifier = Modifier) {
    val p = LocalKeyboardPalette.current
    var current by remember { mutableStateOf(PET_STRIP_FORMS.first()) }
    var step by remember { mutableIntStateOf(0) }
    LaunchedEffect(Unit) {
        while (true) {
            delay(PET_FORM_HOLD_MS)
            step += 1
            current = petStripFormAt(step)
        }
    }
    val look = lookOf(current)
    val wash by animateColorAsState(
        targetValue = look.wash,
        animationSpec = tween(PET_FORM_FADE_MS),
        label = "petWash",
    )
    val accent by animateColorAsState(
        targetValue = look.accent,
        animationSpec = tween(PET_FORM_FADE_MS),
        label = "petAccent",
    )
    val dark = p.barBg.luminance() < 0.5f
    val petPalette = if (dark) PetPalette.Dark else PetPalette.Default
    // 相位 0..1 无限循环：各形态按不同倍频取用（见下面的 graphicsLayer），
    // 只在绘制层读它 —— 不触发重组。
    val transition = rememberInfiniteTransition(label = "petStrip")
    val phase by transition.animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(tween(PET_PHASE_MS, easing = LinearEasing)),
        label = "petPhase",
    )

    Box(
        modifier = modifier
            .fillMaxWidth()
            .height(COMPOSING_BAR_HEIGHT_DP.dp)
            .background(p.barBg)
            // 光晕/粒子的半径大于槽高：裁在条内，别漏到下一行的候选/工具条上。
            .clipToBounds()
            .clearAndSetSemantics {},
        contentAlignment = Alignment.Center,
    ) {
        // 条底：横向情绪洗色（两端淡、中间浓）+ 小欧身后一束同色光晕 + 漂浮粒子。
        Canvas(Modifier.fillMaxSize()) {
            drawRect(
                Brush.horizontalGradient(
                    listOf(
                        wash.copy(alpha = 0.04f),
                        wash.copy(alpha = 0.18f),
                        wash.copy(alpha = 0.04f),
                    ),
                ),
            )
            val glow = 30.dp.toPx()
            drawCircle(
                brush = Brush.radialGradient(
                    listOf(accent.copy(alpha = 0.16f), Color.Transparent),
                    center = Offset(size.width / 2f, size.height * 0.62f),
                    radius = glow,
                ),
                radius = glow,
                center = Offset(size.width / 2f, size.height * 0.62f),
            )
            drawPetParticles(current, phase, accent)
        }
        // 小欧本体：整只做情绪动效，脸用 Crossfade 换（两帧叠化，不闪断）。
        Box(
            modifier = Modifier.graphicsLayer {
                val a = phase * 2f * PI.toFloat()
                when (current) {
                    PetMood.IDLE -> {
                        translationY = (sin(a) * 1.1f).dp.toPx()
                        rotationZ = sin(a * 0.5f) * 1.1f
                    }
                    PetMood.WAITING -> {
                        val hop = abs(sin(a))
                        translationY = (-hop * 2.4f).dp.toPx()
                        scaleX = 1f + hop * 0.03f
                        scaleY = 1f - hop * 0.03f
                    }
                    PetMood.CRYING -> {
                        translationX = (sin(a * 10f) * 0.5f).dp.toPx()
                        rotationZ = sin(a * 5f) * 1.4f
                        translationY = 1.dp.toPx()
                    }
                    PetMood.PUZZLED -> {
                        translationY = (1f + sin(a) * 0.7f).dp.toPx()
                        rotationZ = sin(a * 0.5f) * 2.2f
                    }
                    PetMood.TERRIFIED -> {
                        translationX = (sin(a * 16f) * 1.1f).dp.toPx()
                        rotationZ = sin(a * 11f) * 1.1f
                    }
                    PetMood.ANGRY -> {
                        translationX = (sin(a * 18f) * 1.9f).dp.toPx()
                        rotationZ = sin(a * 15f) * 0.9f
                    }
                    PetMood.COQUETTISH -> {
                        translationX = (sin(a * 0.7f) * 0.8f).dp.toPx()
                        rotationZ = sin(a * 0.7f) * 1.8f
                    }
                    PetMood.CUTE -> {
                        val hop = abs(sin(a))
                        translationY = (-hop * 1.8f).dp.toPx()
                        rotationZ = sin(a) * 2f
                    }
                    PetMood.PLEASING -> {
                        // 点头：只向下沉，不左右晃。
                        translationY = (abs(sin(a)) * 1.2f).dp.toPx()
                    }
                    PetMood.KNOCKING -> {
                        // 敲打：高频小顿挫。
                        translationY = (-abs(sin(a * 14f)) * 1.4f).dp.toPx()
                        rotationZ = sin(a * 14f) * 0.6f
                    }
                    PetMood.FACE_PALM -> {
                        translationX = (sin(a * 6f) * 0.9f).dp.toPx()
                        rotationZ = sin(a * 6f) * 1.2f
                    }
                    PetMood.TONGUE_OUT -> {
                        translationY = (sin(a * 2.2f) * 0.6f).dp.toPx()
                        rotationZ = sin(a * 2.2f) * 1.6f
                    }
                    else -> Unit
                }
            },
            contentAlignment = Alignment.Center,
        ) {
            Crossfade(
                targetState = current,
                animationSpec = tween(PET_FORM_FADE_MS),
                label = "petForm",
            ) { mood ->
                OpiPet(mood = mood, palette = petPalette, size = PET_SIZE)
            }
        }
    }
}

/** 每个形态的停留时长（ms）与淡入时长（ms）；十二形态共用同一节奏。 */
internal const val PET_FORM_HOLD_MS = 2800L
internal const val PET_FORM_FADE_MS = 420

/** 动效相位周期（ms）：相位 0..1 循环，各形态按不同倍频取用。 */
private const val PET_PHASE_MS = 2400

/** 小欧在条里的宽度（高按 240:250 推导 ≈ 33.3dp，上下各留 ~2dp 给动效）。 */
private val PET_SIZE = 32.dp

/**
 * 顶部宠物条十二形态轮播次序 —— **用户点名的次序**（2026-10-17 前六枚 + 2026-10-18
 * 后六枚）：笑、欢乐、哭、沮丧、惊恐、愤怒、撒娇、卖萌、讨好、敲打、捂脸、吐舌头。
 * 前三枚与 [xyz.erik.opi.pet.petMood] 的状态脸同源（笑 = 待命、欢乐 = 等选、沮丧 =
 * 无匹配），其余是专为宠物条追加的表情（见 PetMood 的枚举注释）。
 */
internal val PET_STRIP_FORMS: List<PetMood> = listOf(
    PetMood.IDLE, // 笑
    PetMood.WAITING, // 欢乐
    PetMood.CRYING, // 哭
    PetMood.PUZZLED, // 沮丧
    PetMood.TERRIFIED, // 惊恐
    PetMood.ANGRY, // 愤怒
    PetMood.COQUETTISH, // 撒娇
    PetMood.CUTE, // 卖萌
    PetMood.PLEASING, // 讨好
    PetMood.KNOCKING, // 敲打
    PetMood.FACE_PALM, // 捂脸
    PetMood.TONGUE_OUT, // 吐舌头
)

/** 轮播下标 → 形态；任意整数都安全落回同一圈（负数取模修正）。 */
internal fun petStripFormAt(step: Int): PetMood {
    val n = PET_STRIP_FORMS.size
    return PET_STRIP_FORMS[((step % n) + n) % n]
}

/** 每个形态的条底洗色与点缀色（情绪色只做低透明叠加，跟着皮肤走）。 */
private data class FormLook(val wash: Color, val accent: Color)

private fun lookOf(mood: PetMood): FormLook = when (mood) {
    PetMood.IDLE -> FormLook(Color(0xFFFFE7A8), Color(0xFFF0B34E)) // 金
    PetMood.WAITING -> FormLook(Color(0xFFFFC9D6), Color(0xFFEE7C9F)) // 桃粉
    PetMood.CRYING -> FormLook(Color(0xFFCBE4FB), Color(0xFF5C9FDF)) // 泪蓝
    PetMood.PUZZLED -> FormLook(Color(0xFFD7DDE9), Color(0xFF8797B4)) // 霾灰
    PetMood.TERRIFIED -> FormLook(Color(0xFFE0D0F7), Color(0xFF8A6DC8)) // 惊紫
    PetMood.ANGRY -> FormLook(Color(0xFFFFC6AE), Color(0xFFDD5334)) // 怒红
    PetMood.COQUETTISH -> FormLook(Color(0xFFFFC9DC), Color(0xFFF2638F)) // 玫瑰
    PetMood.CUTE -> FormLook(Color(0xFFFFE9B8), Color(0xFFF2B84B)) // 暖金
    PetMood.PLEASING -> FormLook(Color(0xFFC9F0E2), Color(0xFF5FBF9F)) // 薄荷
    PetMood.KNOCKING -> FormLook(Color(0xFFFFE0B8), Color(0xFFE8912F)) // 琥珀
    PetMood.FACE_PALM -> FormLook(Color(0xFFFFD3C6), Color(0xFFE06A4E)) // 赭红
    PetMood.TONGUE_OUT -> FormLook(Color(0xFFFFC9E0), Color(0xFFEF5D8F)) // 桃粉
    else -> FormLook(Color(0xFFFFE7A8), Color(0xFFF0B34E))
}

/**
 * 情绪粒子：横贯条底的细碎光点（幅度小、alpha 低，不抢注意力）。位置由循环相位
 * 推导（无随机数，跨帧稳定）；哭与沮丧向下落，其余向上飘。
 */
private fun DrawScope.drawPetParticles(mood: PetMood, phase: Float, accent: Color) {
    val tau = 2f * PI.toFloat()
    val falling = mood == PetMood.CRYING || mood == PetMood.PUZZLED
    for (i in 0 until 8) {
        val seed = (i * 0.618f) % 1f
        val t = (phase + seed) % 1f
        val sway = when (mood) {
            PetMood.WAITING -> sin(tau * phase + i * 1.7f) * 3f
            PetMood.TERRIFIED -> sin(tau * phase * 9f + i) * 1.8f
            PetMood.ANGRY -> sin(tau * phase * 11f + i) * 1.2f
            PetMood.CUTE -> sin(tau * phase * 2f + i * 1.7f) * 2.5f
            PetMood.COQUETTISH -> sin(tau * phase * 0.8f + i) * 2f
            PetMood.KNOCKING -> sin(tau * phase * 13f + i) * 1.4f
            else -> 0f
        }
        val x = size.width * (0.05f + seed * 0.9f) + sway.dp.toPx()
        val y = (if (falling) t else 1f - t) * size.height
        val r = (1.1f + (i % 3) * 0.5f).dp.toPx()
        val alpha = 0.16f + 0.40f * sin(t * PI.toFloat())
        drawCircle(accent.copy(alpha = alpha), radius = r, center = Offset(x, y))
    }
}
