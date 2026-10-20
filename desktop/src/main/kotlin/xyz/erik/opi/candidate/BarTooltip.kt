// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 按钮的悬停提示窗（2026-10-20）：必须用**独立 Window**。
//! Compose 的 Popup 会被夹在 42dp 高的状态栏窗内（`PopupProperties(clippingEnabled = false)`
//! 也救不了：xwininfo 实测没有独立弹层，只剩条顶一条白边），所以这里按
//! 「条窗屏幕位置 + 按钮在窗内位置」换算坐标，在按钮正上方开一个不可聚焦的小窗。
//! 文案仍来自 [tooltipText]（唯一源 = [StatusBarButton] 的 label/hint）。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.WindowPosition
import androidx.compose.ui.window.rememberWindowState
import java.awt.event.ComponentAdapter
import java.awt.event.ComponentEvent

/**
 * 在 [anchor]（按钮在条窗内的像素矩形）正上方开提示窗；[barPosition] 是条窗的屏幕位置（dp）。
 * 窗宽高按文字实测（最多 320dp 换行），不裁字、不盖住光标；focusable=false 不抢焦点。
 */
@Composable
internal fun BarTooltip(
    b: StatusBarButton, anchor: Rect, barPosition: WindowPosition, palette: SkinColors, barWindow: java.awt.Window,
) {
    val text = tooltipText(b)
    val density = LocalDensity.current
    val measurer = rememberTextMeasurer()
    val measured = remember(text, density) {
        measurer.measure(
            text, TextStyle(fontSize = 12.sp),
            constraints = Constraints(maxWidth = with(density) { 320.dp.roundToPx() }),
        )
    }
    // +1dp 宽度余量：px→dp 取整后不因 1px 之差多折一行。
    val width = with(density) { measured.size.width.toDp() } + 17.dp
    val height = with(density) { measured.size.height.toDp() } + 8.dp
    Window(
        onCloseRequest = {},
        state = rememberWindowState(
            width = width,
            height = height,
            position = WindowPosition(
                barPosition.x + with(density) { anchor.center.x.toDp() } - width / 2,
                barPosition.y + with(density) { anchor.top.toDp() } - height,
            ),
        ),
        undecorated = true,
        alwaysOnTop = true,
        resizable = false,
        focusable = false,
        title = "小欧输入法 提示",
    ) {
        // 条窗一藏，Compose 就不再重组它的内容（悬停也收不到 Exit）⇒ 提示会变成孤儿浮窗。
        // 直接听条窗的 AWT 隐藏事件，把自己一起收掉。
        DisposableEffect(barWindow) {
            val hide = object : ComponentAdapter() {
                override fun componentHidden(e: ComponentEvent) { window.isVisible = false }
            }
            barWindow.addComponentListener(hide)
            onDispose { barWindow.removeComponentListener(hide) }
        }
        // 与条同一套摘除任务栏的逻辑（见 SkipTaskbar）；标题唯一，BFS 不会认错窗。
        LaunchedEffect(window) { SkipTaskbar.apply(window) }
        Box(
            Modifier.fillMaxSize().background(palette.bg).border(1.dp, palette.line)
                .padding(horizontal = 8.dp, vertical = 4.dp),
        ) {
            Text(text, color = palette.fg, fontSize = 12.sp)
        }
    }
}
