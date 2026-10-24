// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
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
import androidx.compose.ui.platform.LocalDensity
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

/** 长按/上滑弹层里的一个备选键面；列表**首项画在最左**。 */
data class KeyAlternative(
    val label: String,
    val value: String,
    /** null = 用发起键的字号。 */
    val fontSize: TextUnit? = null,
    /** true = 直接原样上屏（大写/数字/符号）；false = 进引擎（小写字母/9 键数字键）。 */
    val direct: Boolean = false,
)

/** 弹层每格宽/高：手势按同一常数把**横向**拖动换算成「移了几格」。 */
internal val KEY_POPUP_ITEM_WIDTH: Dp = 36.dp
private val KEY_POPUP_ITEM_HEIGHT: Dp = 44.dp

/** 横向拖动 → 选中项（首项在最左；向右拖选更靠后的项）。纯函数，JVM 单测钉住。 */
internal fun popupSelectionForDrag(
    start: Int,
    dragXPx: Float,
    itemWidthPx: Float,
    count: Int,
): Int {
    if (count <= 0) return 0
    if (itemWidthPx <= 0f) return start.coerceIn(0, count - 1)
    val steps = kotlin.math.round(dragXPx / itemWidthPx).toInt()
    return (start + steps).coerceIn(0, count - 1)
}

/** 默认格中心对准发起键时，弹层左边相对键中心的偏移（px）。 */
internal fun popupAnchorOffset(
    paddingPx: Float,
    defaultIndex: Int,
    itemWidthPx: Float,
): Int = (paddingPx + (defaultIndex + 0.5f) * itemWidthPx).toInt()

/**
 * 备选键面弹层（2026-10-24，2026-10-25 订正为**横向一行**）：画在键上方，
 * 条目从左到右排列，选中项高亮。它只负责画 —— 手指的按下/拖动/抬起仍由
 * [KeyButton] 自己的 pointerInput 收，所以 Popup 必须 `focusable = false`
 * （抢焦点会中断原手势）。
 */
@Composable
internal fun KeyAlternativesPopup(
    alternatives: List<KeyAlternative>,
    selectedIndex: Int,
    /** 打开时的默认项：决定弹层锚点（默认格正对发起键）；拖动改的是
     *  [selectedIndex]，弹层**不能**跟着移动。 */
    anchorIndex: Int,
    fontSize: TextUnit,
    /** 键盘窗口尺寸（px）：弹层夹在它里面，见 [KeyPopupPositionProvider]。 */
    windowSize: IntSize,
) {
    val palette = LocalKeyboardPalette.current
    val itemWidthPx = with(LocalDensity.current) { KEY_POPUP_ITEM_WIDTH.toPx() }
    val positionProvider = remember(windowSize, anchorIndex, alternatives.size, itemWidthPx) {
        KeyPopupPositionProvider(
            windowSize,
            defaultIndex = anchorIndex.coerceIn(0, alternatives.lastIndex.coerceAtLeast(0)),
            itemCount = alternatives.size,
            itemWidthPx = itemWidthPx,
        )
    }
    Popup(
        popupPositionProvider = positionProvider,
        properties = PopupProperties(focusable = false),
    ) {
        Row(
            modifier = Modifier
                .clip(RoundedCornerShape(8.dp))
                .background(palette.keyBg)
                .padding(4.dp),
        ) {
            alternatives.forEachIndexed { index, alternative ->
                val selected = index == selectedIndex
                Box(
                    modifier = Modifier
                        .width(KEY_POPUP_ITEM_WIDTH)
                        .height(KEY_POPUP_ITEM_HEIGHT)
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
 * 弹层定位：优先键上方，且让**初始选中项**正对发起键（横向一行时整块居中会让
 * 9 键默认的末位数字偏到一边）；上方放不下（首行键、英文数字行之下）翻到键下方。
 * 横纵都夹在键盘窗口内 —— 弹层是独立 Popup 窗口，不夹住就会盖到键盘外。
 *
 * ⚠️ [keyboardWindowSize] 必须是**键盘窗口**尺寸（px）：Compose 传给 provider 的 windowSize
 * 是整个可见屏（`getWindowVisibleDisplayFrame`），在 IME 里比键盘窗高得多，
 * 拿它夹会把弹层放到键盘外。键位坐标本身是窗口坐标，见 KeyButton 传入的 LocalView 尺寸。
 */
internal class KeyPopupPositionProvider(
    private val keyboardWindowSize: IntSize,
    private val defaultIndex: Int = 0,
    private val itemCount: Int = 1,
    private val itemWidthPx: Float = 0f,
) : PopupPositionProvider {
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
        // 默认格中心对准键中心：内容宽 = 格宽 × 条数 + 两侧 padding。
        val paddingPx = ((popupContentSize.width - itemCount * itemWidthPx) / 2f).coerceAtLeast(0f)
        val anchorOffset = if (itemWidthPx > 0f) {
            popupAnchorOffset(paddingPx, defaultIndex, itemWidthPx)
        } else {
            popupContentSize.width / 2
        }
        val x = anchorBounds.center.x - anchorOffset
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
