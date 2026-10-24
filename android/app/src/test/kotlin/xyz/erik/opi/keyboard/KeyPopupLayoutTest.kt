// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import org.junit.Assert.assertEquals
import org.junit.Test

/** 横向弹层的两段纯几何（2026-10-25 用户订正：竖排改横排）。 */
class KeyPopupLayoutTest {

    @Test
    fun draggingRightSelectsLaterAlternatives() {
        // 首项在最左，向右拖一格选下一项。
        assertEquals(1, popupSelectionForDrag(start = 0, dragXPx = 44f, itemWidthPx = 44f, count = 3))
        assertEquals(2, popupSelectionForDrag(start = 1, dragXPx = 44f, itemWidthPx = 44f, count = 3))
        // 向左拖回前一项。
        assertEquals(0, popupSelectionForDrag(start = 1, dragXPx = -44f, itemWidthPx = 44f, count = 3))
    }

    @Test
    fun selectionClampsAtBothEnds() {
        assertEquals(0, popupSelectionForDrag(0, -500f, 44f, 5))
        assertEquals(4, popupSelectionForDrag(4, 500f, 44f, 5))
        // 缺省项是末位（9 键数字）：向左拖选字母，向右拖不越界。
        assertEquals(4, popupSelectionForDrag(4, 10f, 44f, 5))
    }

    @Test
    fun anchorOffsetPutsTheDefaultCellOverTheKey() {
        // 三格、默认中间：弹层左边 = 键中心 - 1.5 格。
        assertEquals(66, popupAnchorOffset(paddingPx = 0f, defaultIndex = 1, itemWidthPx = 44f))
        // 五格、默认末位：左边 = 键中心 - 4.5 格。
        assertEquals(198, popupAnchorOffset(paddingPx = 0f, defaultIndex = 4, itemWidthPx = 44f))
        // 计入两侧 padding（整块内容的左内边距）。
        assertEquals(74, popupAnchorOffset(paddingPx = 8f, defaultIndex = 1, itemWidthPx = 44f))
    }
}
