// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏的一枚按钮（2026-10-30 从 StatusBar.kt 拆出：那边加「错落溢出」窗口几何
//! 后要撞 500 行门禁；同包 internal 引用不变）。
//!
//! 2026-10-30 同时给普通按钮加了一层**浅键帽底**（用户裁决「按键有时候还显示不
//! 分明」）：条底现在可能压着插画/渐变，裸字形会糊进去；底板比条底略向主文字色
//! 偏一档，再描 1dp 分隔线，字形本身颜色/大小都不动。enpro/mode 本来就自带白底
//! 键帽，不再套第二层框。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.WindowPosition
import androidx.compose.ui.window.WindowState
import kotlinx.coroutines.delay

/**
 * 一枚按钮：[BarMetrics.button] 见方的框 + 浅键帽底 + 悬停/按下反馈 + 点击（LOGO 另挂
 * 拖动）。**面**（图标/字形）在 [StatusBarButtonFace]（BarGlyphs.kt）—— 分开是为了
 * 让门禁能离屏直接渲那一支。
 */
@Composable
internal fun BarButton(
    b: StatusBarButton,
    st: ImeState,
    mode: String,
    metrics: BarMetrics,
    windowState: WindowState,
    barWindow: java.awt.Window,
    onClick: (StatusBarButton) -> Unit,
) {
    val interaction = remember { MutableInteractionSource() }
    val hovered by interaction.collectIsHoveredAsState()
    val pressed by interaction.collectIsPressedAsState()
    val palette = LocalBarPalette.current
    val sceneActive = LocalBarSceneActive.current
    // 英文专业键常亮（2026-10-19）：否则切了 pro 看不出任何变化。用既有 hover 色，
    // 不新造颜色/角色（见 SkinPalette.kt 的角色映射）。键帽本身是固定白底（设计图），
    // 这一圈 hover 只露在键帽外缘 —— 激活态仍可见，且不染色键帽。
    val active = b == StatusBarButton.ENPRO && st.englishPro
    var tip by remember { mutableStateOf(false) }
    // 按钮在条窗内的像素矩形 —— 提示窗要靠它换算屏幕坐标。
    var anchor by remember { mutableStateOf(Rect.Zero) }
    // 悬停 400ms 出提示。提示必须是独立 Window：Compose 的 Popup 会被夹进 42dp 高的
    // 条窗（clippingEnabled=false 也救不了，xwininfo 实测无独立弹层），见 BarTooltip.kt。
    LaunchedEffect(hovered) { tip = false; if (hovered) { delay(400); tip = true } }

    val keyPlated = b != StatusBarButton.ENPRO && b != StatusBarButton.MODE
    val shape = RoundedCornerShape((metrics.button.value * 0.18f).dp)
    val plate = lerp(palette.bg, palette.fg, 0.07f)
        .copy(alpha = if (sceneActive) 0.62f else 1f)
    val plateBg = when {
        pressed -> lerp(plate, palette.accent, 0.32f)
        hovered || active -> lerp(plate, palette.fg, 0.09f)
        else -> plate
    }
    // enpro/mode 保持旧反馈（它们的白底键帽在面上，不套第二层框）。
    val bareBg = when {
        pressed -> palette.accent.copy(alpha = 0.30f)
        hovered || active -> palette.hover
        else -> Color.Transparent
    }
    val box = Modifier.size(metrics.button).then(
        if (keyPlated) {
            Modifier.clip(shape)
                .background(plateBg)
                .border(1.dp, palette.line.copy(alpha = 0.72f), shape)
        } else {
            Modifier.background(bareBg)
        },
    )
    Box(
        modifier = box
            .clickable(interactionSource = interaction, indication = null) { onClick(b) }
            .then(if (b == StatusBarButton.LOGO) dragToMove(windowState) else Modifier)
            .onGloballyPositioned { anchor = it.boundsInWindow() }
            .semantics { contentDescription = b.label },
        contentAlignment = Alignment.Center,
    ) {
        StatusBarButtonFace(b, st, mode, metrics.button)
        if (tip) BarTooltip(b, anchor, windowState.position, palette, barWindow)
    }
}

/**
 * 把窗口拖动交给这个修饰符（只有 logo 挂它）。
 *
 * 指针增量单位是**像素**，`WindowPosition` 要 dp ⇒ 经 LocalDensity 换算；不换算的话
 * 高 DPI 下拖起来会「跑」（指针走 1dp，窗口走 2dp）。key 用 density：换 DPI 后重建，
 * 免得一直用旧的换算比。
 */
@Composable
private fun dragToMove(windowState: WindowState): Modifier {
    val density = LocalDensity.current
    return Modifier.pointerInput(density) {
        detectDragGestures { change, drag ->
            change.consume()
            val cur = windowState.position
            windowState.position = WindowPosition(
                cur.x + with(density) { drag.x.toDp() },
                cur.y + with(density) { drag.y.toDp() },
            )
        }
    }
}
