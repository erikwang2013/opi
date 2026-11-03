// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏轮询节奏的纯逻辑门禁（2026-11-03）：切回 opi 必须尽快出现，读失败必须快重试。
//! 真机故障链：fcitx5 切换输入法时主循环卡住（`opi-ctl state` 与 `fcitx5-remote -n`
//! 同一时刻超时）⇒ 旧节奏读失败后等满 1s，连续两次卡顿就让状态栏在切回后藏 4s 以上
//!（终端里最容易撞上，用户报障「切回来也不显示」）。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Test

class StatusBarPollTest {

    @Test
    fun `poll speeds up while hidden and retries immediately after a failed read`() {
        // 正常/可见：1s 一拍（设置文件同步 + 状态刷新）。
        assertEquals(1000L, statePollDelayMs(readFailed = false, im = "opi", windows = false))
        assertEquals(1000L, statePollDelayMs(readFailed = false, im = "", windows = false))
        // 隐藏在其他输入法：500ms 一拍，切回 opi 最迟半秒内被发现。
        assertEquals(500L, statePollDelayMs(readFailed = false, im = SOGOU, windows = false))
        // 读失败：200ms 立即重试，不把一次 fcitx5 卡顿拖成多秒。
        assertEquals(200L, statePollDelayMs(readFailed = true, im = SOGOU, windows = false))
        assertEquals(200L, statePollDelayMs(readFailed = true, im = "opi", windows = false))
        // Windows 有事件通道（StatusEventWatcher），节奏保持 1s。
        assertEquals(1000L, statePollDelayMs(readFailed = true, im = SOGOU, windows = true))
    }

    @Test
    fun `hidden polls use a short timeout so a stalled fcitx5 is abandoned quickly`() {
        // 正常查询 20–50ms；隐藏时 800ms 足够，卡住就快点放弃重试。
        assertEquals(800L, statePollTimeoutMs(SOGOU, windows = false))
        // 可见/未知/Windows 保持 1.5s（高负载机器上的余量）。
        assertEquals(1500L, statePollTimeoutMs("opi", windows = false))
        assertEquals(1500L, statePollTimeoutMs("", windows = false))
        assertEquals(1500L, statePollTimeoutMs(SOGOU, windows = true))
    }

    private companion object {
        const val SOGOU = "com.sogou.ime.ng.fcitx5.deepin"
    }
}
