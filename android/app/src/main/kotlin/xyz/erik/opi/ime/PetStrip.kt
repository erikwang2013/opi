// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import androidx.compose.animation.Crossfade
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
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
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import xyz.erik.opi.keyboard.LocalKeyboardPalette
import xyz.erik.opi.pet.OpiPet
import xyz.erik.opi.pet.PetMood
import xyz.erik.opi.pet.PetPalette
import xyz.erik.opi.pet.petMotion
import xyz.erik.opi.pet.rememberPetMotionPhase
import kotlin.math.PI
import kotlin.math.sin
import kotlin.random.Random

/**
 * 顶部**宠物条**（2026-10-17 用户裁决：「默认页最上面的动态花纹不好看，移除，对应的
 * 功能也移除，改成项目宠物的多形态：笑、欢乐、哭、沮丧、惊恐、愤怒，要是动态的、
 * 有色彩感」）。
 *
 * 位置 = 拼音行那条 38dp 槽位**空闲时**的内容（输入中仍是拼音行 —— 共用槽位不变）。
 * 2026-10-20 用户裁决：不再单形态轮播，改成**每次进入/每轮换随机同时显示 1..12 个
 * 互不重复的形态**（[petStripSelection]），数量多时按条宽整体缩小、不重叠不裁切；
 * 小欧本体仍各有随情绪变化的动效（见 shared/pet/PetMotion.kt），条底洗色与粒子
 * 跟当前这组的第一枚走。
 *
 * 纯装饰：读屏跳过（`clearAndSetSemantics`）。条底走键盘皮肤（`barBg`），情绪色只做
 * 低透明叠加 —— 深色皮肤下不会变成一块亮斑。
 */
@Composable
fun PetStrip(modifier: Modifier = Modifier) {
    val p = LocalKeyboardPalette.current
    var forms by remember { mutableStateOf(petStripSelection(Random.Default)) }
    LaunchedEffect(Unit) {
        while (true) {
            delay(PET_FORM_HOLD_MS)
            forms = petStripSelection(Random.Default)
        }
    }
    val lead = forms.first()
    val look = lookOf(lead)
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
    // 相位 0..1 无限循环：各形态按不同倍频取用（见 shared/pet/PetMotion.kt），
    // 只在绘制层读它 —— 不触发重组。
    val phase = rememberPetMotionPhase()

    BoxWithConstraints(
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
            drawPetParticles(lead, phase.value, accent)
        }
        // 一枚形态占一格：按条宽均分算尺寸，12 枚时自动缩小（最窄机型也 ≥ ~24dp）。
        val gap = if (forms.size > 8) 2.dp else 4.dp
        val petSize = minOf(
            PET_SIZE,
            (maxWidth - PET_ROW_PADDING * 2 - gap * (forms.size - 1)) / forms.size,
        ).coerceAtLeast(1.dp)
        // 整组一起淡入淡出（每个形态自己的动效由 petMotion 承担），不闪断。
        Crossfade(
            targetState = forms,
            animationSpec = tween(PET_FORM_FADE_MS),
            label = "petForms",
        ) { moods ->
            Row(
                modifier = Modifier.fillMaxWidth().padding(horizontal = PET_ROW_PADDING),
                horizontalArrangement = Arrangement.spacedBy(gap, Alignment.CenterHorizontally),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                for (mood in moods) {
                    Box(
                        modifier = Modifier.petMotion(mood, phase),
                        contentAlignment = Alignment.Center,
                    ) {
                        OpiPet(mood = mood, palette = petPalette, size = petSize)
                    }
                }
            }
        }
    }
}

/** 每组形态的停留时长（ms）与淡入时长（ms）；随机重抽共用同一节奏。 */
internal const val PET_FORM_HOLD_MS = 2800L
internal const val PET_FORM_FADE_MS = 420

/** 小欧在条里的**最大**宽度（高按 240:250 推导 ≈ 33.3dp，上下各留 ~2dp 给动效）；
 *  多枚并排时按条宽均分缩小。 */
private val PET_SIZE = 32.dp

/** 多枚并排时的左右内边距：与 [PET_SIZE] 一起保证 12 枚时不出条、不重叠。 */
private val PET_ROW_PADDING = 4.dp

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

/**
 * 宠物条的随机形态集合（2026-10-20 用户裁决「随机同时显示多个不同形态（不能重复），
 * 数量从 1 到全部占满」）：随机抽 1..[capacity] 枚**互不重复**的形态，顺序也随机。
 *
 * [capacity] 是数量上限（默认 = 全部 12 形态）；越界钳到 1..12，纯函数不抛。
 * 随机源由调用方注入 —— 生产用 `Random.Default`，单测用固定种子。
 */
internal fun petStripSelection(
    random: Random,
    capacity: Int = PET_STRIP_FORMS.size,
): List<PetMood> {
    val cap = capacity.coerceIn(1, PET_STRIP_FORMS.size)
    return PET_STRIP_FORMS.shuffled(random).take(random.nextInt(1, cap + 1))
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
