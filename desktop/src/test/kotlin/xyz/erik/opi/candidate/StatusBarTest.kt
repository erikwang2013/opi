// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏的**纯逻辑**门禁（无 UI、无进程、无 JNA）：可见性判定、命令映射、条宽、
//! 状态解析与图标选择、动作后补读。
//!
//! 为什么单列一份：这几条判定在状态栏窗与设置窗里**各用一次**（同一份纯函数），
//! 两处各写一遍必然漂移；而漂移的表现是「设置里关了、条上还在」这种一眼假的错。

package xyz.erik.opi.candidate

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class StatusBarTest {

    @Test
    fun `default shows every button except the soft keyboard`() {
        // 2026-10-10 早先裁决「8 按钮全上」被**同日中午**的用户裁决收窄：软键盘默认关
        // （要用的在设置里自己开；已有配置里的显式值照旧）。其余七个仍默认常驻 ——
        // 「全关」会让老用户升级后条上什么都不剩。
        assertEquals(
            StatusBarButton.entries.filter { it != StatusBarButton.VK },
            visibleStatusButtons(StatusBarSettings(), windows = false),
        )
        assertEquals(7, visibleStatusButtons(StatusBarSettings(), windows = false).size)
        assertFalse("软键盘默认必须是关（2026-10-10 中午裁决）", StatusBarSettings().vk)
        assertFalse("缺键的旧配置读出来也得是关（fromIni 的回落别写成 true）", OpSettings.fromIni("").statusbar.vk)
    }

    @Test
    fun `button order is the display order`() {
        // 枚举序 = 条上从左到右（用户 2026-10-10 **最终冻结**：logo/中英/。，/简繁/
        // 月相/键盘/Ω/设置；第 3 位 = 标点「。，」、第 7 位 = 符号大全「Ω」）。
        // 换序 = 用户肌肉记忆全废，且没有任何东西会报错，所以钉住。
        // ⚠️ 边界（验收时发生过的错）：这条只锁**顺序**，锁不住「哪枚图标在哪个键上」
        // —— 当时图标映射先改对、枚举序没跟着换，这条照样全绿；图标语义归
        // `frozen icon literals` 那条管。两处配套，只改一处时对面的钉要跟着动。
        assertEquals(
            listOf("logo", "中英文切换", "中英标点", "简繁转换", "全半角", "软键盘", "符号大全", "设置"),
            StatusBarButton.entries.map { it.label },
        )
    }

    @Test
    fun `frozen icon literals stay as the user picked them`() {
        // 两轮裁决的**时间线**（别按任一半截「改回去」）：
        // · 2026-10-10 深夜 B 版：SYMBOLS = 「。，」静态、PUNCT = 「Ω」静态；
        // · 同日**中午订正**（按用户对按钮行为的描述）：两枚**互换**且 PUNCT 双态 ——
        //   SYMBOLS（符号大全）= 静态「Ω」；PUNCT（中英标点）= punct on「。，」/ off「.,」。
        // 字面都是用户写的：「。，」中文侧**先句号**（勿按惯例改成「，。」）；「.,」
        // 英文侧**先点号** —— 都不是笔误。
        assertEquals("Ω", iconText(StatusBarButton.SYMBOLS, ImeState()))
        assertEquals("。，", iconText(StatusBarButton.PUNCT, ImeState(punct = true)))
        assertEquals(".,", iconText(StatusBarButton.PUNCT, ImeState(punct = false)))
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
        // 这条就在那个按钮上红。基线取**全开**（默认里 VK 已是关，拿默认当基线的话
        // 「关 VK」是空操作，那条分支就测不到了）。
        val allOn = StatusBarButton.entries.fold(StatusBarSettings()) { sb, b -> sb.with(b, true) }
        for (b in StatusBarButton.entries) {
            assertEquals(
                "关掉 $b 只该少它一个",
                StatusBarButton.entries.filter { it != b },
                visibleStatusButtons(allOn.with(b, false), windows = false),
            )
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
        // Windows 上没有 opi-ctl（那五个动作 symbols/trad/punct/fullwidth/lang 是
        // Linux 侧的），没动作的按钮不上条 ——「宁可少一页，不做点不动的开关」。
        // 显式开 VK：这条测的是「按有无动作过滤」，别被默认值变更（VK 默认关）牵连。
        // 期望值按**枚举序**（VK 现在排 SYMBOLS 前面，见枚举头的验收订正）。
        assertEquals(
            listOf(StatusBarButton.LOGO, StatusBarButton.VK, StatusBarButton.SYMBOLS, StatusBarButton.SETTINGS),
            visibleStatusButtons(StatusBarSettings(vk = true), windows = true),
        )
        // 设置窗的开关行用同一个判定（两处同源）
        assertEquals(4, StatusBarButton.entries.count { statusButtonWired(it, windows = true) })
        assertEquals(8, StatusBarButton.entries.count { statusButtonWired(it, windows = false) })
    }

    @Test
    fun `command mapping is the frozen opi-ctl contract`() {
        // 契约：`opi-ctl <action>`，动作名 2026-10-10 冻结（另一条轨实现）。
        // 改这里的字符串 = 状态栏点了没反应（进程起来了、退出码 0、什么都没发生）：
        // 这是跨轨契约，本仓惯例是「钉死在测试里」。五个动作**逐条钉字面**。
        assertEquals(listOf("opi-ctl", "symbols"), statusActionCommand(StatusBarButton.SYMBOLS))
        assertEquals(listOf("opi-ctl", "trad"), statusActionCommand(StatusBarButton.TRAD))
        assertEquals(listOf("opi-ctl", "punct"), statusActionCommand(StatusBarButton.PUNCT))
        assertEquals(listOf("opi-ctl", "fullwidth"), statusActionCommand(StatusBarButton.FULLWIDTH))
        // ⚠️ 事故留痕（2026-10-10 中午用户报障）：LANG 曾是 fcitx5-remote -t ——
        // `-t` 切的是**整个 fcitx5 的启用态**，不是中英模式：用户点两下 ⇒ 全框架被
        // 停用、中文一个字打不出来（lead 真机实测指纹：点两次后 `fcitx5-remote` 报 2）。
        // 中英切换的正确动作是契约里的 `opi-ctl lang`。改回 -t 之前先读完这段。
        assertEquals(listOf("opi-ctl", "lang"), statusActionCommand(StatusBarButton.LANG))
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
        // 「样例串 → 图标选择」端到端：简/繁、中/英、标点两态随状态翻转；月相看
        // fullwidth 位（满月 = 全角、半月 = 半角，见 StatusBar.kt 的 MoonIcon）。
        assertEquals("繁", tradIcon(parseImeState("trad=on")))
        assertEquals("简", tradIcon(parseImeState("trad=off")))
        assertEquals("英", langIcon(parseImeState("lang=en")))
        assertEquals("中", langIcon(parseImeState("lang=zh")))
        // 标点双态（2026-10-10 中午订正）：整行状态串 → 按钮面
        assertEquals("。，", iconText(StatusBarButton.PUNCT, parseImeState("lang=zh;punct=on;fw=off")))
        assertEquals(".,", iconText(StatusBarButton.PUNCT, parseImeState("lang=zh;punct=off;fw=off")))
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

    // ---------- 切到别的输入法就藏（2026-10-10 下午用户裁决）----------

    @Test
    fun `state line carries the im field`() {
        // 契约扩展（Actions.kt 头）：行尾 `[;im=<uniqueName>]`。缺键 = ""（新 glue 没合时
        // 旧输出正好长这样）；空值 `im=` 也归一成未知；既有四键的解析不受影响。
        assertEquals("opi", parseImeState("lang=zh;trad=off;punct=on;fw=off;im=opi").im)
        assertEquals("", parseImeState("lang=zh;trad=off;punct=on;fw=off").im) // 缺键
        assertEquals("", parseImeState("im=").im) // 空值 = 未知
        // im 是任意串（uniqueName 无枚举面），且不碰其它键
        assertEquals(
            ImeState(zh = false, trad = true, punct = false, fullwidth = true, im = "pinyin"),
            parseImeState("lang=en;trad=on;punct=off;fw=on;im=pinyin"),
        )
    }

    @Test
    fun `bar hides while another input method is active`() {
        // 四态：im 缺键/空串 = 未知 ⇒ 维持现状（可见，不闪）；"opi" ⇒ 可见；
        // 其他输入法 ⇒ 隐藏。基线按钮 = 默认设置的 7 键（非空）。
        val buttons = visibleStatusButtons(StatusBarSettings(), windows = false)
        assertTrue(barVisible(buttons, im = "")) // 缺键 = 空串缺省（旧 glue / 读失败）
        assertTrue(barVisible(buttons, im = "opi")) // 本输入法
        assertFalse(barVisible(buttons, im = "pinyin")) // 别的输入法（fcitx5 内置拼音）
        // 端到端串一下：整行 → 解析 → 判定（少一环的表现正是「切走了条还在」）
        assertFalse(barVisible(buttons, parseImeState("lang=zh;trad=off;punct=on;fw=off;im=pinyin").im))
        assertTrue(barVisible(buttons, parseImeState("lang=zh;trad=off;punct=on;fw=off;im=opi").im))
    }

    @Test
    fun `master hidden still wins with im present`() {
        // 老优先级不变：master 隐藏 / 按钮全关 ⇒ 恒隐藏，与 im 无关（隐藏不是「等 im 来救」）。
        val none = visibleStatusButtons(StatusBarSettings(hidden = true), windows = false)
        assertFalse(barVisible(none, im = ""))
        assertFalse(barVisible(none, im = "opi"))
        assertFalse(barVisible(emptyList(), im = "pinyin"))
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

    @Test
    fun `bar palette follows the skin and keep stays the built-in dark`() {
        // 2026-10-12 用户报障「设置皮肤后状态栏皮肤没同步变化」的修复：条按 `Skin=` 取色。
        // 两条判据（纯函数，色值逐项）：
        //   · KEEP（不改动）= 条自绘以来的固定深色 —— 行为必须一字不变；
        //   · 选了一款 = 该款 SKINS 的角色表（这里以宣纸为例；五角色与生成器的
        //     逐值比对在 SettingsTest 的 `skin palettes match the theme generator`）。
        assertEquals(
            "KEEP = 1.7.14 的五个常量原值",
            listOf(0xFF2D2D2DL, 0xFFE8E8E8L, 0xFF3F3F3FL, 0xFF4A4A4AL, 0xFFFF8A80L),
            listOf(
                argb(KeepColors.bg), argb(KeepColors.fg), argb(KeepColors.hover),
                argb(KeepColors.line), argb(KeepColors.accent),
            ),
        )
        assertEquals("不改动与缺省同一份", KeepColors, skinColors(SkinOption.KEEP))
        val paper = skinColors(SkinOption.PAPER)
        assertEquals(
            "宣纸 = SKINS paper 的 bg/text/hl_bg/sep/accent",
            listOf(0xFFF5EFE1L, 0xFF3D3222L, 0x1F8A5A2BL, 0xFFD8CDB4L, 0xFF8A5A2BL),
            listOf(
                argb(paper.bg), argb(paper.fg), argb(paper.hover),
                argb(paper.line), argb(paper.accent),
            ),
        )
    }

    /** `Color` → `0xAARRGGBB` 的无符号 Long（`toArgb()` 是带符号 Int，掩一下再比）。 */
    private fun argb(c: Color): Long = c.toArgb().toLong() and 0xFFFFFFFFL
}
