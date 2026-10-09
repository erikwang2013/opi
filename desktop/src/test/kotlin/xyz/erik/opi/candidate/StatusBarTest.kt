// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏的**纯逻辑**门禁（无 UI、无进程、无 JNA）：可见性判定、命令映射、条宽。
//!
//! 为什么单列一份：这几条判定在状态栏窗与设置窗里**各用一次**（同一份纯函数），
//! 两处各写一遍必然漂移；而漂移的表现是「设置里关了、条上还在」这种一眼假的错。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class StatusBarTest {

    @Test
    fun `default is all eight buttons visible`() {
        // 2026-10-10 用户裁决「8 按钮全上」—— 默认值不是「全关」。
        assertEquals(StatusBarButton.entries.toList(), visibleStatusButtons(StatusBarSettings(), windows = false))
        assertEquals(8, StatusBarButton.entries.size)
    }

    @Test
    fun `button order is the display order`() {
        // 枚举序 = 条上从左到右（用户裁决的列举顺序）。换序 = 用户肌肉记忆全废，
        // 且没有任何东西会报错，所以钉住。
        assertEquals(
            listOf("软键盘", "符号大全", "简繁转换", "中英标点", "中英文切换", "logo", "全半角", "设置"),
            StatusBarButton.entries.map { it.label },
        )
    }

    @Test
    fun `master hidden wins over every resident switch`() {
        val allOnButHidden = StatusBarSettings(hidden = true)
        assertTrue(visibleStatusButtons(allOnButHidden, windows = false).isEmpty())
    }

    @Test
    fun `each button can be turned off alone`() {
        // 覆盖 resident ⇄ with 的八对分支：任一对写错（比如 VK 那行复制成 SYMBOLS），
        // 这条就在那个按钮上红。
        for (b in StatusBarButton.entries) {
            val shown = visibleStatusButtons(StatusBarSettings().with(b, false), windows = false)
            assertFalse("$b 关掉后不该在条上", b in shown)
            assertEquals("关掉 $b 只该少它一个", 7, shown.size)
        }
    }

    @Test
    fun `turning a button back on restores it`() {
        val off = StatusBarButton.entries.fold(StatusBarSettings()) { sb, b -> sb.with(b, false) }
        assertTrue(visibleStatusButtons(off, windows = false).isEmpty())
        assertEquals(
            listOf(StatusBarButton.TRAD),
            visibleStatusButtons(off.with(StatusBarButton.TRAD, true), windows = false),
        )
    }

    @Test
    fun `windows only keeps buttons that have an action there`() {
        // Windows 上没有 opi-ctl / fcitx5-remote（那四个动作是 Linux 侧的），
        // 没动作的按钮不上条 ——「宁可少一页，不做点不动的开关」。
        assertEquals(
            listOf(StatusBarButton.VK, StatusBarButton.SYMBOLS, StatusBarButton.LOGO, StatusBarButton.SETTINGS),
            visibleStatusButtons(StatusBarSettings(), windows = true),
        )
        // 设置窗的开关行用同一个判定（两处同源）
        assertEquals(4, StatusBarButton.entries.count { statusButtonWired(it, windows = true) })
        assertEquals(8, StatusBarButton.entries.count { statusButtonWired(it, windows = false) })
    }

    @Test
    fun `command mapping is the frozen opi-ctl contract`() {
        // 契约：`opi-ctl <action>`，动作名 2026-10-10 冻结（另一条轨实现）。
        // 改这里的字符串 = 状态栏点了没反应（进程起来了、退出码 0、什么都没发生）：
        // 这是跨轨契约，本仓惯例是「钉死在测试里」。
        assertEquals(listOf("opi-ctl", "symbols"), statusActionCommand(StatusBarButton.SYMBOLS))
        assertEquals(listOf("opi-ctl", "trad"), statusActionCommand(StatusBarButton.TRAD))
        assertEquals(listOf("opi-ctl", "punct"), statusActionCommand(StatusBarButton.PUNCT))
        assertEquals(listOf("opi-ctl", "fullwidth"), statusActionCommand(StatusBarButton.FULLWIDTH))
        // 中英文切换走 fcitx5 自带工具（与 Shift 轻按同一条通路）
        assertEquals(listOf("fcitx5-remote", "-t"), statusActionCommand(StatusBarButton.LANG))
        // 这三个是进程内动作（开键盘窗 / 开符号页 / 开设置窗），不出命令
        assertNull(statusActionCommand(StatusBarButton.VK))
        assertNull(statusActionCommand(StatusBarButton.LOGO))
        assertNull(statusActionCommand(StatusBarButton.SETTINGS))
    }

    @Test
    fun `bar width follows the button count`() {
        // 不钉具体像素（那是版面微调的自由）；钉的是「空条也有宽度」与「每多一个按钮
        // 只多一格 + 一个间距」—— 后者错了就是按钮被裁掉或挤出空白。
        assertTrue("空条宽度也得是正的", statusBarWidthDp(0) > 0)
        assertEquals(
            statusBarWidthDp(2) - statusBarWidthDp(1),
            statusBarWidthDp(9) - statusBarWidthDp(8),
        )
        assertTrue(statusBarWidthDp(8) > statusBarWidthDp(4))
    }
}
