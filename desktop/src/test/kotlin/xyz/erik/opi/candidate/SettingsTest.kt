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
        // 默认值 = 引擎既有行为。空文件/没配过 ⇒ 全开（不是全关！）。
        assertEquals(OpSettings(), OpSettings.fromIni(""))
        assertEquals(OpSettings(), OpSettings.fromIni("# 只有注释\n\n"))
    }

    @Test
    fun `key names are pinned`() {
        // 键名是**三处逐字对应**的契约（本文件 ⇄ opi_config.h ⇄ tsf settings.rs）。
        // 钉整行而不是 contains("Learn")：改名/改大小写/改空格都该在这里红。
        val ini = OpSettings().toIni()
        for (line in listOf(
            "Learn=True", "ChinesePunct=True", "Fuzzy=True", "Jianpin=True",
            "ShiftToggle=True", "Skin=不改动",
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
    fun `skin literal values match the cpp enum`() {
        // 与 opi_config.h 的 FCITX_CONFIG_ENUM_NAME_WITH_I18N 逐字对应：两侧按
        // **字面值**匹配（C++ 的 unmarshallOption），写错一个字 = 静默回「不改动」。
        // 顺序也钉：opi_skin.h 的 opiSkinThemeName 与主题目录按同一顺序对应。
        assertEquals(
            listOf("不改动", "浅色", "深色", "宣纸", "朱砂", "清爽", "湛蓝", "柠檬", "晚霞", "星光"),
            SkinOption.entries.map { it.value },
        )
    }

    // ---------- 三端同集同序（2026-10-10 五款新皮肤时加的回归钉）----------

    /** 仓库内文件太远就现读：desktop 的测试工作目录 = desktop/（同 SymbolCategoriesTest）。 */
    private fun repoFile(rel: String): String {
        val p = Paths.get("../$rel")
        assertTrue("源文件不在了（搬家了？）：$rel ⇒ ${p.toAbsolutePath()}", Files.exists(p))
        return Files.readString(p)
    }

    private fun captures(pattern: Regex, text: String, group: Int = 1): List<String> =
        pattern.findAll(text).map { it.groupValues[group] }.toList()

    @Test
    fun `skin names stay in lockstep across all four ends`() {
        // 皮肤名字/顺序是**跨语言契约**：Android（Palette.kt 的 label + id）、桌面（本
        // 枚举的 value + 常量名）、fcitx5（opi_config.h 的枚举序与 i18n 名 = 配置字面值、
        // opi_skin.h 的主题目录映射、CMakeLists 安装清单、生成脚本的 SKINS）。任一漂开 =
        // 一端静默换错皮肤或不认识新值 —— 都不会报错（本仓的老毛病），所以这钉从
        // **源文件现读现比**，不复述列表：复述的清单会跟着漂（老测试就是复述，正是这
        // 轮要换掉的形态）。
        val desktop = SkinOption.entries.map { it.value }

        // Android：SkinOption(id, label) —— 显式中文名
        val androidPairs = Regex("""\b([A-Z][A-Z_0-9]*)\("([a-z]+)", "([^"]+)"\)""")
            .findAll(repoFile("android/app/src/main/kotlin/xyz/erik/opi/keyboard/Palette.kt"))
            .map { it.groupValues[1] to (it.groupValues[2] to it.groupValues[3]) }.toList()
        val androidIds = androidPairs.map { it.second.first }
        val androidLabels = androidPairs.map { it.second.second }

        // fcitx5：i18n 名（=写进配置的字面值）+ 枚举名→主题目录映射
        val config = repoFile("crates/fcitx5-opi/cpp/opi_config.h")
        val i18nStart = config.indexOf("FCITX_CONFIG_ENUM_NAME_WITH_I18N(OpiSkin,")
        assertTrue("opi_config.h 里找不到 i18n 名清单（宏改名了？）", i18nStart >= 0)
        val cppNames = captures(
            Regex("""N_\("([^"]+)"\)"""),
            config.substring(i18nStart, config.indexOf(");", i18nStart)),
        )
        val cppDirs = Regex("""case OpiSkin::(\w+):\s*return "(opi-[a-z]+)"""").findAll(
            repoFile("crates/fcitx5-opi/cpp/opi_skin.h"),
        ).map { it.groupValues[1].lowercase() to it.groupValues[2] }.toList()
        // 枚举序 ⇄ i18n 宏是**按位置**配的（宏一个名对一个枚举值）：两边顺序漂开 =
        // 「选清爽、存成湛蓝」这种错名落盘，故枚举声明顺序也一并现读现比。
        val enumStart = config.indexOf("{", config.indexOf("enum class OpiSkin"))
        assertTrue("opi_config.h 里找不到 OpiSkin 枚举声明（改名了？）", enumStart >= 0)
        val cppEnum = captures(
            Regex("""(\w+)"""),
            config.substring(enumStart + 1, config.indexOf("}", enumStart)),
        ).map { it.lowercase() }
        assertEquals("先钉住枚举名单本身（多了少了都红）", listOf("keep") + androidIds.drop(1), cppEnum)

        // 主题目录：生成脚本的 SKINS 键 + CMakeLists 的安装清单
        val pyKeys = captures(Regex("""(?m)^    "([a-z]+)": \{"""), repoFile("scripts/fcitx5_skins.py"))
        val cmakeDirs = captures(
            Regex("""data/themes/(opi-[a-z]+)""""),
            repoFile("crates/fcitx5-opi/cpp/CMakeLists.txt"),
        )

        assertEquals("十款（含不改动）", 10, desktop.size)
        assertEquals("Android 的中文名与桌面同集合同序", desktop, androidLabels)
        assertEquals("fcitx5 的 i18n 名与桌面同集合同序", desktop, cppNames)
        // 英文 id 一侧：「不改动」不映射主题（Keep → nullptr），故三份目录清单都是九款
        assertEquals("Android 的 id 顺序与桌面常量名一致",
            SkinOption.entries.drop(1).map { it.name.lowercase() }, androidIds.drop(1))
        assertEquals("生成脚本的 SKINS 键与 id 同集合同序", androidIds.drop(1), pyKeys)
        assertEquals("opi_config.h 的枚举序与 i18n 名序一致（按位置配对的契约）",
            cppEnum.drop(1), androidIds.drop(1))
        assertEquals("opi_skin.h 的映射与 id 同集合同序", androidIds.drop(1), cppDirs.map { it.first })
        assertEquals("CMakeLists 的安装目录与映射一致",
            cppDirs.map { it.second }, cmakeDirs)
    }

    @Test
    fun `skin palettes match the theme generator`() {
        // 调色板（SkinPalette.kt 的 skinColors）必须逐值等于 scripts/fcitx5_skins.py
        // 的 SKINS：bg←bg、fg←text、hover←hl_bg（#RRGGBBAA → Color(0xAARRGGBB)）、
        // line←sep、accent←accent。错了就是「预览卡/状态栏一种颜色、fcitx5 另一种」，
        // 两端都不报任何错（本仓的老毛病）。
        // 2026-10-12：比对对象从 SettingsWindow.kt 的预览三色改成 SkinPalette.kt 的
        // 五角色表 —— 状态栏也吃这份色板（用户报障「设皮肤后状态栏没同步」），
        // 预览卡改为从同一份派生（SettingsWindow.previewOf 的 else 支路）。
        val palette = repoFile("desktop/src/main/kotlin/xyz/erik/opi/candidate/SkinPalette.kt")
        val gen = repoFile("scripts/fcitx5_skins.py")

        val names = SkinOption.entries.drop(1).map { it.name }
        // 每个支路块：`SkinOption.NAME -> SkinColors(` 起到该调用的收尾 `    )` 为止。
        val blocks = Regex(
            """SkinOption\.(\w+) -> SkinColors\((.*?)\n    \)""",
            RegexOption.DOT_MATCHES_ALL,
        ).findAll(palette).associate { it.groupValues[1] to it.groupValues[2] }
        assertEquals("九款支路全在（不改动 = KeepColors，不参与本比对）", names, blocks.keys.toList())

        fun role(block: String, key: String): String? =
            Regex("""$key = Color\(0x([0-9A-Fa-f]{8})\)""").find(block)?.groupValues?.get(1)

        fun field(name: String) = captures(Regex(""""$name": "#([0-9A-Fa-f]{6,8})""""), gen)
        val bg = field("bg")
        val text = field("text")
        val hl = field("hl_bg")
        val sep = field("sep")
        val accent = field("accent")
        assertEquals("生成器九款字段齐", 9, bg.size)
        assertEquals(names, captures(Regex("""(?m)^    "([a-z]+)": \{"""), gen).map { it.uppercase() })

        /** 生成器的 `#RRGGBB`（或 `#RRGGBBAA`）→ Kotlin 的 `Color(0xAARRGGBB)` 字面。 */
        fun argb(py: String): String =
            if (py.length == 6) "FF${py.uppercase()}" else (py.substring(6) + py.substring(0, 6)).uppercase()

        for ((i, name) in names.withIndex()) {
            val b = blocks.getValue(name)
            assertEquals("$name 的 bg 必须与生成器逐值相同", argb(bg[i]), role(b, "bg"))
            assertEquals("$name 的 fg（←text）", argb(text[i]), role(b, "fg"))
            assertEquals("$name 的 hover（←hl_bg，注意 alpha 搬位）", argb(hl[i]), role(b, "hover"))
            assertEquals("$name 的 line（←sep）", argb(sep[i]), role(b, "line"))
            assertEquals("$name 的 accent", argb(accent[i]), role(b, "accent"))
        }
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
        assertTrue(s.fuzzy)
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
    fun `tabs follow the platform's real knobs`() {
        // 2026-10-12（用户裁决「布局和功能参考搜狗输入法」）：左导航的页集合是纯函数，
        // 按平台过滤 —— 少了这个判据，Windows 上会出两页点不动的开关。
        assertEquals(
            listOf(
                SettingsTab.COMMON, SettingsTab.KEYS, SettingsTab.APPEARANCE,
                SettingsTab.STATUSBAR, SettingsTab.SYMBOLS, SettingsTab.ABOUT,
            ),
            settingsTabs(windows = false),
        )
        // Windows：按键（ShiftToggle 只有 Linux 读侧认）与外观（Skin 是 fcitx5 主题）不出。
        assertEquals(
            listOf(SettingsTab.COMMON, SettingsTab.STATUSBAR, SettingsTab.SYMBOLS, SettingsTab.ABOUT),
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
}
