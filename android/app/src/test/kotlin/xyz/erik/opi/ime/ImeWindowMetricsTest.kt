// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * IME 窗口高度计算的门禁（[ImeWindowMetrics]）：竖屏手机像素不变、横屏不再吃掉 77%
 * 屏高、平板不随宽度无限放大、任何窗口里都不超过窗口高度。
 *
 * 数值取自本机模拟器口径（1080×2400 @ density 2.625）与常见平板（1600×2560 @ 2.0）。
 */
class ImeWindowMetricsTest {

    private val phoneDensity = 2.625f

    /** 竖屏手机：既有值 = 0.48×1080 + (76+38)×2.625 ≈ 817px，无 cap 生效。 */
    @Test
    fun portraitPhoneKeepsExistingPixels() {
        val expected = (1080 * 0.48f + 114 * 2.625f).toInt()
        assertEquals(expected, ImeWindowMetrics.keyboardHeightPx(1080, 2400, phoneDensity))
    }

    /** 横屏手机：旧公式 817px / 1080 = 76%，新规则压到 60%（594px）。 */
    @Test
    fun landscapePhoneIsCappedToSixTenths() {
        val h = ImeWindowMetrics.keyboardHeightPx(2400, 1080, phoneDensity)
        assertEquals((1080 * 0.60f).toInt(), h)
        assertTrue("横屏键盘不得超过屏高 60%：$h", h * 100 <= 1080 * 60)
    }

    /** 平板竖屏：旧公式 0.48×1600 + 228 = 996px（498dp），绝对上限 340dp = 680px。 */
    @Test
    fun tabletPortraitIsCappedByAbsoluteHeight() {
        assertEquals(680, ImeWindowMetrics.keyboardHeightPx(1600, 2560, 2.0f))
    }

    /** 平板横屏：绝对上限优先于 60% 屏高（后者是 960px）。 */
    @Test
    fun tabletLandscapeUsesAbsoluteCap() {
        assertEquals(680, ImeWindowMetrics.keyboardHeightPx(2560, 1600, 2.0f))
    }

    /** 分屏/自由窗口再矮也不能超过窗口本身（原实现也只有 coerceAtMost(h)）。 */
    @Test
    fun neverExceedsWindowHeight() {
        val windowH = 320
        val h = ImeWindowMetrics.keyboardHeightPx(1200, windowH, 2.625f)
        assertTrue("键盘高度 $h 超过窗口 $windowH", h <= windowH)
        assertTrue("高度必须为正", h > 0)
    }
}
