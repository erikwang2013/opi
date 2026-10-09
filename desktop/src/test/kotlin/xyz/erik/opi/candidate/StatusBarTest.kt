// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏的**纯逻辑**门禁（无 UI、无进程、无 JNA）：可见性判定、命令映射、条宽、
//! 状态解析与图标选择、动作后补读。
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
        // 枚举序 = 条上从左到右（2026-10-10 深夜**最终冻结**：logo/中英/。，/简繁/
        // 月相/键盘/Ω/设置）。换序 = 用户肌肉记忆全废，且没有任何东西会报错，所以钉住。
        assertEquals(
            listOf("logo", "中英文切换", "符号大全", "简繁转换", "全半角", "软键盘", "中英标点", "设置"),
            StatusBarButton.entries.map { it.label },
        )
    }

    @Test
    fun `frozen icon literals stay as the user picked them`() {
        // 用户两轮都按「句号、逗号」写（先句号）——按字面来，别按中文标点惯例改回
        // 「，。」；Ω 表「标点/特殊字符」，也不带状态显示。改字前先回去问。
        assertEquals("。，", iconText(StatusBarButton.SYMBOLS, ImeState()))
        assertEquals("Ω", iconText(StatusBarButton.PUNCT, ImeState()))
        // 单字随状态（与 tradIcon/langIcon 同源，两处漂了这里会红）
        assertEquals("繁", iconText(StatusBarButton.TRAD, ImeState(trad = true)))
        assertEquals("简", iconText(StatusBarButton.TRAD, ImeState(trad = false)))
        assertEquals("英", iconText(StatusBarButton.LANG, ImeState(zh = false)))
        assertEquals("中", iconText(StatusBarButton.LANG, ImeState(zh = true)))
        // 画图标的四个不出字（出字就会在自绘图形上叠一层文字）
        for (b in listOf(
            StatusBarButton.VK, StatusBarButton.LOGO,
            StatusBarButton.FULLWIDTH, StatusBarButton.SETTINGS,
        )) {
            assertEquals("$b 该画图标、不出字", "", iconText(b, ImeState()))
        }
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
            listOf(StatusBarButton.LOGO, StatusBarButton.SYMBOLS, StatusBarButton.VK, StatusBarButton.SETTINGS),
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

    // ---------- 状态通道：解析与图标选择（2026-10-10 深夜裁决②）----------

    @Test
    fun `state line parses every field`() {
        // 契约：`opi-ctl state` 一行 lang=zh|en;trad=on|off;punct=on|off;fw=on|off。
        assertEquals(
            ImeState(zh = false, trad = true, punct = false, fullwidth = true),
            parseImeState("lang=en;trad=on;punct=off;fw=on"),
        )
        assertEquals(
            ImeState(zh = true, trad = false, punct = true, fullwidth = false),
            parseImeState("lang=zh;trad=off;punct=on;fw=off"),
        )
    }

    @Test
    fun `sample state lines pick the icons`() {
        // 「样例串 → 图标选择」：简/繁、中/英随状态翻转；月相看 fullwidth 位
        // （满月 = 全角、半月 = 半角，见 StatusBar.kt 的 MoonIcon）。
        assertEquals("繁", tradIcon(parseImeState("trad=on")))
        assertEquals("简", tradIcon(parseImeState("trad=off")))
        assertEquals("英", langIcon(parseImeState("lang=en")))
        assertEquals("中", langIcon(parseImeState("lang=zh")))
        assertTrue(parseImeState("fw=on").fullwidth) // 满月
        assertFalse(parseImeState("fw=off").fullwidth) // 半月
    }

    @Test
    fun `bad or partial state falls back to the neutral defaults`() {
        // 容错是规格的一部分：读失败（通道不通 ⇒ runCapture 给不出行为 null）/坏串/
        // 认不得的值/空输出都 ⇒ 中性缺省（中/简/半月），不抛、条上照常渲染。
        // 缺省语义定案见 Actions.kt 的 ImeState。
        val neutral = ImeState()
        assertEquals(neutral, parseImeState(null))
        assertEquals(neutral, parseImeState(""))
        assertEquals(neutral, parseImeState("这不是状态"))
        assertEquals(neutral, parseImeState("lang=fr;trad=yes;punct=maybe;fw=1"))
        // 缺键 = 只有那个键落缺省，不把整行判坏（格式加字段时旧前端不至于全瞎）
        assertEquals(ImeState(zh = false), parseImeState("lang=en"))
        assertEquals(ImeState(trad = true), parseImeState("trad=on"))
        // 空格容错（手写调试）：键值两侧的空格不管，无 '=' 的碎片忽略
        assertEquals(
            ImeState(zh = false, fullwidth = true),
            parseImeState(" lang = en ; junk ; fw = on "),
        )
    }

    @Test
    fun `action always refreshes state right after it returns`() {
        // 「点完立刻翻转」的落点：动作返回后一定补读一次。抽成纯函数就是为了这条 ——
        // 埋在 Compose 里的话，将来重构掉了它，唯一表现是「点完等 1s 才变」，
        // 没有任何测试会红。
        val order = mutableListOf<String>()
        val r = actionWithStateRefresh({ order.add("action"); 42 }, { order.add("refresh") })
        assertEquals(42, r)
        assertEquals(listOf("action", "refresh"), order)
        // 动作抛异常也补读（finally）：否则一次异常之后状态可能一直停在旧值。
        val order2 = mutableListOf<String>()
        try {
            actionWithStateRefresh({ order2.add("boom"); error("动作炸了") }, { order2.add("refresh") })
        } catch (_: IllegalStateException) {
        }
        assertEquals(listOf("boom", "refresh"), order2)
    }
}
