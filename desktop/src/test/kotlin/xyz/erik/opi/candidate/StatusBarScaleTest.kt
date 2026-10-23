// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏统一键大小（2026-10-19 用户裁决）：配置落盘与 `BarMetrics` 的单一系数。
//!
//! 像素/比例的门禁在 StatusBarRenderProbeTest（普通面 150%）与 KeycapRenderTest
//! （键帽 150%）；这里只钉「一个值进、全组尺寸出一份」的纯函数与 ini 语义 ——
//! 状态栏窗、按钮、条宽都从同一份 [BarMetrics] 派生，别出现第二份尺寸表。

package xyz.erik.opi.candidate

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class StatusBarScaleTest {

    @Test
    fun `scale defaults to 100 and clamps hand-edited values`() {
        // 缺键 = 100（与数据类缺省一致；SettingsTest 的整对象比对也覆盖这条）。
        assertEquals(STATUS_BAR_SCALE_DEFAULT, StatusBarSettings().scale)
        assertEquals(StatusBarSettings(), OpSettings.fromIni("Learn=False\n").statusbar)
        assertEquals(100, OpSettings.fromIni("StatusBarScale=oops\n").statusbar.scale)
        assertEquals(75, OpSettings.fromIni("StatusBarScale=1\n").statusbar.scale)
        assertEquals(150, OpSettings.fromIni("StatusBarScale=999\n").statusbar.scale)
        assertEquals(125, OpSettings.fromIni("StatusBarScale=125\n").statusbar.scale)
    }

    @Test
    fun `scale persists through toIni merge and roundtrip`() {
        val s = OpSettings(statusbar = StatusBarSettings(scale = 125))
        assertTrue(s.toIni(), s.toIni().contains("StatusBarScale=125\n"))
        assertEquals(s, OpSettings.fromIni(s.toIni()))
        // 合并写：旧值必须被新值顶掉（漏进 managed 表 = 拨了不动）。
        val merged = mergeIni(OpSettings(statusbar = StatusBarSettings(scale = 75)).toIni(), s)
        assertTrue(merged, merged.contains("StatusBarScale=125\n"))
        assertFalse(merged, merged.contains("StatusBarScale=75\n"))
    }

    @Test
    fun `bar metrics derive every dimension from one factor`() {
        val m100 = barMetrics(100)
        val m150 = barMetrics(150)
        assertEquals(BTN.dp, m100.button)
        assertEquals(42.dp, m100.height) // 100% 的基准条高
        assertEquals(45.dp, m150.button)
        assertEquals(63.dp, m150.height)
        // 间距/内边距是私有基准，按同一系数比（100% → 150% 恰为 1.5×）。
        assertEquals(m100.gap.value * 1.5f, m150.gap.value, 1e-4f)
        assertEquals(m100.pad.value * 1.5f, m150.pad.value, 1e-4f)
        // 越界值按边界算 —— 与 fromIni 的收口同一条。
        assertEquals(barMetrics(150), barMetrics(999))
        assertEquals(barMetrics(75), barMetrics(1))
    }

    @Test
    fun `bar width uses the same metrics as the buttons`() {
        // 条宽不能自己另算一套：逐档用 BarMetrics 拼出期望（pad 两侧 + 按钮 + 间距）。
        for (scale in STATUS_BAR_SCALE_CHOICES) {
            val m = barMetrics(scale)
            val n = 6
            val expected = 2 * m.pad.value + n * m.button.value + (n - 1) * m.gap.value
            assertEquals(expected, statusBarWidthDp(n, scale), 1e-3f)
            // 空条也有正的宽度（既有用例同口径）。
            assertTrue(statusBarWidthDp(0, scale) > 0f)
        }
        // 设置页档位都在合法区间内，且缺省 100% 可选中。
        assertTrue(100 in STATUS_BAR_SCALE_CHOICES)
        assertTrue(STATUS_BAR_SCALE_CHOICES.all { statusBarScalePercent(it) == it })
    }

    @Test
    fun `windows compact factor shrinks every drawn size but not the persisted scale`() {
        // 2026-10-24 用户报障「win端的状态栏整体大了」：Windows 100% = Linux 的 85%，
        // 逐项同一系数（按钮/间距/内边距/条高）。Linux 不乘 —— 既有像素门禁不受影响。
        val linux = barMetrics(100)
        val win = barMetrics(100, windows = true)
        assertEquals(BTN.dp, linux.button)
        assertEquals(BTN * WINDOWS_BAR_FACTOR, win.button.value, 1e-4f)
        assertEquals(42f * WINDOWS_BAR_FACTOR, win.height.value, 1e-4f)
        assertEquals(linux.gap.value * WINDOWS_BAR_FACTOR, win.gap.value, 1e-4f)
        assertEquals(linux.pad.value * WINDOWS_BAR_FACTOR, win.pad.value, 1e-4f)
        // 条宽走同一个 barScaleFactor；设置页档位在 Windows 上仍然有意义
        // （150% 仍大于 100%，只是整体比 Linux 小一档）。
        val n = 6
        for (scale in STATUS_BAR_SCALE_CHOICES) {
            assertEquals(
                statusBarWidthDp(n, scale) * WINDOWS_BAR_FACTOR,
                statusBarWidthDp(n, scale, windows = true),
                1e-3f,
            )
        }
        assertTrue(statusBarWidthDp(n, 150, windows = true) > statusBarWidthDp(n, 100, windows = true))
        // 紧凑只在绘制层：配置里读到的仍是用户选的百分比（StatusBarSettings 不认识 Windows）。
        assertEquals(125, OpSettings.fromIni("StatusBarScale=125\n").statusbar.scale)
        assertEquals(1.25f * WINDOWS_BAR_FACTOR, barScaleFactor(125, windows = true), 1e-6f)
        assertEquals(1.25f, barScaleFactor(125, windows = false), 1e-6f)
    }

    @Test
    fun `only windows gets the pill shape`() {
        // 2026-10-24 用户报障「边角也不是弧形」：Windows = 胶囊（配合透明窗，四角真透明）；
        // Linux 未报障 ⇒ 直角。形状是纯函数，绘制层见 StatusBar.kt 的根 Box。
        assertEquals(RectangleShape, barShape(windows = false))
        assertTrue(barShape(windows = true) is RoundedCornerShape)
        assertNotEquals(barShape(windows = false), barShape(windows = true))
    }
}
