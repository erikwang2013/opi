// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏 1s 轮询的「连续读不到 ⇒ 回中性」判定门禁（纯函数，无 UI/进程/JNA）。
//!
//! 2026-10-22 真机：切换到别的输入法后 OPI 状态栏仍在。TSF 侧已加 profile 切换
//! 监听断开候选管道；本侧钉住管道断开后的兜底 —— 状态读不到连续 3 拍，`im` 必须
//! 清空（Windows 上 `barVisible(requireActive=true)` 随之隐藏），而不是停在旧值。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class StatePollTest {
    @Test
    fun third_consecutive_miss_clears_the_bar() {
        var fails = 0
        repeat(2) { attempt ->
            val poll = nextStatePoll(fails, readOk = false, imPresent = true)
            assertFalse("第 ${attempt + 1} 拍是短暂抖动，应保留旧状态", poll.clear)
            fails = poll.fails
        }
        val third = nextStatePoll(fails, readOk = false, imPresent = true)
        assertEquals(STATE_CLEAR_AFTER_FAILS, third.fails)
        assertTrue("连续第 3 拍读不到必须回中性（Windows = 隐藏）", third.clear)
    }

    @Test
    fun successful_read_resets_the_counter() {
        val poll = nextStatePoll(previousFails = 2, readOk = true, imPresent = true)
        assertEquals(0, poll.fails)
        assertFalse(poll.clear)
    }

    @Test
    fun nothing_to_clear_when_im_is_already_empty() {
        val poll = nextStatePoll(previousFails = 2, readOk = false, imPresent = false)
        assertEquals(STATE_CLEAR_AFTER_FAILS, poll.fails)
        assertFalse("im 已为空：没有旧状态可清", poll.clear)
    }

    @Test
    fun counter_stays_capped_while_misses_continue() {
        assertEquals(STATE_CLEAR_AFTER_FAILS, nextStatePoll(99, readOk = false, imPresent = true).fails)
    }
}
