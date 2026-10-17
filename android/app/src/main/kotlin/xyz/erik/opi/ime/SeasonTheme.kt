// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import java.time.LocalDate
import java.time.MonthDay

/** 四季（顶部花纹边框的切换依据）。 */
enum class Season { SPRING, SUMMER, AUTUMN, WINTER }

/**
 * 四季切换的**本地时间锚点配置**（2026-10-17 用户裁决「通过获取公开的本地时间来确定
 * 四季，根据本地时间锚点配置的四季切换显示」）。
 *
 * 判据 = 本地日期 + 这里的锚点；默认取公历气象四季（3/1、6/1、9/1、12/1 起算）。
 * 要改「什么时候算换季」就改这一处 —— 它是唯一来源（宿主在每次键盘隐藏/关闭时
 * 用 [seasonAt] 重新校准，见 `OpiImeService.onWindowHidden` 与 `ImeState.recalibrateSeason`）。
 *
 * 跨年规则：12/1 起至次年春季锚点前 = 冬（`LocalDate` 自带年份，比较按月日进行）。
 */
data class SeasonAnchors(
    val spring: MonthDay = MonthDay.of(3, 1),
    val summer: MonthDay = MonthDay.of(6, 1),
    val autumn: MonthDay = MonthDay.of(9, 1),
    val winter: MonthDay = MonthDay.of(12, 1),
) {
    /**
     * 本地日期 → 季节。做法：把日期映射到「月日轴上的一年」（MonthDay 可直接比较）+ 年份无关：
     * 冬（monthDay ≥ winter 或 < spring）→ 其余按锚点区间。
     * 锚点须满足 spring < summer < autumn < winter（构造方保证；乱序不在支持面内）。
     */
    fun seasonAt(date: LocalDate): Season {
        val md = MonthDay.of(date.month, date.dayOfMonth)
        return when {
            md >= winter || md < spring -> Season.WINTER
            md < summer -> Season.SPRING
            md < autumn -> Season.SUMMER
            else -> Season.AUTUMN
        }
    }

    companion object {
        /** 默认锚点（公历气象四季）。 */
        val DEFAULT = SeasonAnchors()
    }
}

/** 当前本地日期对应哪个季节（公开的本地时钟，不联网）。 */
fun currentSeason(anchors: SeasonAnchors = SeasonAnchors.DEFAULT): Season =
    anchors.seasonAt(LocalDate.now())
