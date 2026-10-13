// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **跨源锁步门禁**（2026-10-14 从 `SettingsTest.kt` 拆出：加拼音方式后那个文件
//! 558 行撞 500 行门禁）。主题统一：本文件的每条用例都**从别的源文件现读现比**
//! （引擎 Rust 源、fcitx5 的 C++ 配置头、生成脚本、CMake 安装清单、Android 调色板），
//! 钉的是「同一份清单在几个语言里各写一份」这类**不会报错的漂移**—— 一端改了另一
//! 端没跟，表现是选了这个、存成那个，静默。
//!
//! 拆的界：**需要 repoFile 的用例全在这**（那是它们的共用工具）；纯 ini 往返/容错的
//! 用例留在 `SettingsTest.kt`。测试工作目录 = `desktop/`（同 SymbolCategoriesTest）。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.nio.file.Files
import java.nio.file.Paths

class SettingsLockstepTest {

    @Test
    fun `skin literal values match the cpp enum`() {
        // 与 opi_config.h 的 FCITX_CONFIG_ENUM_NAME_WITH_I18N 逐字对应：两侧按
        // **字面值**匹配（C++ 的 unmarshallOption），写错一个字 = 静默回「不改动」。
        // 顺序也钉：opi_skin.h 的 opiSkinThemeName 与主题目录按同一顺序对应。
        assertEquals(
            listOf("不改动", "浅色", "深色", "宣纸", "朱砂", "清爽", "湛蓝", "柠檬", "晚霞", "星光", "紫藤", "松林"),
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

        assertEquals("十二款（含不改动）", 12, desktop.size)
        assertEquals("Android 的中文名与桌面同集合同序", desktop, androidLabels)
        assertEquals("fcitx5 的 i18n 名与桌面同集合同序", desktop, cppNames)
        // 英文 id 一侧：「不改动」不映射主题（Keep → nullptr），故三份目录清单都是十一款
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
        assertEquals("十一款支路全在（不改动 = KeepColors，不参与本比对）", names, blocks.keys.toList())

        fun role(block: String, key: String): String? =
            Regex("""$key = Color\(0x([0-9A-Fa-f]{8})\)""").find(block)?.groupValues?.get(1)

        fun field(name: String) = captures(Regex(""""$name": "#([0-9A-Fa-f]{6,8})""""), gen)
        val bg = field("bg")
        val text = field("text")
        val hl = field("hl_bg")
        val sep = field("sep")
        val accent = field("accent")
        assertEquals("生成器十一款字段齐", 11, bg.size)
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

    // ---------- 模糊音逐对（2026-10-13 用户裁决「可多选类似 z=zh」）----------

    /** 名字表/顺序是跨语言契约：从引擎源**现读现比**（复述会跟着漂）。 */
    @Test
    fun `fuzzy pairs stay in lockstep with the engine`() {
        val src = repoFile("crates/engine-core/src/fuzzy.rs")
        val block = src.substringAfter("pub const FUZZY_PAIRS").substringBefore("];")
        // 源里是 `("zh", "z"),` 两个字符串一组：两组分别抽，配对重建。
        val a = captures(Regex("\"([a-z]+)\", \"[a-z]+\""), block)
        val b = captures(Regex("\"[a-z]+\", \"([a-z]+)\""), block)
        val pairs = a.zip(b).map { (x, y) -> "$x/$y" }
        assertEquals(
            listOf("zh/z", "ch/c", "sh/s", "an/ang", "en/eng", "in/ing", "n/l", "f/h"),
            pairs,
        )
        // 桌面侧的名字表必须与引擎同序同名（位序 = 掩码位序）。
        assertEquals(pairs, FUZZY_PAIR_NAMES)
    }

    @Test
    fun `fuzzy pairs round trip and fall back to the legacy switch`() {
        // 非空列表：写得出、读得回（管理键）。
        val s = OpSettings(fuzzyPairs = "zh/z,n/l", fuzzy = true)
        assertTrue(s.toIni().contains("FuzzyPairs=zh/z,n/l\n"))
        assertEquals("zh/z,n/l", OpSettings.fromIni(s.toIni()).fuzzyPairs)
        // 缺键 = 空 = 未设置：生效集回落到旧布尔（全开/全关）。
        assertEquals("", OpSettings.fromIni("Fuzzy=True\n").fuzzyPairs)
        assertEquals(FUZZY_PAIR_NAMES.toSet(), OpSettings(fuzzy = true).fuzzyPairSet())
        assertEquals(emptySet<String>(), OpSettings(fuzzy = false).fuzzyPairSet())
        // 首次勾选把旧布尔态**具体化**成列表；再勾回来。
        val all = OpSettings(fuzzy = true)
        val t1 = all.toggleFuzzyPair("n/l")
        assertEquals("取消 n/l 后应剩七组", "zh/z,ch/c,sh/s,an/ang,en/eng,in/ing,f/h", t1.fuzzyPairs)
        val t2 = t1.toggleFuzzyPair("n/l")
        assertEquals("再勾回 = 全列表", FUZZY_PAIR_NAMES.joinToString(","), t2.fuzzyPairs)
        // 全不勾 = 空列表 + fuzzy=false（仍是「全关」，读侧一致）。
        var off = OpSettings(fuzzyPairs = FUZZY_PAIR_NAMES.joinToString(","), fuzzy = true)
        for (n in FUZZY_PAIR_NAMES) off = off.toggleFuzzyPair(n)
        assertEquals("", off.fuzzyPairs)
        assertFalse(off.fuzzy)
        assertEquals(emptySet<String>(), off.fuzzyPairSet())
        // merge：管理键（既有的 FuzzyPairs 行被新值顶掉）。
        val merged = mergeIni("FuzzyPairs=zh/z\n", OpSettings(fuzzyPairs = "n/l"))
        assertTrue(merged, merged.contains("FuzzyPairs=n/l\n"))
        assertTrue(merged, !merged.contains("FuzzyPairs=zh/z\n"))
    }

    // ---------- 双拼布局（2026-10-13 用户裁决「搜狗同款全套」）----------

    /**
     * 名字表/顺序是跨语言契约（序 = FFI 下标）：从**生成表**现读现比。
     * 生成物是 `scripts/gen_shuangpin.py` 的产物，手改它会被 `ShuangpinTables` 的门禁
     * 抓，所以这里只需比对名字与表序。
     */
    @Test
    fun `shuangpin layout names stay in lockstep with the engine`() {
        val src = repoFile("crates/engine-core/src/shuangpin_tables.rs")
        val names = captures(Regex("name: \"([^\"]+)\""), src)
        assertEquals(
            listOf("自然码", "微软", "小鹤", "智能ABC", "紫光", "拼音加加"),
            names,
        )
        assertEquals("桌面侧名字表必须与引擎同序同名（序 = FFI 下标）", names, SHUANGPIN_LAYOUT_NAMES)

        // fcitx5：C++ 的枚举名与 i18n 字面值（= 配置文件里存的值、也是
        // `static_cast<int>` 传进引擎的**顺序**来源）。枚举序 ⇄ i18n 宏按位置配对，
        // 故两处都现读现比 —— 漂开 = 「选微软、存成小鹤」这种错名落盘，不报错。
        val config = repoFile("crates/fcitx5-opi/cpp/opi_config.h")
        val i18nStart = config.indexOf("FCITX_CONFIG_ENUM_NAME_WITH_I18N(OpiShuangpinLayout,")
        assertTrue("opi_config.h 里找不到双拼布局的 i18n 名清单（改名了？）", i18nStart >= 0)
        val cppNames = captures(
            Regex("""N_\("([^"]+)"\)"""),
            config.substring(i18nStart, config.indexOf(");", i18nStart)),
        )
        assertEquals("fcitx5 的 i18n 名（配置字面值）必须与引擎同序同名", names, cppNames)
        val enumStart = config.indexOf("{", config.indexOf("enum class OpiShuangpinLayout"))
        assertTrue("opi_config.h 里找不到 OpiShuangpinLayout 枚举声明（改名了？）", enumStart >= 0)
        val cppEnum = captures(Regex("""(\w+)"""), config.substring(enumStart + 1, config.indexOf("}", enumStart)))
        assertEquals(
            "枚举值数与 i18n 名数必须相等（宏按位置配对，少一个名不是报错是错配）",
            cppNames.size,
            cppEnum.size,
        )
    }

    /**
     * 拼音方式（2026-10-14）：名字的真源是引擎 `Scheme::name`，桌面只列可达的两项
     * （全拼/双拼）；fcitx5 的 i18n 字面值 + **枚举取值**（= 方案整数码）也一并钉。
     */
    @Test
    fun `pinyin method names stay in lockstep across all ends`() {
        val src = repoFile("crates/engine-core/src/scheme.rs")
        // `Scheme::Full => "全拼",` 那一组（name() 的臂）；code() 的臂是无引号的数字。
        // ⚠️ 用**转义串**而不是原生串：原生串结尾 `","""` 的引号与终止符毗邻，实测 Kotlin
        // 解析出的正则比我意图的多一个 `"`（零命中、静默红）—— 别在原地数引号。
        val rustNames = Regex("Scheme::(\\w+) => \"([^\"]+)\",")
            .findAll(src).map { it.groupValues[2] }.toList()
        assertTrue("scheme.rs 里一条名字都没解析出来（源码格式变了？）", rustNames.isNotEmpty())
        assertEquals(
            "引擎方案名字表（顺序 = 枚举声明序）",
            listOf("全拼", "拼音9键", "拼音14键", "笔画", "双拼"),
            rustNames,
        )
        // 桌面可达的子集 = 全拼 + 双拼（9/14 键/笔画是移动端方案，物理键盘上没有意义）。
        assertEquals("桌面名字表 = 引擎名字表的可达子集（同序）", listOf("全拼", "双拼"), SCHEME_NAMES)
        assertEquals("SCHEME_NAMES 首项 = 缺省（全拼）", "全拼", OpSettings().pinyinMethod)

        val config = repoFile("crates/fcitx5-opi/cpp/opi_config.h")
        val i18nStart = config.indexOf("FCITX_CONFIG_ENUM_NAME_WITH_I18N(OpiPinyinMethod,")
        assertTrue("opi_config.h 里找不到拼音方式的 i18n 名清单（改名了？）", i18nStart >= 0)
        val cppNames = captures(
            Regex("""N_\("([^"]+)"\)"""),
            config.substring(i18nStart, config.indexOf(");", i18nStart)),
        )
        assertEquals("fcitx5 的 i18n 名（配置字面值）必须与桌面同集合同序", SCHEME_NAMES, cppNames)
        // 枚举值 = 方案整数码（0=全拼 4=双拼，即 Scheme::code）—— 非连续取值是刻意的，
        // 中间没有第二张映射表；漂了 = 选了双拼而引擎收到别的码。
        val enumStart = config.indexOf("{", config.indexOf("enum class OpiPinyinMethod"))
        val pairs = Regex("""(\w+) = (\d+)""")
            .findAll(config.substring(enumStart + 1, config.indexOf("}", enumStart)))
            .map { it.groupValues[1] to it.groupValues[2] }.toList()
        assertEquals(listOf("Quanpin" to "0", "Shuangpin" to "4"), pairs)
    }

    @Test
    fun `pinyin method round trips and falls back to quanpin`() {
        val s = OpSettings(pinyinMethod = "双拼")
        assertTrue(s.toIni().contains("PinyinMethod=双拼\n"))
        assertEquals("双拼", OpSettings.fromIni(s.toIni()).pinyinMethod)
        // 缺键/认不得 → 全拼（引擎既有行为）。
        assertEquals("全拼", OpSettings.fromIni("Learn=True\n").pinyinMethod)
        assertEquals("全拼", OpSettings.fromIni("PinyinMethod=五笔\n").pinyinMethod)
        val merged = mergeIni("PinyinMethod=双拼\n", OpSettings(pinyinMethod = "全拼"))
        assertTrue(merged, merged.contains("PinyinMethod=全拼\n"))
        assertTrue(merged, !merged.contains("PinyinMethod=双拼\n"))
    }
}
