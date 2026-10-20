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
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class StatusBarTest {

    @Test
    fun `default shows every button except the soft keyboard`() {
        // 2026-10-10 早先裁决「8 按钮全上」被**同日中午**的用户裁决收窄：软键盘默认关
        // （要用的在设置里自己开；已有配置里的显式值照旧）。其余仍默认常驻 ——
        // 「全关」会让老用户升级后条上什么都不剩。
        // 2026-10-14 起多一枚**恒常驻**的模式按钮（见 `the mode button has no switch`）；
        // 2026-10-19 再加默认常驻的英文专业键 ENPRO：
        // 默认条上 9 枚 = 10 枚里去掉软键盘。
        assertEquals(
            StatusBarButton.entries.filter { it != StatusBarButton.VK },
            visibleStatusButtons(StatusBarSettings(), windows = false),
        )
        assertEquals(9, visibleStatusButtons(StatusBarSettings(), windows = false).size)
        assertFalse("软键盘默认必须是关（2026-10-10 中午裁决）", StatusBarSettings().vk)
        assertFalse("缺键的旧配置读出来也得是关（fromIni 的回落别写成 true）", OpSettings.fromIni("").statusbar.vk)
    }

    @Test
    fun `button order is the display order`() {
        // 枚举序 = 条上从左到右（用户 2026-10-10 **最终冻结**：logo/中英/。，/简繁/
        // 月相/键盘/Ω/设置；第 3 位 = 标点「。，」、第 7 位 = 符号大全「Ω」）。
        // 换序 = 用户肌肉记忆全废，且没有任何东西会报错，所以钉住。
        // **2026-10-14 用户裁决**插入「模式」：「状态栏在简繁按钮后增加模式切换按钮，
        // 默认拼音…」⇒ 位置钉在**简繁之后、月相之前**（本轮新增的第 5 位；用户说的是
        // 「简繁按钮后」，别按「放到最右」或「放到设置旁边」理解）。
        // ⚠️ 边界（验收时发生过的错）：这条只锁**顺序**，锁不住「哪枚图标在哪个键上」
        // —— 当时图标映射先改对、枚举序没跟着换，这条照样全绿；图标语义归
        // `frozen icon literals` 那条管。两处配套，只改一处时对面的钉要跟着动。
        // **2026-10-19 用户裁决把英文专业移到简繁之后**（旧位置在标点之前，已废止；
        // 见 EnProTest 头部的时间线）。位置 = 枚举序，没有第二处。
        assertEquals(
            listOf("logo", "中英文切换", "中英标点", "简繁转换", "英文专业", "模式", "全半角", "软键盘", "符号大全", "设置"),
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
        // MODE 也不出字：它的面由**档位 token** 驱动（modeFace/modeCorner），而 iconText
        // 的入参里根本没有档位 —— 在 iconText 里给它写字面就是把「当前档」写死成常量。
        assertEquals("MODE 该由档位 token 出字、不走 iconText", "", iconText(StatusBarButton.MODE, ImeState()))
        // ENPRO 同样不出单字：en/pro 两个槽由 enproSlots 画。
        assertEquals("ENPRO 该由 enproSlots 出字", "", iconText(StatusBarButton.ENPRO, ImeState()))
    }

    @Test
    fun `the face scale is frozen at the 2026-10-15 ruling`() {
        // 用户原话（2026-10-15）：「状态栏图标、文字在自己所在的按钮显示太大，缩小40％」
        // ⇒ 保留 60% = 0.6。**钉成字面量**的理由：这个数是**用户裁决**，不是版面微调 ——
        // 改它 = 改产品外观，得连着裁决一起改。落到哪儿见 BarGlyphs.kt 的 FACE_SCALE
        // （唯一落点，图标/字形/小欧都吃它；2026-10-19 的 enpro/en26 键帽按设计图
        // 的 26/30 比例画，是明列的例外）。
        // ⚠️ 改了要同步 StatusBarRenderProbeTest：那边的期望按 FACE_SCALE 现算（自适应，
        // 断的是「缩放真落在渲染里」），**不是**在断这个数 —— 这个数的牙只有这一条。
        assertEquals(0.6f, FACE_SCALE, 0f)
    }

    // ---------- 模式按钮（2026-10-14 用户裁决「简繁后加模式切换按钮」）----------

    @Test
    fun `the mode button has no switch and stays on the bar`() {
        // 它和另外八枚有两处不同，两条都钉住：
        // ① **没有常驻开关**（没有 `StatusBarMode=` ini 键，本批不动 Settings.kt 的读写表）
        //    ⇒ 设置窗不出它那行（出了就是拨了没反应的开关），resident/with 也不该改得动它；
        // ② 条上**一直在**（恒常驻）；2026-10-20 起两平台都有（Windows 走本机控制面，
        //    见 ControlPlane.kt —— 原先「Windows 没有 opi-ctl mode」的缺口已补）。
        assertTrue("MODE 该恒常驻", StatusBarSettings().resident(StatusBarButton.MODE))
        assertEquals(
            "MODE 没有可写的键位：with 该原样返回（不是「假装改了」）",
            StatusBarSettings(),
            StatusBarSettings().with(StatusBarButton.MODE, false),
        )
        assertFalse("设置窗不该给 MODE 出行", statusButtonWired(StatusBarButton.MODE, windows = false))
        assertFalse("MODE 没有常驻开关，两平台都不该出行", statusButtonWired(StatusBarButton.MODE, windows = true))
        // 全关（九枚开关逐个关掉）之后条上只剩 MODE —— 「恒常驻」的直接后果。
        // ⚠️ 顺带作废一条老路：「按钮全关 ⇒ 条隐藏」现在到不了了（条至少剩 MODE 一枚），
        // 要隐藏整条走 master 开关（它本来就是给用户的那条路，见 StatusBarPage 的文案）。
        val allOff = StatusBarButton.entries.filter { it != StatusBarButton.MODE }
            .fold(StatusBarSettings()) { sb, b -> sb.with(b, false) }
        assertEquals(
            listOf(StatusBarButton.MODE),
            visibleStatusButtons(allOff, windows = false),
        )
        // 两平台条面相同：MODE 恒在，ENPRO 由开关决定（显式开）
        val winOn = StatusBarButton.entries.fold(StatusBarSettings(vk = true)) { sb, b -> sb.with(b, true) }
        assertTrue("Windows 条上该有 MODE（控制面已补）", StatusBarButton.MODE in visibleStatusButtons(winOn, windows = true))
        assertTrue("Linux 条上该有 MODE", StatusBarButton.MODE in visibleStatusButtons(winOn, windows = false))
        assertTrue("Windows 条上该有 ENPRO（控制面已补）", StatusBarButton.ENPRO in visibleStatusButtons(winOn, windows = true))
        assertTrue("Linux 条上该有 ENPRO", StatusBarButton.ENPRO in visibleStatusButtons(winOn, windows = false))
    }

    @Test
    fun `mode tokens map to the frozen abbreviations`() {
        // 用户原话：「按钮缩写：拼、五、en图标，其中en右下角加26」。token 取值域由 opi-ctl
        // 侧钉（check_mode_shape）+ Actions.cycleModeToken 只放行这三个 ⇒ 桌面侧的字面
        // 与那三处**同改**。默认档 = 拼音（用户裁决「默认拼音」）。
        assertEquals("pinyin", MODE_DEFAULT)
        assertEquals("拼", modeFace("pinyin"))
        assertEquals("五", modeFace("wubi"))
        assertEquals("en", modeFace("en26"))
        // 认不得的值落「拼」（中性档位，不是空白按钮）；角标只有 en26 有
        assertEquals("拼", modeFace(""))
        assertEquals("拼", modeFace("english")) // 候选窗那套 token（modeLabel）不在这张表里
        // **锁步**：凡是被 `cycleModeToken` 放行的 token，都必须有**自己的面**，不许落进
        // else 的「拼」—— 那正是「引擎报 wubi、条上画拼音」这条**给用户看假档位**的坏法
        // （本文件两处警告过「别只改一处」）。取值域在 modeplumb 那边的 MODE_TOKENS
        // （private，读不到）⇒ 拿它的真出口 cycleModeToken 当域，别在这儿重抄一张会漂的表。
        // 用户 2026-10-14 裁决「先两态，五笔随 M4 接」：三行里 wubi 今天**不可达**（引擎
        // 只循环 pinyin/en26），但它已在域里 ⇒ 面也必须已在（收到就画「五」）。
        val fallback = modeFace("not-a-mode")
        for (t in listOf("pinyin", "en26", "wubi")) {
            assertEquals("cycleModeToken 该放行「$t」（它是取值域的真出口）", t, cycleModeToken(0, t))
        }
        // ⚠️ 「pinyin」**故意**与 else 同一支（它就是这个中性档的语义，见 modeFace 的注释）
        // ⇒ 除它以外，每个放行的 token 都必须有**自己的面**：落进 else 就是把别的档画成
        // 「拼」= 给用户看假档位（起草时把 pinyin 也圈进来了，红在它自己身上）。
        for (t in listOf("en26", "wubi")) {
            assertNotEquals("放行的 token「$t」没有自己的面：落进 else 的「$fallback」了（面表漏一行）", fallback, modeFace(t))
        }
        assertNull(modeCorner("pinyin"))
        assertNull(modeCorner("wubi"))
        assertEquals("26", modeCorner("en26"))
        assertNull(modeCorner(""))
    }

    @Test
    fun `mode slots split the face without overlapping`() {
        // 摆法：无角标 ⇒ 单槽占满整块面；en26 ⇒ 「en」居中 + 「26」占右下，两槽**按分数
        // 就不相交**（en 的底 0.66 ≤ 角标顶 0.69）—— 这一条是**与字体无关**的保证（像素
        // 判据在 KeycapRenderTest，量的是键帽内真画出来的墨迹）。
        assertEquals(listOf("拼"), modeSlots("pinyin").map { it.text })
        assertEquals(listOf("五"), modeSlots("wubi").map { it.text })
        assertEquals(listOf("en", "26"), modeSlots("en26").map { it.text })
        val big = modeSlots("pinyin").single()
        assertEquals("无角标该占满整块面", listOf(0f, 0f, 1f, 1f), listOf(big.left, big.top, big.right, big.bottom))
        val (en, corner) = modeSlots("en26")
        assertTrue("en 槽的底 ≤ 角标槽的顶（不然两字会叠在一起）", en.bottom <= corner.top)
        assertTrue("角标槽的左边 ≤ en 槽的右边（同一条：槽必须不相交）", corner.left <= en.right)
        assertTrue("角标该在右下角", corner.left >= 0.5f && corner.top >= 0.5f)
        // 2026-10-20 用户裁决：en 放大到与其他键名同宽（0.10..0.74 × 0.20..0.66）；角标在右下。
        assertEquals(listOf(0.10f, 0.20f, 0.74f, 0.66f), listOf(en.left, en.top, en.right, en.bottom))
        assertEquals(listOf(0.55f, 0.69f, 0.93f, 0.94f), listOf(corner.left, corner.top, corner.right, corner.bottom))
    }

    @Test
    fun `punct slots put the period first on the diagonal`() {
        // 用户原话：「句号在按钮中左上侧，逗号在右下侧」。两枚字形各占一个**不相交**的
        // 象限槽 ⇒ 左上那枚先画、右下那枚后画；字面仍从 punctIcon 这个唯一源拆（不是在这里
        // 重抄「。，」/「.,」）。这条与字体无关（纯分数），像素判据在渲染探针那边。
        for ((st, pair) in listOf(
            ImeState(punct = true) to listOf("。", "，"),
            ImeState(punct = false) to listOf(".", ","),
        )) {
            val slots = punctSlots(st)
            assertEquals("标点该拆成两枚（中文态 $st）", pair, slots.map { it.text })
            val (first, second) = slots
            assertEquals("句号该在左上", listOf(0f, 0f, 0.5f, 0.5f), listOf(first.left, first.top, first.right, first.bottom))
            assertEquals("逗号该在右下", listOf(0.5f, 0.5f, 1f, 1f), listOf(second.left, second.top, second.right, second.bottom))
            assertTrue("两个象限不该相交", first.right <= second.left && first.bottom <= second.top)
        }
    }

    @Test
    fun `master hidden wins over every resident switch`() {
        val allOnButHidden = StatusBarSettings(hidden = true)
        assertTrue(visibleStatusButtons(allOnButHidden, windows = false).isEmpty())
    }

    @Test
    fun `each button can be turned off alone`() {
        // 覆盖 resident ⇄ with 的九对分支：任一对写错（比如 VK 那行复制成 SYMBOLS），
        // 这条就在那个按钮上红。基线取**全开**（默认里 VK 已是关，拿默认当基线的话
        // 「关 VK」是空操作，那条分支就测不到了）。
        // MODE 不在这一组里：它没有开关（关不掉、也关不着）—— 见
        // `the mode button has no switch and stays on the bar`。
        val switchable = StatusBarButton.entries.filter { it != StatusBarButton.MODE }
        val allOn = StatusBarButton.entries.fold(StatusBarSettings()) { sb, b -> sb.with(b, true) }
        for (b in switchable) {
            assertEquals(
                "关掉 $b 只该少它一个",
                StatusBarButton.entries.filter { it != b },
                visibleStatusButtons(allOn.with(b, false), windows = false),
            )
        }
    }

    @Test
    fun `turning a button back on restores it`() {
        // 可开关的按钮全关 ⇒ 条上只剩恒常驻的 MODE；再把简繁拨回来，它按枚举序
        // 出现在 MODE 前面。
        val off = StatusBarButton.entries
            .filter { it != StatusBarButton.MODE }
            .fold(StatusBarSettings()) { sb, b -> sb.with(b, false) }
        assertEquals(
            listOf(StatusBarButton.MODE),
            visibleStatusButtons(off, windows = false),
        )
        assertEquals(
            listOf(StatusBarButton.TRAD, StatusBarButton.MODE),
            visibleStatusButtons(off.with(StatusBarButton.TRAD, true), windows = false),
        )
    }

    @Test
    fun `logo click cycles every skin and comes back around`() {
        // LOGO 单击 = 换下一款皮肤（2026-10-13 用户裁决「点击 logo 的颜色变化」）。
        // 钉三件：第一步、一圈把每款都走到（**含「不改动」**，转得回条的本色）、再一步回到起点。
        // 由 entries 驱动、不复述清单：将来加款这条自己跟上（加款要同时改的六处在 SettingsTest 钉着）。
        assertEquals(SkinOption.LIGHT, nextSkin(SkinOption.KEEP))
        val seen = generateSequence(SkinOption.KEEP) { nextSkin(it) }.take(SkinOption.entries.size).toList()
        assertEquals("绕一圈该把每款都恰好走到一次", SkinOption.entries.toList(), seen.sorted())
        assertEquals("再走一步该回到起点", SkinOption.KEEP, nextSkin(seen.last()))
    }

    @Test
    fun `windows shows the same buttons as linux`() {
        // 2026-10-20 控制面落地后两平台动作面相同：Windows 的 trad/punct/fullwidth/
        // lang/englishpro/mode 走 ControlPlane.kt → 候选窗 broker → TSF，不再按平台过滤。
        // 显式开 VK：这条测的是「哪些上条」，别被默认值变更（VK 默认关）牵连。
        // 期望值按**枚举序**（VK 现在排 SYMBOLS 前面，见枚举头的验收订正）。
        assertEquals(
            StatusBarButton.entries.toList(),
            visibleStatusButtons(StatusBarSettings(vk = true), windows = true),
        )
        assertEquals(
            StatusBarButton.entries.toList(),
            visibleStatusButtons(StatusBarSettings(vk = true), windows = false),
        )
        // 设置窗的开关行：两平台都是除 MODE 外的 9 枚（MODE 没有开关、不出行）。
        assertEquals(9, StatusBarButton.entries.count { statusButtonWired(it, windows = true) })
        assertEquals(9, StatusBarButton.entries.count { statusButtonWired(it, windows = false) })
    }

    @Test
    fun `command mapping is the frozen opi-ctl contract`() {
        // 契约：`opi-ctl <action>`，动作名 2026-10-10 冻结（另一条轨实现）。
        // 改这里的字符串 = 状态栏点了没反应（进程起来了、退出码 0、什么都没发生）：
        // 这是跨轨契约，本仓惯例是「钉死在测试里」。四个动作**逐条钉字面**。
        assertEquals(listOf("opi-ctl", "trad"), statusActionCommand(StatusBarButton.TRAD))
        assertEquals(listOf("opi-ctl", "punct"), statusActionCommand(StatusBarButton.PUNCT))
        assertEquals(listOf("opi-ctl", "fullwidth"), statusActionCommand(StatusBarButton.FULLWIDTH))
        // ⚠️ 事故留痕（2026-10-10 中午用户报障）：LANG 曾是 fcitx5-remote -t ——
        // `-t` 切的是**整个 fcitx5 的启用态**，不是中英模式：用户点两下 ⇒ 全框架被
        // 停用、中文一个字打不出来（lead 真机实测指纹：点两次后 `fcitx5-remote` 报 2）。
        // 中英切换的正确动作是契约里的 `opi-ctl lang`。改回 -t 之前先读完这段。
        assertEquals(listOf("opi-ctl", "lang"), statusActionCommand(StatusBarButton.LANG))
        // 这四个是进程内动作（开键盘窗 / 开符号页 / 开设置窗），不出命令。
        // SYMBOLS 2026-10-12 从「跑 opi-ctl symbols」改到这一档：用户裁决「点击后要是
        // 个大的页面，可以翻页选择」（两端同款符号页窗，见 StatusBar.openSymbols）。
        assertNull(statusActionCommand(StatusBarButton.VK))
        assertNull(statusActionCommand(StatusBarButton.SYMBOLS))
        assertNull(statusActionCommand(StatusBarButton.LOGO))
        assertNull(statusActionCommand(StatusBarButton.SETTINGS))
        // MODE 也不出命令：它走 Actions.cycleMode（`opi-ctl mode` 由那条路发、应答是新档位
        // token），不是这张「按钮 → 命令行」的表 —— 它若漏在这里被 else 兜住，就会出现
        // 「点了模式按钮跑了条别的命令」。
        assertNull(statusActionCommand(StatusBarButton.MODE))
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
        // fullwidth 位（满月 = 全角、半月 = 半角，见 BarGlyphs.kt 的 MoonIcon —— 2026-10-14
        // 从 StatusBar.kt 搬到那儿，几何改成按面取最大）。
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
    fun `the bar follows the mode on the state line`() {
        // 2026-10-14；模式按钮的档位**读 state 行**（`mode=` 位，modeplumb 落地）——
        // 此前条只记得「自己点出来的」档，重启/外部切档后就画错。四态：
        //   缺键（老版本插件）⇒ 回落默认档；有键 ⇒ 用它；认不得 ⇒ 解析层筛掉 ⇒ 回落。
        // 回落这一步在 barMode 里（不在解析层），所以下面对照的两侧都断言：
        // 解析层**不**代我回落（`""` 就交出来），画到条上才是「拼」。
        assertEquals("缺 mode 键 ⇒ 回落默认档", MODE_DEFAULT, barMode(ImeState()))
        assertEquals("默认档的字面值 = 用户裁决的「拼音」", "pinyin", MODE_DEFAULT)
        assertEquals(
            "整行读不到（读失败 = 中性 ImeState）也回落",
            MODE_DEFAULT,
            barMode(parseImeState(null)),
        )
        // 有 mode 位 ⇒ 初始档就是它（端到端：整行 → 解析 → 条上画的面）
        assertEquals("en26", barMode(ImeState(mode = "en26")))
        assertEquals(
            "en26 到条上画「en」（右下的 26 角标由 modeCorner 出）",
            "en",
            modeFace(barMode(parseImeState("lang=zh;trad=off;punct=on;fw=off;im=opi;mode=en26"))),
        )
        // 跨层锁步：解析层认这个 token 与 barMode 取用它是两回事，这里一步穿到底 ——
        // 任一侧漏行（parser 不收 / 条不读）都会红（wubi 面同理，见 modeFace 的注）。
        assertEquals(
            "wubi",
            barMode(parseImeState("lang=zh;trad=off;punct=on;fw=off;im=opi;mode=wubi")),
        )
        // 认不得的值：解析层筛成 ""（**不在那边回落**，否则本条门禁没牙）⇒ 条上回落。
        assertEquals("解析层不许代为回落（域外值原样交出来才算筛干净）", "", parseImeState("mode=ok").mode)
        assertEquals("域外值 ⇒ 回落默认档", MODE_DEFAULT, barMode(parseImeState("lang=zh;trad=off;punct=on;fw=off;im=opi;mode=ok")))
        // 繁体态（2026-10-14 lead **改判**后的口径）：档位面只有三档，表达不了繁体 ⇒
        // 服务端在繁体态报 `mode=pinyin`；旧口径是把未定义态收进 en26，那让打繁体的人
        // 看见「en26」＝假标签（与 modeFace 那条 wubi 防假标签同一类错），故改判。
        // 繁体仍由自己的 TRAD 位表达 ⇒ 「拿 mode 反推简繁」恒假，别哪天「顺手」去推。
        val tradLine = parseImeState("lang=zh;trad=on;punct=on;fw=off;im=opi;mode=pinyin")
        assertEquals("繁体态报拼音档（旧的 en26 形态已改判，别改回去）", "pinyin", barMode(tradLine))
        assertEquals("繁体态条上画「拼」，不是「en26」", "拼", modeFace(barMode(tradLine)))
        assertTrue("繁体由 TRAD 那一枚表达，与 mode 各管各", tradLine.trad)
        assertEquals("繁体态 TRAD 按钮照旧出「繁」", "繁", iconText(StatusBarButton.TRAD, tradLine))
        // 反推恒假的机制面：拼音态与繁体态的 `mode` 位**逐字相同**，只有 trad 位分得开
        // ⇒ mode 这一位里没有「是不是繁体」这个信息（上面那两条断言说的是看到的效果）。
        assertEquals(
            "mode 位反推不出简繁（两种行 mode 一样）",
            parseImeState("lang=zh;trad=off;punct=on;fw=off;im=opi;mode=pinyin").mode,
            tradLine.mode,
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
        assertFalse(barVisible(buttons, im = "", requireActive = true)) // Windows 未连接 ⇒ 隐藏
        assertTrue(barVisible(buttons, im = "opi", requireActive = true)); assertFalse(barVisible(buttons, im = "pinyin", requireActive = true))
        assertFalse(barVisible(buttons, parseImeState("lang=zh;trad=off;punct=on;fw=off;im=pinyin").im)) // 整行→解析→判定
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
