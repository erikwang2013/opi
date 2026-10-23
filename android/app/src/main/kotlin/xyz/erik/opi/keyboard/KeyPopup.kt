// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntRect
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupPositionProvider
import androidx.compose.ui.window.PopupProperties

/** 长按/上滑弹层里的一个备选键面；列表**首项画在最上方**。 */
data class KeyAlternative(
    val label: String,
    val value: String,
    /** null = 用发起键的字号。 */
    val fontSize: TextUnit? = null,
)

/** 弹层行高：手势按同一常数把纵向拖动换算成「移了几行」。 */
internal val KEY_POPUP_ROW_HEIGHT: Dp = 40.dp

/** 定宽：Popup 内容在窗口约束下测量，fillMaxWidth 会撑满整个键盘窗，故不用自适应宽。 */
private val KEY_POPUP_WIDTH: Dp = 64.dp

/**
 * 备选键面弹层（2026-10-24 参考图 aa/cc）：画在键上方，选中行高亮。
 * 它只负责画 —— 手指的按下/拖动/抬起仍由 [KeyButton] 自己的 pointerInput 收，
 * 所以 Popup 必须 `focusable = false`（抢焦点会中断原手势）。
 */
@Composable
internal fun KeyAlternativesPopup(
    alternatives: List<KeyAlternative>,
    selectedIndex: Int,
    fontSize: TextUnit,
    /** 键盘窗口尺寸（px）：弹层夹在它里面，见 [KeyPopupPositionProvider]。 */
    windowSize: IntSize,
) {
    val palette = LocalKeyboardPalette.current
    val positionProvider = remember(windowSize) { KeyPopupPositionProvider(windowSize) }
    Popup(
        popupPositionProvider = positionProvider,
        properties = PopupProperties(focusable = false),
    ) {
        Column(
            modifier = Modifier
                .width(KEY_POPUP_WIDTH)
                .clip(RoundedCornerShape(8.dp))
                .background(palette.keyBg)
                .padding(vertical = 4.dp),
        ) {
            alternatives.forEachIndexed { index, alternative ->
                val selected = index == selectedIndex
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(KEY_POPUP_ROW_HEIGHT)
                        .background(if (selected) palette.keyHighlightBg else Color.Transparent),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        alternative.label,
                        fontSize = alternative.fontSize ?: fontSize,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        color = if (selected) palette.keyHighlightFg else palette.keyFg,
                    )
                }
            }
        }
    }
}

/**
 * 弹层定位：优先键上方居中；上方放不下（首行键、英文数字行之下）翻到键下方。
 * 横纵都夹在键盘窗口内 —— 弹层是独立 Popup 窗口，不夹住就会盖到键盘外。
 *
 * ⚠️ [keyboardWindowSize] 必须是**键盘窗口**尺寸（px）：Compose 传给 provider 的 windowSize
 * 是整个可见屏（`getWindowVisibleDisplayFrame`），在 IME 里比键盘窗高得多，
 * 拿它夹会把弹层放到键盘外。键位坐标本身是窗口坐标，见 KeyButton 传入的 LocalView 尺寸。
 */
internal class KeyPopupPositionProvider(private val keyboardWindowSize: IntSize) : PopupPositionProvider {
    private companion object {
        const val MARGIN_PX = 8
        const val GAP_PX = 4
    }

    override fun calculatePosition(
        anchorBounds: IntRect,
        windowSize: IntSize,
        layoutDirection: LayoutDirection,
        popupContentSize: IntSize,
    ): IntOffset {
        // 宿主尺寸量不到时退回 Compose 给的窗口尺寸（至少保证在屏内）。
        val bounds = if (keyboardWindowSize.width > 0 && keyboardWindowSize.height > 0) {
            keyboardWindowSize
        } else {
            windowSize
        }
        val above = anchorBounds.top - GAP_PX - popupContentSize.height
        val y = if (above >= MARGIN_PX) above else anchorBounds.bottom + GAP_PX
        val x = anchorBounds.center.x - popupContentSize.width / 2
        return IntOffset(
            x.coerceIn(
                MARGIN_PX,
                (bounds.width - popupContentSize.width - MARGIN_PX).coerceAtLeast(MARGIN_PX),
            ),
            y.coerceIn(
                MARGIN_PX,
                (bounds.height - popupContentSize.height - MARGIN_PX).coerceAtLeast(MARGIN_PX),
            ),
        )
    }
}
