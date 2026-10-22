// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 工具条图标尺寸门禁（[toolbarIconSizeDp]）：常见手机宽度不溢出、大屏保持 40dp、
 * 极窄窗口不低于最小触控尺寸。
 *
 * 固定开销 76dp = 小欧 36dp + 英文「中」键预留 40dp；7 枚图标。
 */
class ImeToolbarSizingTest {

    private val reserved = 76f
    private val icons = 7

    private fun totalWidth(width: Float): Float = reserved + icons * toolbarIconSizeDp(width)

    @Test
    fun commonPhoneWidthsFit() {
        for (width in listOf(320f, 360f, 390f, 411f, 430f)) {
            assertTrue(
                "宽 ${width}dp 时工具条需要 ${totalWidth(width)}dp，会裁掉最右图标",
                totalWidth(width) <= width + 0.01f,
            )
        }
    }

    @Test
    fun wideScreensKeepOriginalIconSize() {
        assertEquals(40f, toolbarIconSizeDp(390f), 0.001f)
        assertEquals(40f, toolbarIconSizeDp(430f), 0.001f)
    }

    @Test
    fun narrowestPhoneShrinksIcons() {
        // 320dp 是任务要求覆盖的最窄手机：图标 34.86dp，仍大于 26dp 下限。
        assertEquals(34.857f, toolbarIconSizeDp(320f), 0.01f)
    }

    @Test
    fun tinyWindowStopsAtMinimumTouchSize() {
        assertEquals(26f, toolbarIconSizeDp(200f), 0.001f)
    }
}
