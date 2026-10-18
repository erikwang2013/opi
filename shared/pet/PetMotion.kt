// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.pet

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.unit.dp
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.sin

/**
 * 小欧的**情绪动效**（Android 顶部宠物条 / 桌面设置页 / 表情贴纸格共用）。
 *
 * 2026-10-19 从 `android/.../PetStrip.kt` 抽出：同一只小欧在 Android 与桌面都按
 * 同一套相位公式摆动。纯函数 [petMotionSpec] 可在 JVM 单测里断言，UI 只负责把
 * 它落到 [graphicsLayer]。
 *
 * 相位 0..1 无限循环；调用方用 [rememberPetMotionPhase] 拿 `State`，只在
 * [petMotion] 的 graphicsLayer 块里读 `.value`（读在绘制阶段，不触发重组）。
 */
@Composable
fun rememberPetMotionPhase(periodMs: Int = PET_MOTION_PERIOD_MS): State<Float> =
    rememberInfiniteTransition(label = "opiPetMotion").animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(tween(periodMs, easing = LinearEasing)),
        label = "opiPetPhase",
    )

/** 动效周期（ms）：相位 0..1 循环，各形态按不同倍频取用。 */
const val PET_MOTION_PERIOD_MS = 2400

/** 一帧的变换量（单位：dp / 倍率 / 度）。默认值 = 不动。 */
data class PetMotionSpec(
    val translationXDp: Float = 0f,
    val translationYDp: Float = 0f,
    val scaleX: Float = 1f,
    val scaleY: Float = 1f,
    val rotationZ: Float = 0f,
)

/**
 * 某形态在相位 [phase]（0..1）的变换。公式与 2026-10-17 宠物条逐条一致；
 * `else`（SLEEPY/DEGRADED 这两个输入状态）保持不动。
 */
fun petMotionSpec(mood: PetMood, phase: Float): PetMotionSpec {
    val a = phase * 2f * PI.toFloat()
    return when (mood) {
        PetMood.IDLE -> PetMotionSpec(
            translationYDp = sin(a) * 1.1f,
            rotationZ = sin(a * 0.5f) * 1.1f,
        )
        PetMood.WAITING -> {
            val hop = abs(sin(a))
            PetMotionSpec(
                translationYDp = -hop * 2.4f,
                scaleX = 1f + hop * 0.03f,
                scaleY = 1f - hop * 0.03f,
            )
        }
        PetMood.CRYING -> PetMotionSpec(
            translationXDp = sin(a * 10f) * 0.5f,
            translationYDp = 1f,
            rotationZ = sin(a * 5f) * 1.4f,
        )
        PetMood.PUZZLED -> PetMotionSpec(
            translationYDp = 1f + sin(a) * 0.7f,
            rotationZ = sin(a * 0.5f) * 2.2f,
        )
        PetMood.TERRIFIED -> PetMotionSpec(
            translationXDp = sin(a * 16f) * 1.1f,
            rotationZ = sin(a * 11f) * 1.1f,
        )
        PetMood.ANGRY -> PetMotionSpec(
            translationXDp = sin(a * 18f) * 1.9f,
            rotationZ = sin(a * 15f) * 0.9f,
        )
        PetMood.COQUETTISH -> PetMotionSpec(
            translationXDp = sin(a * 0.7f) * 0.8f,
            rotationZ = sin(a * 0.7f) * 1.8f,
        )
        PetMood.CUTE -> {
            val hop = abs(sin(a))
            PetMotionSpec(
                translationYDp = -hop * 1.8f,
                rotationZ = sin(a) * 2f,
            )
        }
        PetMood.PLEASING -> PetMotionSpec(translationYDp = abs(sin(a)) * 1.2f)
        PetMood.KNOCKING -> PetMotionSpec(
            translationYDp = -abs(sin(a * 14f)) * 1.4f,
            rotationZ = sin(a * 14f) * 0.6f,
        )
        PetMood.FACE_PALM -> PetMotionSpec(
            translationXDp = sin(a * 6f) * 0.9f,
            rotationZ = sin(a * 6f) * 1.2f,
        )
        PetMood.TONGUE_OUT -> PetMotionSpec(
            translationYDp = sin(a * 2.2f) * 0.6f,
            rotationZ = sin(a * 2.2f) * 1.6f,
        )
        else -> PetMotionSpec()
    }
}

/** 把 [petMotionSpec] 落到图形层（只在绘制阶段读相位，不触发重组）。 */
fun Modifier.petMotion(mood: PetMood, phase: State<Float>): Modifier = graphicsLayer {
    val spec = petMotionSpec(mood, phase.value)
    translationX = spec.translationXDp.dp.toPx()
    translationY = spec.translationYDp.dp.toPx()
    scaleX = spec.scaleX
    scaleY = spec.scaleY
    rotationZ = spec.rotationZ
}
