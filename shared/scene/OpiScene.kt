// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 键盘/状态栏的动态插画背景（2026-10-30 用户裁决：纯色底换成卡通动物、花草、
//! 人物的动态背景；桌面状态栏允许角色错落探出条外）。
//!
//! 设计约束：
//! * **没有图片资产、没有新依赖** —— 全部 Compose Canvas 自绘（与 `shared/pet`
//!   的小欧同一条纪律），Android 与桌面共用这一份几何；
//! * 颜色不写死：所有形状都从当前皮肤的 `base/ink/accent` 调出来（[ScenePalette]），
//!   深色皮肤自动压暗、浅色自动提亮，十二款皮肤都成立；
//! * 背景是**低对比**的：键面/按钮面永远画在它上面（[muted] 越大越淡），文字
//!   对比度由键帽本身保证，不靠背景让路；
//! * 动画只动相位，不触发重组：调用方在绘制阶段读 [rememberOpiScenePhase] 的
//!   `State.value`（同 `shared/pet/PetMotion.kt` 的做法）。
//!
//! 桌面端「错落溢出」的落点：条窗外的角色由 [drawOpiSceneHalo] 画在窗口的透明
//! 边距里，桌面侧按 [HALO_TOP_ANCHORS]/[HALO_BOTTOM_ANCHORS] 生成窗口裁剪形状
//! —— 两处锚点必须同源，否则角色会被裁掉半张脸。

package xyz.erik.opi.scene

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.runtime.Composable
import androidx.compose.runtime.State
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.min
import kotlin.math.sin

/**
 * 插画取色的三个角色（每个宿主从自己的皮肤色板映射）：
 * [base] = 键盘底/条底，[ink] = 主文字色（描边/眼睛），[accent] = 强调色。
 */
data class ScenePalette(
    val base: Color,
    val ink: Color,
    val accent: Color,
)

/** 一个完整动画循环（ms）。慢一点：这是背景，不是小游戏。 */
const val OPI_SCENE_PERIOD_MS = 16000

internal const val SCENE_TAU = 2f * PI.toFloat()

/** 缓慢的呼吸/摆幅：0..1 相位 → -1..1。 */
internal fun sceneSway(phase: Float, seed: Float): Float = sin((phase + seed) * SCENE_TAU)

/** 眨眼：每循环三次、每次约 0.3 秒；用尖峰函数而不是方波，边缘更自然。 */
internal fun sceneBlink(phase: Float, seed: Float): Boolean =
    sin((phase * 3f + seed) * SCENE_TAU) > 0.975f

/**
 * 条外角色的水平锚点（窗口宽度的比例）。**桌面窗口裁剪形状与 [drawOpiSceneHalo]
 * 必须共用这一份** —— 改这里就两边一起动。
 */
val HALO_TOP_ANCHORS = listOf(0.08f, 0.30f, 0.55f, 0.80f)
val HALO_BOTTOM_ANCHORS = listOf(0.18f, 0.48f, 0.76f)

/**
 * 相位 0..1 无限循环。`animate = false` 给渲染门禁/静态截图用（相位恒 0）。
 * 只在绘制阶段读 `.value`，不会每帧重组整棵 UI。
 */
@Composable
fun rememberOpiScenePhase(animate: Boolean = true): State<Float> {
    if (!animate) return remember { mutableStateOf(0f) }
    return rememberInfiniteTransition(label = "opiScene").animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(tween(OPI_SCENE_PERIOD_MS, easing = LinearEasing)),
        label = "opiScenePhase",
    )
}

/** 把固定色往当前皮肤的底色调，[muted] 越大越像底色（移动端背景用大值）。 */
internal fun sceneFill(color: Color, palette: ScenePalette, muted: Float): Color {
    val mixed = lerpColor(color, palette.base, muted.coerceIn(0f, 0.9f))
    // 深色底上再抬一点主文字色，免得整幅画糊成一团黑。
    return if (palette.base.luminance() < 0.3f) lerpColor(mixed, palette.ink, 0.06f) else mixed
}

/** 描边色 = 皮肤主文字色带透明度；深浅底自动反过来。 */
internal fun sceneEdge(palette: ScenePalette, alpha: Float): Color =
    palette.ink.copy(alpha = alpha.coerceIn(0f, 1f))

/**
 * 整幅背景（手机键盘用）的总入口：按 [theme] 分派到对应场景。
 * 坐标全按当前画布比例算 —— 横屏/竖屏/单手窄键区都不会画到框外。
 */
fun DrawScope.drawOpiSceneBackdrop(
    palette: ScenePalette,
    theme: SceneTheme,
    phase: Float,
    muted: Float = 0.24f,
) {
    when (theme) {
        SceneTheme.MEADOW -> drawMeadowBackdrop(palette, phase, muted)
        SceneTheme.FOREST -> drawForestBackdrop(palette, phase, muted)
        SceneTheme.GARDEN -> drawGardenBackdrop(palette, phase, muted)
        SceneTheme.NIGHT -> drawNightBackdrop(palette, phase, muted)
        SceneTheme.CITY -> drawCityBackdrop(palette, phase, muted)
        SceneTheme.MOUNTAIN -> drawMountainBackdrop(palette, phase, muted)
        SceneTheme.SEA -> drawSeaBackdrop(palette, phase, muted)
        SceneTheme.POND -> drawPondBackdrop(palette, phase, muted)
        SceneTheme.AUTUMN -> drawAutumnBackdrop(palette, phase, muted)
        SceneTheme.WINTER -> drawWinterBackdrop(palette, phase, muted)
        SceneTheme.MIST -> drawMistBackdrop(palette, phase, muted)
        SceneTheme.VEGETABLE -> drawVegetableBackdrop(palette, phase, muted)
        SceneTheme.SUNSET -> drawSunsetBackdrop(palette, phase, muted)
        SceneTheme.STUDY -> drawStudyBackdrop(palette, phase, muted)
        SceneTheme.KITCHEN -> drawKitchenBackdrop(palette, phase, muted)
        SceneTheme.COUNTRYSIDE -> drawCountrysideBackdrop(palette, phase, muted)
        SceneTheme.STREETLIGHT -> drawStreetlightBackdrop(palette, phase, muted)
        SceneTheme.DOGS -> drawDogsBackdrop(palette, phase, muted)
        SceneTheme.CATS -> drawCatsBackdrop(palette, phase, muted)
    }
}

/** 田园（改动前的原画）：天空、草坡、猫、兔子、小人、花草、蝴蝶、飘落的花瓣。 */
internal fun DrawScope.drawMeadowBackdrop(
    palette: ScenePalette,
    phase: Float,
    muted: Float = 0.24f,
) {
    val w = size.width
    val h = size.height
    if (w < 2f || h < 2f) return
    val u = min(w, h)
    val a = phase * 2f * PI.toFloat()

    // 天空底色：在皮肤底上极轻的冷暖过渡，保证任何皮肤都只泛一点色。
    drawRect(
        brush = Brush.verticalGradient(
            0f to sceneFill(SceneSky, palette, (muted * 0.55f).coerceAtLeast(0.28f)),
            0.58f to palette.base,
            1f to sceneFill(SceneGround, palette, (muted * 0.42f).coerceAtLeast(0.24f)),
        ),
        size = size,
    )
    drawSun(palette, Offset(w * 0.83f, h * 0.14f), u * 0.075f, phase, muted)
    // 远景树篱 + 草坡（先画，角色站在上面）。
    drawRect(
        sceneFill(LeafDark, palette, 0.45f),
        Offset(0f, h * 0.695f),
        Size(w, h * 0.02f),
    )
    for ((i, x) in listOf(0.06f, 0.22f, 0.40f, 0.60f, 0.78f, 0.94f).withIndex()) {
        drawShrub(palette, Offset(w * x, h * 0.705f), u * (0.10f + 0.02f * (i % 2)), phase + i * 0.2f, muted + 0.12f)
    }
    drawOval(
        color = sceneFill(LeafGreen, palette, 0.42f),
        topLeft = Offset(-w * 0.12f, h * 0.68f),
        size = Size(w * 0.78f, h * 0.52f),
    )
    drawOval(
        color = sceneFill(LeafDark, palette, 0.48f),
        topLeft = Offset(w * 0.42f, h * 0.74f),
        size = Size(w * 0.78f, h * 0.5f),
    )
    // 云（低对比，只做层次）。
    drawCloud(palette, Offset(w * 0.20f, h * 0.18f), u * 0.20f, phase, muted)
    drawCloud(palette, Offset(w * 0.74f, h * 0.12f), u * 0.16f, phase + 0.35f, muted)
    // 两角伸进来的花枝 + 两侧花藤（根在地面、梢在画面上沿）。
    drawFloweringBranch(
        palette, Offset(-w * 0.02f, h * 0.08f), Offset(w * 0.24f, h * 0.22f),
        u * 0.42f, phase, muted, BlossomPink, seed = 0.15f,
    )
    drawFloweringBranch(
        palette, Offset(w * 1.02f, h * 0.05f), Offset(w * 0.76f, h * 0.20f),
        u * 0.40f, phase + 0.4f, muted, BlossomWhite, seed = 0.65f,
    )
    drawClimbingVine(
        palette, Offset(w * 0.02f, h * 1.02f), Offset(w * 0.05f, h * 0.18f),
        u * 0.50f, phase, muted, BlossomPink, flowers = 3, seed = 0.1f,
    )
    drawClimbingVine(
        palette, Offset(w * 0.98f, h * 1.02f), Offset(w * 0.95f, h * 0.20f),
        u * 0.46f, phase + 0.45f, muted, BlossomWhite, flowers = 3, seed = 0.7f,
    )

    // 角色：底边花圃，左右两只动物，中间的小人举着气球。
    drawFlowerBed(
        palette, Offset(w * 0.18f, h * 0.94f), u * 0.26f, u * 0.16f,
        phase + 0.10f, muted, listOf(BlossomPink, BlossomYellow, BlossomWhite), count = 5,
    )
    drawFlowerBed(
        palette, Offset(w * 0.43f, h * 0.98f), u * 0.22f, u * 0.14f,
        phase + 0.55f, muted, listOf(BlossomYellow, BlossomRed, BlossomPink), count = 4,
    )
    drawFlowerBed(
        palette, Offset(w * 0.73f, h * 0.94f), u * 0.24f, u * 0.15f,
        phase + 0.30f, muted, listOf(BlossomWhite, BlossomPink, BlossomYellow), count = 5,
    )
    val personS = u * 0.24f
    val personC = Offset(w * 0.53f, h * 0.78f)
    drawContactShadow(palette, Offset(w * 0.13f, h * 0.80f + u * 0.20f), u * 0.30f, muted)
    drawContactShadow(palette, Offset(w * 0.87f, h * 0.80f + u * 0.15f), u * 0.26f, muted)
    drawContactShadow(palette, Offset(personC.x, personC.y + u * 0.15f), u * 0.28f, muted)
    drawCatFigure(palette, Offset(w * 0.13f, h * 0.80f), u * 0.28f, phase, muted, detail = 0.55f)
    drawRabbitFigure(palette, Offset(w * 0.87f, h * 0.80f), u * 0.22f, phase + 0.45f, muted, detail = 0.55f)
    drawPersonFigure(palette, personC, personS, phase, muted, detail = 0.55f)
    drawBalloon(
        palette, Offset(w * 0.63f, h * 0.30f), u * 0.085f, phase + 0.2f, muted,
        stringEnd = Offset(personC.x + personS * 0.23f, personC.y - personS * 0.28f),
    )

    // 前景：蝴蝶和飘落的花瓣（很淡，不挡字）。
    drawButterfly(
        palette,
        Offset(w * 0.35f + sin(a) * w * 0.018f, h * 0.28f + cos(a * 1.3f) * h * 0.02f),
        u * 0.05f,
        phase,
        muted,
    )
    drawBird(palette, Offset(w * 0.35f, h * 0.30f), u * 0.022f, phase, muted)
    drawBird(palette, Offset(w * 0.41f, h * 0.26f), u * 0.018f, phase + 0.3f, muted)
    drawPetals(palette, w, h, phase, muted)
}

/**
 * 条外角色（桌面状态栏用）的总入口：按 [theme] 分派。
 * [topXs]/[bottomXs] 是窗口像素坐标里的水平锚点 —— 桌面侧用同一份列表生成窗口
 * 裁剪形状（[HALO_TOP_ANCHORS]/[HALO_BOTTOM_ANCHORS] 只给桌面算默认值），两边
 * 必须传同一批数字，否则角色会被裁掉半张脸。
 * [halo] = 条窗四周的透明边距；[barTop]/[barBottom] = 条本体在窗口内的上下沿。
 */
fun DrawScope.drawOpiSceneHalo(
    palette: ScenePalette,
    theme: SceneTheme,
    phase: Float,
    halo: Float,
    barTop: Float,
    barBottom: Float,
    topXs: List<Float>,
    bottomXs: List<Float>,
    muted: Float = 0.08f,
) {
    if (halo < 1f) return
    when (theme) {
        SceneTheme.MEADOW -> drawMeadowHalo(palette, phase, halo, barTop, barBottom, topXs, bottomXs, muted)
        SceneTheme.FOREST -> drawForestHalo(palette, phase, halo, barTop, barBottom, topXs, bottomXs, muted)
        SceneTheme.GARDEN -> drawGardenHalo(palette, phase, halo, barTop, barBottom, topXs, bottomXs, muted)
        SceneTheme.NIGHT -> drawNightHalo(palette, phase, halo, barTop, barBottom, topXs, bottomXs, muted)
        SceneTheme.CITY -> drawCityHalo(palette, phase, halo, barTop, barBottom, topXs, bottomXs, muted)
        SceneTheme.MOUNTAIN -> drawMountainHalo(palette, phase, halo, barTop, barBottom, topXs, bottomXs, muted)
        SceneTheme.SEA -> drawSeaHalo(palette, phase, halo, barTop, barBottom, topXs, bottomXs, muted)
        SceneTheme.POND -> drawPondHalo(palette, phase, halo, barTop, barBottom, topXs, bottomXs, muted)
        SceneTheme.AUTUMN -> drawAutumnHalo(palette, phase, halo, barTop, barBottom, topXs, bottomXs, muted)
        SceneTheme.WINTER -> drawWinterHalo(palette, phase, halo, barTop, barBottom, topXs, bottomXs, muted)
        SceneTheme.MIST -> drawMistHalo(palette, phase, halo, barTop, barBottom, topXs, bottomXs, muted)
        SceneTheme.VEGETABLE -> drawVegetableHalo(palette, phase, halo, barTop, barBottom, topXs, bottomXs, muted)
        SceneTheme.SUNSET -> drawSunsetHalo(palette, phase, halo, barTop, barBottom, topXs, bottomXs, muted)
        SceneTheme.STUDY -> drawStudyHalo(palette, phase, halo, barTop, barBottom, topXs, bottomXs, muted)
        SceneTheme.KITCHEN -> drawKitchenHalo(palette, phase, halo, barTop, barBottom, topXs, bottomXs, muted)
        SceneTheme.COUNTRYSIDE -> drawCountrysideHalo(palette, phase, halo, barTop, barBottom, topXs, bottomXs, muted)
        SceneTheme.STREETLIGHT -> drawStreetlightHalo(palette, phase, halo, barTop, barBottom, topXs, bottomXs, muted)
        SceneTheme.DOGS -> drawDogsHalo(palette, phase, halo, barTop, barBottom, topXs, bottomXs, muted)
        SceneTheme.CATS -> drawCatsHalo(palette, phase, halo, barTop, barBottom, topXs, bottomXs, muted)
    }
}

/** 田园条外角色（改动前的原画）。 */
internal fun DrawScope.drawMeadowHalo(
    palette: ScenePalette,
    phase: Float,
    halo: Float,
    barTop: Float,
    barBottom: Float,
    topXs: List<Float>,
    bottomXs: List<Float>,
    muted: Float = 0.08f,
) {
    val u = halo

    drawCatFigure(
        palette,
        Offset(topXs[0], barTop - u * 0.30f),
        u * 0.72f,
        phase,
        muted,
        detail = 1f,
    )
    drawRabbitFigure(
        palette,
        Offset(topXs[1], barTop - u * 0.24f),
        u * 0.60f,
        phase + 0.3f,
        muted,
        detail = 1f,
    )
    drawPersonFigure(
        palette,
        Offset(topXs[2], barTop - u * 0.20f),
        u * 0.62f,
        phase,
        muted,
        detail = 1f,
    )
    drawBalloon(
        palette,
        Offset(topXs[3], barTop - u * 0.55f),
        u * 0.38f,
        phase + 0.2f,
        muted,
    )

    drawFlowerCluster(
        palette,
        Offset(bottomXs[0], barBottom + u * 0.42f),
        u * 0.42f,
        phase + 0.10f,
        muted,
        hanging = true,
    )
    drawFlowerCluster(
        palette,
        Offset(bottomXs[1], barBottom + u * 0.38f),
        u * 0.36f,
        phase + 0.55f,
        muted,
        hanging = true,
    )
    drawFlowerCluster(
        palette,
        Offset(bottomXs[2], barBottom + u * 0.44f),
        u * 0.40f,
        phase + 0.30f,
        muted,
        hanging = true,
    )
}

// 固定色只作「色相种子」，最终一律经 sceneFill 混进皮肤底色。
internal val SceneSky = Color(0xFFCDE8F2)
internal val SceneGround = Color(0xFFBFDCC2)

private fun lerpColor(a: Color, b: Color, t: Float): Color =
    Color(
        red = a.red + (b.red - a.red) * t,
        green = a.green + (b.green - a.green) * t,
        blue = a.blue + (b.blue - a.blue) * t,
        alpha = a.alpha + (b.alpha - a.alpha) * t,
    )

internal fun Color.luminance(): Float = 0.2126f * red + 0.7152f * green + 0.0722f * blue
