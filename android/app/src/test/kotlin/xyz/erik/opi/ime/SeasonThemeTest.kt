// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import org.junit.Assert.assertEquals
import org.junit.Test
import java.time.LocalDate
import java.time.MonthDay

/**
 * 四季判定（2026-10-17 用户裁决「通过获取公开的本地时间来确定四季，根据本地时间锚点
 * 配置的四季切换显示」）：判据是**本地日期 + 锚点配置**，跨年段（12/1–2月末 = 冬）正确。
 */
class SeasonThemeTest {
    private fun season(month: Int, day: Int) =
        SeasonAnchors.DEFAULT.seasonAt(LocalDate.of(2026, month, day))

    @Test
    fun defaultAnchorsFollowTheMeteorologicalSeasons() {
        assertEquals("1 月 = 冬", Season.WINTER, season(1, 15))
        assertEquals("2 月末 = 冬", Season.WINTER, season(2, 28))
        assertEquals("3/1 = 春", Season.SPRING, season(3, 1))
        assertEquals("5/31 = 春", Season.SPRING, season(5, 31))
        assertEquals("6/1 = 夏", Season.SUMMER, season(6, 1))
        assertEquals("8/31 = 夏", Season.SUMMER, season(8, 31))
        assertEquals("9/1 = 秋", Season.AUTUMN, season(9, 1))
        assertEquals("11/30 = 秋", Season.AUTUMN, season(11, 30))
        assertEquals("12/1 = 冬", Season.WINTER, season(12, 1))
        assertEquals("12/31 = 冬", Season.WINTER, season(12, 31))
    }

    @Test
    fun customAnchorsMoveTheSwitches() {
        // 换一套锚点（例：按农历习惯提前换季）—— 判据单一来源，改配置就够。
        val anchors = SeasonAnchors(
            spring = MonthDay.of(2, 4),
            summer = MonthDay.of(5, 5),
            autumn = MonthDay.of(8, 7),
            winter = MonthDay.of(11, 7),
        )
        assertEquals(Season.WINTER, anchors.seasonAt(LocalDate.of(2026, 2, 3)))
        assertEquals(Season.SPRING, anchors.seasonAt(LocalDate.of(2026, 2, 4)))
        assertEquals(Season.SUMMER, anchors.seasonAt(LocalDate.of(2026, 5, 5)))
        assertEquals(Season.AUTUMN, anchors.seasonAt(LocalDate.of(2026, 8, 7)))
        assertEquals(Season.WINTER, anchors.seasonAt(LocalDate.of(2026, 11, 7)))
    }
}
