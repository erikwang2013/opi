// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「属性设置」的配置 IO 门禁（纯 JVM，无 UI、无 JNA）。
//!
//! 盯的是两处**静默失效**的地方：键名（三处逐字对应的契约）与落点路径
//! （「本窗写哪个文件」⇄「IME 侧读哪个文件」的接缝）。

package xyz.erik.opi.candidate

import java.nio.file.Files
import java.nio.file.Paths
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SettingsTest {

    @Test
    fun `round trip keeps every switch`() {
        val s = OpSettings(
            learn = false, chinesePunct = true, fuzzy = false, jianpin = true, shiftToggle = false,
        )
        assertEquals(s, OpSettings.fromIni(s.toIni()))
    }

    @Test
    fun `defaults are all on`() {
        // 默认值 = 产品缺省（2026-10-15 用户裁决「默认关闭模糊音、开启拼音纠错、简拼」；
        // 其余仍全开）。空文件/没配过 ⇒ 取这组缺省（不是全关）。
        assertEquals(OpSettings(), OpSettings.fromIni(""))
        assertEquals(OpSettings(), OpSettings.fromIni("# 只有注释\n\n"))
    }

    @Test
    fun `key names are pinned`() {
        // 键名是**三处逐字对应**的契约（本文件 ⇄ opi_config.h ⇄ tsf settings.rs）。
        // 钉整行而不是 contains("Learn")：改名/改大小写/改空格都该在这里红。
        val ini = OpSettings().toIni()
        for (line in listOf(
            // 模糊音缺省**关**（2026-10-15 用户裁决）：整行契约跟着翻。
            "Learn=True", "ChinesePunct=True", "Fuzzy=False", "Jianpin=True",
            "ShiftToggle=True", "Skin=不改动", "CandidateFont=中",
        )) {
            assertTrue("缺少键行：$line\n$ini", ini.contains("$line\n"))
        }
    }

    @Test
    fun `bool spelling is True or False`() {
        val ini = OpSettings(fuzzy = false).toIni()
        assertTrue(ini.contains("Fuzzy=False\n"))
        assertFalse(ini.contains("Fuzzy=true")) // 全小写不写出去（读侧认，但写侧统一）
    }

    @Test
    fun `merge keeps unknown keys and comments`() {
        // 2026-10-09 起保存走 mergeIni：本窗**不管**的键与手写注释**不能被本窗抹掉**
        // —— 此前是整文件重写，抹一次就是「设置窗保存后那个选项静默失效」。
        // ⚠️ 例子用 `DisplayName`（fcitx5 配置面的字段，本窗不管理）。**别再拿
        // ShiftToggle/Skin 举例**：两个都已是管理键（2026-10-12 起 ShiftToggle 有开关行；
        // 详见 `shift toggle round-trips`）—— 拿管理键当「未知键」的样本，这条用例
        // 测的就是另一回事了。
        val existing = """
            # 手写的注释
            Learn=True
            DisplayName=OPI
            # 保底
        """.trimIndent() + "\n"
        val merged = mergeIni(existing, OpSettings(learn = false))
        // JUnit4 的两个参数是 (message, condition) —— 与 Kotlin 测试框架的习惯相反，
        // 写反了是**编译错**（Boolean 塞不进 String 位），实测抓过。
        assertTrue(merged, merged.contains("# 手写的注释\n"))
        assertTrue(merged, merged.contains("DisplayName=OPI\n"))
        assertTrue(merged, merged.contains("Learn=False\n"))
        assertFalse(merged, merged.contains("Learn=True"))
        // 没出现过的管理键要补齐（旧文件里缺键 = 用默认值写回去）
        assertTrue(merged, merged.contains("ChinesePunct=True\n"))
        assertTrue(merged, merged.contains("Skin=不改动\n"))
        assertTrue(merged, merged.contains("ShiftToggle=True\n"))
    }

    @Test
    fun `merge rewrites skin and preserves other unknowns`() {
        val existing = "# 手写的注释\nSkin=深色\nDisplayName=OPI\n"
        // 真流程是「先 loadSettings（把 Skin=深色 读进来）再保存」——这里模拟的
        // 正是那一步：读到的值原样写回，别的键一个不动。
        val loaded = OpSettings.fromIni(existing)
        assertEquals(SkinOption.DARK, loaded.skin)
        val merged = mergeIni(existing, loaded.copy(learn = false))
        assertTrue(merged, merged.contains("Skin=深色\n"))
        assertTrue(merged, merged.contains("DisplayName=OPI\n"))
        assertTrue(merged, merged.contains("Learn=False\n"))
        assertEquals(loaded.copy(learn = false), OpSettings.fromIni(merged))
    }

    @Test
    fun `unknown skin falls back to keep`() {
        // 手改坏/未来版本的键值 ⇒ 默认「不改动」（与 bool 坏值同一档，不抛）。
        assertEquals(SkinOption.KEEP, OpSettings.fromIni("Skin=彩虹\n").skin)
        assertEquals(SkinOption.KEEP, OpSettings.fromIni("").skin)
    }

    @Test
    fun `merge output round trips through the reader`() {
        // 行内注释、未知键、行内空格都掺上：合并结果必须仍被 fromIni 正确解析。
        val existing = "Learn=True\nFuzzy=False # 关掉\n未知键=whatever\n"
        val s = OpSettings(learn = false, chinesePunct = false, fuzzy = true, jianpin = true)
        val merged = mergeIni(existing, s)
        assertEquals(s, OpSettings.fromIni(merged))
        assertTrue(merged, merged.contains("未知键=whatever\n"))
    }

    @Test
    fun `tolerant reader`() {
        // 手改的配置：小写值、行内注释、未知键、多余空格 —— 都该认/该忽略，不该抛。
        val s = OpSettings.fromIni(
            """
            # 手改的
            Fuzzy = false   # 关模糊音
            Jianpin=OFF
            Learn=1
            未来才有的键=whatever
            """.trimIndent(),
        )
        assertEquals(false, s.fuzzy)
        assertEquals(false, s.jianpin)
        assertEquals(true, s.learn)
        assertEquals(true, s.chinesePunct) // 缺键 ⇒ 默认开
    }

    @Test
    fun `garbage value falls back to default`() {
        val s = OpSettings.fromIni("Fuzzy=maybe\nJianpin=\n")
        // 坏值/空值 ⇒ 回落到**缺省**。模糊音的缺省 2026-10-15 起是 false（用户裁决）。
        assertFalse(s.fuzzy)
        assertTrue(s.jianpin)
    }

    @Test
    fun `linux path is the fcitx5 addon config`() {
        // 对侧读侧 = opi_config.h 的 readAsIni(PkgConfig, "conf/opi_fcitx5.conf")。
        // 路径错 = 设置窗写了个 fcitx5 不读的文件（静默不生效），故钉死。
        assertEquals(
            "/x/cfg/fcitx5/conf/opi_fcitx5.conf",
            settingsPathFor(windows = false, xdgConfigHome = "/x/cfg", home = "/home/u", localAppData = null).toString(),
        )
        // XDG_CONFIG_HOME 缺省 ⇒ $HOME/.config
        assertEquals(
            "/home/u/.config/fcitx5/conf/opi_fcitx5.conf",
            settingsPathFor(windows = false, xdgConfigHome = null, home = "/home/u", localAppData = null).toString(),
        )
        // 空串等同未设置（与 Rust 侧 dict_path/data_dir 的 takeIf { isNotEmpty() } 同口径）
        assertEquals(
            "/home/u/.config/fcitx5/conf/opi_fcitx5.conf",
            settingsPathFor(windows = false, xdgConfigHome = "", home = "/home/u", localAppData = null).toString(),
        )
    }

    @Test
    fun `windows path is localappdata opi`() {
        // 对侧读侧 = crates/tsf-opi/src/settings.rs（与 dict_path.rs 的候选目录同一处）。
        //
        // ⚠️ 断言的是**结构**（基目录 + opi/settings.ini 两段），不是那个反斜杠字面串：
        // `Paths.get` 按**运行平台**的分隔符拼路径 —— 在 Linux 上跑这条测试时，
        // Windows 字面串永远不会出现（实测：首版就是这么红的，errorFileCount=1）。
        // 真正的 Windows 字面串只能在 Windows 上验；这里钉的是「拼了哪两段、基目录是谁」。
        val p = settingsPathFor(
            windows = true, xdgConfigHome = null, home = null,
            localAppData = "C:\\Users\\u\\AppData\\Local",
        )
        // 只用 Path 自己的 API（`takeLast` 那条扩展在 JDK11 目标下解析不到，实测红）
        assertEquals("settings.ini", p.fileName.toString())
        assertEquals("opi", p.parent.fileName.toString())
        assertEquals("C:\\Users\\u\\AppData\\Local", p.parent.parent.toString())
    }

    // ---------- 状态栏九个键（2026-10-10 加）----------

    @Test
    fun `statusbar round trip keeps every switch`() {
        val s = OpSettings(
            statusbar = StatusBarSettings(hidden = true, vk = false, symbols = false, fullwidth = false),
        )
        assertEquals(s, OpSettings.fromIni(s.toIni()))
    }

    @Test
    fun `statusbar key names are pinned`() {
        // 与上面四个开关同一条纪律：**钉整行**。这九个键只有本窗读写（两端 IME 的读侧
        // 都不认识它们，见 Settings.kt 文件头），所以没有第二处能替这里发现改名。
        val ini = OpSettings(
            statusbar = StatusBarSettings(hidden = true, vk = false, trad = false),
        ).toIni()
        for (line in listOf(
            "StatusBarHidden=True", "StatusBarVk=False", "StatusBarSymbols=True",
            "StatusBarTrad=False", "StatusBarPunct=True", "StatusBarLang=True",
            "StatusBarLogo=True", "StatusBarFullwidth=True", "StatusBarSettings=True",
        )) {
            assertTrue("缺少状态栏键行：$line\n$ini", ini.contains("$line\n"))
        }
    }

    @Test
    fun `config without statusbar keys gets the decided defaults`() {
        // 1.7.5 及更早写的配置文件没有这九个键 ⇒ 默认「不隐藏 + 除软键盘外全常驻」
        // （2026-10-10 中午裁决收窄：软键盘默认关），而不是全关 —— 全关会让老用户
        // 升级后条上什么都不剩。比对**整个** StatusBarSettings()：fromIni 的回落值
        // 与数据类默认漂了一处就会红在这里。
        val s = OpSettings.fromIni("Learn=False\nFuzzy=True\n")
        assertEquals(StatusBarSettings(), s.statusbar)
        assertFalse("软键盘缺键时必须回落到关", s.statusbar.vk)
        assertFalse(s.learn) // 老键照旧生效
    }

    @Test
    fun `merge overwrites every statusbar key`() {
        // 逐个键都真的被写成新值：漏一个进 managed 表 = 那个开关**拨了不动**
        // （保存成功、提示正常、值没变），是本模块最难自查的一类错。
        val allOff = StatusBarButton.entries.fold(StatusBarSettings(hidden = true)) { sb, b ->
            sb.with(b, false)
        }
        val existing = OpSettings(statusbar = allOff).toIni()
        val merged = mergeIni(existing, OpSettings())
        assertEquals(OpSettings(), OpSettings.fromIni(merged)) // 读回来必须等于新值
        // 旧值不能残留。这里**不能**写 contains("=False") 之类：False 本身是合法值
        // （新值里就有 StatusBarHidden=False），那种断言恒真、什么都证明不了 ——
        // 首版就是这么写的，一条真断言都没拦住（被自己的新值满足）。
        // 探测键必须在「旧值 ≠ 新值」的键里挑，且写得进 merged 才算数：
        // · StatusBarTrad：旧 False → 新 True；
        // （StatusBarVk 曾在这儿当过探测器 —— 2026-10-10 中午默认翻成 False 后两边
        //   同值，它就退化成恒真了，按上面的规矩换掉。）
        assertFalse(merged, merged.contains("StatusBarTrad=False\n"))
        assertFalse(merged, merged.contains("StatusBarHidden=True\n"))
    }

    @Test
    fun `merge still keeps unknown keys next to the statusbar block`() {
        // 旧值写 True（新默认是 False，2026-10-10 中午翻的）——「旧值被换成新值」这件
        // 事才看得出来；两边同值时断言会退化成恒真（与上一条测试同一份教训）。
        // ⚠️ 未知键的样本用 `DisplayName`（不再用 ShiftToggle：2026-10-12 起它是管理键，
        // 再拿它当样本这条会测成「管理键被重写」，与用例名不是一回事）。
        val merged = mergeIni("# 手写\nDisplayName=OPI\nStatusBarVk=True\n", OpSettings())
        assertTrue(merged, merged.contains("DisplayName=OPI\n"))
        assertTrue(merged, merged.contains("# 手写\n"))
        assertTrue(merged, merged.contains("StatusBarVk=False\n"))
        assertFalse(merged, merged.contains("StatusBarVk=True\n"))
    }

    @Test
    fun `merge does not grow the file on repeated saves`() {
        // 2026-10-14（写候选字号的 merge 时被同形门禁抓到的既有毛病）：文件以 \n 结尾时
        // 按行切会多出一个空段，把它当独立行拼回去 ⇒ 每保存一次文件末尾多一行空行。
        // 真机 ~/.config/fcitx5/conf/opi_fcitx5.conf 末尾就躺着一行这样的空行。
        // 判据 = 幂等（保存两次与一次同形）+ 末尾不许是 \n\n。
        val once = mergeIni("Learn=True\nFuzzy=True\n", OpSettings())
        assertEquals(once, mergeIni(once, OpSettings()))
        assertFalse("保存后文件末尾多出空行：\n$once", once.endsWith("\n\n"))
        // 空行在**中间**照旧逐字保留（只有「表示结尾」的那一个空段被吸收）。
        // ⚠️ 夹具里用 `Fuzzy=False`（= 新缺省，2026-10-15 裁决）—— 合并会把 Fuzzy 那行
        // 改写成设置值，用 True 的话这条会在「值被改写」上红，而它要量的是**空行保留**。
        val kept = mergeIni("Learn=True\n\nFuzzy=False\n", OpSettings())
        assertTrue(kept, kept.contains("Learn=True\n\nFuzzy=False\n"))
    }

    @Test
    fun `tabs follow the platform's real knobs`() {
        // 2026-10-12（用户裁决「布局和功能参考主流输入法」）：左导航的页集合是纯函数，
        // 按平台过滤 —— 少了这个判据，Windows 上会出两页点不动的开关。
        assertEquals(
            listOf(
                SettingsTab.COMMON, SettingsTab.KEYS, SettingsTab.APPEARANCE,
                SettingsTab.STATUSBAR, SettingsTab.SYMBOLS, SettingsTab.DICTS,
                // 五笔页（2026-10-14，M4 第二波）：两平台都出 —— 六键两端读侧都认。
                SettingsTab.WUBI,
                SettingsTab.ABOUT,
            ),
            settingsTabs(windows = false),
        )
        // Windows：按键（ShiftToggle 只有 Linux 读侧认）与外观（Skin 是 fcitx5 主题）不出；
        // 词库页两平台都出（目录推导分平台，页本身同构 —— 见 Dicts.kt）。
        assertEquals(
            listOf(
                SettingsTab.COMMON, SettingsTab.STATUSBAR, SettingsTab.SYMBOLS,
                SettingsTab.DICTS, SettingsTab.WUBI, SettingsTab.ABOUT,
            ),
            settingsTabs(windows = true),
        )
    }

    @Test
    fun `shift toggle round-trips and is a managed key`() {
        // 2026-10-12 加：Linux 读侧（opi_config.h 的 ShiftToggle）本来就在，本窗此前
        // 只是「不认识」它。三条：写得出、读得回、保存时**跟着内存值走**（管理键语义）。
        assertTrue(OpSettings(shiftToggle = false).toIni().contains("ShiftToggle=False\n"))
        assertEquals(false, OpSettings.fromIni("ShiftToggle=False\n").shiftToggle)
        assertEquals(true, OpSettings.fromIni("ShiftToggle=yes\n").shiftToggle) // 宽容读
        val merged = mergeIni("ShiftToggle=True\n", OpSettings(shiftToggle = false))
        assertTrue(merged, merged.contains("ShiftToggle=False\n"))
        assertFalse(merged, merged.contains("ShiftToggle=True\n"))
        // 缺键 → 默认 true（引擎既有行为：Shift 轻按本来就开着）
        assertEquals(OpSettings(), OpSettings.fromIni("Learn=True\n"))
    }

    @Test
    fun `shuangpin layout round trips and falls back to the first entry`() {
        val s = OpSettings(shuangpinLayout = "小鹤")
        assertTrue(s.toIni().contains("ShuangpinLayout=小鹤\n"))
        assertEquals("小鹤", OpSettings.fromIni(s.toIni()).shuangpinLayout)
        // 缺键 → 表首（自然码）；认不得的（手改坏的）也回落表首，不抛。
        assertEquals(SHUANGPIN_LAYOUT_NAMES.first(), OpSettings.fromIni("Learn=True\n").shuangpinLayout)
        assertEquals(SHUANGPIN_LAYOUT_NAMES.first(), OpSettings.fromIni("ShuangpinLayout=Xyzzy\n").shuangpinLayout)
        // merge：管理键（既有的行被新值顶掉）。
        val merged = mergeIni("ShuangpinLayout=微软\n", OpSettings(shuangpinLayout = "紫光"))
        assertTrue(merged, merged.contains("ShuangpinLayout=紫光\n"))
        assertTrue(merged, !merged.contains("ShuangpinLayout=微软\n"))
    }

    // ---------- 五笔六键（2026-10-14，M4 第二波）----------

    @Test
    fun `wubi keys round-trip and are managed keys`() {
        // 六键：方案/风格/混输进入方式/候选个数/回车/自动上屏。**往返**（写得出读得回）
        // + **管理键**（mergeIni 会顶掉旧值而不是留着当未知键）。
        val s = OpSettings(
            wubi = WubiSettings(
                plan = "98",
                style = "五笔拼音",
                mixedEntry = "z键",
                candidateLimit = 7,
                returnStyle = "上屏原文",
                autoCommit = false,
            ),
        )
        val ini = s.toIni()
        for (line in listOf(
            "WubiPlan=98", "WubiStyle=五笔拼音", "WubiMixedEntry=z键",
            "WubiCandidateLimit=7", "WubiReturn=上屏原文", "WubiAutoCommit=False",
        )) {
            assertTrue("缺少键行：$line\n$ini", ini.contains("$line\n"))
        }
        assertEquals(s.wubi, OpSettings.fromIni(ini).wubi)
        // 缺键/坏值 ⇒ 回缺省（与其余名字键同一条容错）。
        assertEquals(WubiSettings(), OpSettings.fromIni("Learn=True\n").wubi)
        assertEquals(
            WubiSettings(),
            OpSettings.fromIni(
                "WubiPlan=06\nWubiStyle=打错了\nWubiMixedEntry=x\n" +
                    "WubiCandidateLimit=99\nWubiReturn=啥\nWubiAutoCommit=maybe\n",
            ).wubi,
        );
        // merge：管理键（既有行被新值顶掉，不是当未知键留着）。
        val merged = mergeIni("WubiPlan=98\nWubiStyle=五笔拼音\n", OpSettings())
        assertTrue(merged, merged.contains("WubiPlan=86\n"))
        assertTrue(merged, !merged.contains("WubiPlan=98\n"))
        // 保存一次不增行（幂等：rewriteIniLines 的既有纪律，顺带覆盖新键段）。
        assertEquals(merged, mergeIni(merged, OpSettings()))
    }
}
