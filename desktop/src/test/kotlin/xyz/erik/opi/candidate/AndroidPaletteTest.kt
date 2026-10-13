// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **android 色板「逐值」门禁**（2026-10-13 补）：四端同集同序那条（SettingsTest 的
//! `skin names stay in lockstep across all four ends`）只比 id/label/顺序 —— android 的
//! 12 个颜色**值**谁手改都不红；柠檬/晚霞这一轮的漂移（#FFEE7E 年代对不上 #FBEE7E）
//! 正是手工同步出来的活例。本门禁把「值」这一层钉在机制角色上。
//!
//! 判据 = 机制角色逐值 ⇄ `scripts/fcitx5_skins.py` 的 SKINS（**唯一源**）：
//!   barBg/panelBg ← bg；keyFg/panelFg ← text；accent/keyHighlightBg ← accent；
//!   keyBg/toolbarBg ← menu_bg；barBufferFg = CC+text（湛蓝 F0 是显式例外，见
//!   [bufferAlphaOverrides]）。
//!
//! ⚠️ **例外有 12 格，逐格附理由**（[deviations]）—— 它们不是漂移，是 Palette.kt 注释里
//! 写明的手挑档（「键面比底深一档」「选中态浅天蓝底」这类）。门禁对例外也有判据：例外格
//! 一旦变得与机制值相等 = 例外过时，同样红（防这张表烂成白名单）。**别默默放过**是这个
//! 文件存在的全部理由。
//!
//! **有意不钉的 3 个角色**（不是漏了，理由各自附上）：
//! · `keyHighlightFg` —— 高亮键的**字色**：SKINS 没有对应键（`hl_bg` 是半透明高亮**底**，
//!   不是键面字色），每款按对比度手挑（浅底款用白字、朱砂/松林这类用深字）；
//! · `barDimFg` —— 候选栏**副**文字（次级信息）：SKINS 也无对应键（`sep` 是分隔线色）；
//! · `toolbarFg` —— 工具条图标色：9 款里 6 款恰好 = accent（含巧合成分），
//!   lemon/sunset/wisteria 三款是手挑的加深档 —— 作为整体按**手挑角色**处理，不钉。
//!
//! **不在判据内的色板**（显式列出，别读成漏了）：
//! · FOLLOW「不改动」—— SKINS 无此键（语义就是「不指定皮肤」），android 侧与 LIGHT 同板
//!   （SkinOption.palette 的 when），无独立值可钉；
//! · light/dark —— android 侧这两个是**手机自己的观感板**（LightPalette 注释「= 2026-10-09
//!   之前的观感，逐值搬运」；DarkPalette 是「暗光模式」按 WCAG 自定），不是 SKINS 派生。
//!   它们与 SKINS 的 light/dark 部分同值属**另一条来源**，钉它 = 把两套设计绑死；
//! · ElderPalette（长辈模式）—— 不是皮肤，SKINS 无对应。
//!
//! 为什么独立成文件：SettingsTest.kt 已 407 行，本门禁（含 12 条例外注记）加进去会顶破
//! 500 行门禁（`crates/opi-ffi/tests/line_limit.rs` 全仓遍历，本目录也在内）。
//!
//! 红边有**常驻自检**（`self check` 用例）：三条红路径 —— 机制格手改、例外格过时、解析少
//! 角色 —— 各自要当场红并报出「哪款哪个角色」。自检的变异在**内存**里做，不落盘：
//! android 的 Palette.kt 是另一轨的在途改动，2026-10-13 实测写进去会被并发方几秒内盖回
//! （真文件那次正控的结论 = 自检用例第一条，已常驻化）。

package xyz.erik.opi.candidate

import java.nio.file.Files
import java.nio.file.Paths
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class AndroidPaletteTest {

    private fun repoFile(rel: String): String {
        val p = Paths.get("../$rel")
        assertTrue("源文件不在了（搬家了？）：$rel", Files.exists(p))
        return Files.readString(p)
    }

    /** 机制角色 → SKINS 键。9 款派生色板里 60/72 格成立，12 格例外见 [deviations]。 */
    private val roleSource = mapOf(
        "keyFg" to "text", "panelFg" to "text",
        "accent" to "accent", "keyHighlightBg" to "accent",
        "barBg" to "bg", "panelBg" to "bg",
        "keyBg" to "menu_bg", "toolbarBg" to "menu_bg",
    )

    /** (皮肤, 角色) → 为什么这格取的不是机制值（手挑档，理由摘自 Palette.kt 注释）。 */
    private val deviations = mapOf(
        // —— keyBg/toolbarBg：机制是 menu_bg（cinnabar/lemon/sunset/wisteria/pine 五款印证）。
        "paper.keyBg" to "键面比底略深一档（#E7DFCC）做分离",
        "paper.toolbarBg" to "工具条跟随键面同档（= paper.keyBg）",
        "fresh.keyBg" to "键面比底深一档（#CFE9DF）做分离",
        "fresh.toolbarBg" to "工具条跟随键面同档（= fresh.keyBg）",
        "azure.keyBg" to "键面比底亮一档（#3C6AA3）",
        "starlight.keyBg" to "手挑键面 #262E4A（比 menu_bg 更偏蓝，注释实测过）",
        "starlight.toolbarBg" to "工具条跟随键面同档（= starlight.keyBg）",
        // —— barBg/panelBg：机制是 bg。
        "cinnabar.barBg" to "候选栏底再压暗一档（#8A1C21）",
        "cinnabar.panelBg" to "面板底取键面档 menu_bg（#9E2228）",
        "cinnabar.toolbarBg" to "跟随候选栏档（= cinnabar.barBg）",
        "starlight.barBg" to "候选栏底再压暗一档（#161B2E）",
        // —— keyHighlightBg：机制是 accent。
        "azure.keyHighlightBg" to "选中态浅天蓝底 + 深海军蓝字（对比度 ~9:1，不必借强调色）",
    )

    /** barBufferFg 的 alpha 例外：湛蓝是深底，缓冲字要更实（手挑 F0；其余 8 款 CC）。 */
    private val bufferAlphaOverrides = mapOf("azure" to "F0")

    /** data class 的 12 个角色；解析数量要与它对上（少一个就是本门禁静默失明）。 */
    private val allRoles = listOf(
        "keyBg", "keyFg", "keyHighlightBg", "keyHighlightFg", "barBg", "barBufferFg", "barDimFg",
        "accent", "panelBg", "panelFg", "toolbarBg", "toolbarFg",
    )

    /** SKINS 解析：款数先钉死，格式一改/正则抓不到 ⇒ 当场红，不许空转变绿。 */
    private fun parseSkins(py: String): Map<String, String> {
        val skins = Regex("""(?m)^    "([a-z]+)": \{\n(.*?)^    \},""", RegexOption.DOT_MATCHES_ALL)
            .findAll(py).associate { it.groupValues[1] to it.groupValues[2] }
        assertEquals("SKINS 解析到的款数不对（格式改了？）", 11, skins.size)
        return skins
    }

    /** 色板解析：**每板的角色数**都钉死（漏解析 ⇒ 本门禁静默失明）。 */
    private fun parsePalettes(kt: String): Map<String, String> {
        val palettes = Regex("""val (\w+Palette) = KeyboardPalette\((.*?)\n\)""", RegexOption.DOT_MATCHES_ALL)
            .findAll(kt).associate { it.groupValues[1] to it.groupValues[2] }
        for ((name, block) in palettes) {
            val found = Regex("""(?m)^ {4}(\w+) = Color\(0x[0-9A-Fa-f]{8}\)""")
                .findAll(block).map { it.groupValues[1] }.toList().sorted()
            assertEquals("$name 解析到的角色数不是 12（漏解析 ⇒ 本门禁静默失明）", allRoles.sorted(), found)
        }
        return palettes
    }

    private fun skinField(skins: Map<String, String>, skin: String, key: String): String {
        val v = Regex(""""$key": "#([0-9A-Fa-f]{6})"""").find(skins.getValue(skin))
            ?.groupValues?.get(1)?.uppercase()
        assertNotNull("$skin 里解析不到 $key（格式改了？）", v)
        return v!!
    }

    private fun roleOf(palettes: Map<String, String>, palette: String, role: String): String {
        val v = Regex("""(?m)^ {4}$role = Color\(0x([0-9A-Fa-f]{8})\)""").find(palettes.getValue(palette))
            ?.groupValues?.get(1)?.uppercase()
        assertNotNull("$palette 里解析不到 $role（格式改了？）", v)
        return v!!
    }

    private fun checkAndroidPalettes(py: String, kt: String) {
        val skins = parseSkins(py)
        val palettes = parsePalettes(kt)

        // 皮肤键 → android 色板：9 款派生（注释「fcitx5 opi-XXX 同套」的）+ 豁免 light/dark。
        // 这条也是「新皮肤没进表」的钉子：SKINS 加第 12 款 ⇒ 这里先红。
        val derived = listOf(
            "paper" to "PaperPalette", "cinnabar" to "CinnabarPalette", "fresh" to "FreshPalette",
            "azure" to "AzurePalette", "lemon" to "LemonPalette", "sunset" to "SunsetPalette",
            "starlight" to "StarlightPalette", "wisteria" to "WisteriaPalette", "pine" to "PinePalette",
        )
        assertEquals(
            "SKINS 的款与本门禁的归属表不一致（新皮肤？豁免表漂了？）",
            derived.map { it.first }.toSet() + setOf("light", "dark"), skins.keys,
        )

        var checked = 0
        val usedDeviations = mutableSetOf<String>()
        for ((skin, palette) in derived) {
            for ((role, key) in roleSource) {
                val actual = roleOf(palettes, palette, role)
                val wanted = "FF" + skinField(skins, skin, key)
                checked++
                val cell = "$skin.$role"
                if (deviations.containsKey(cell)) {
                    // 例外也有判据：一旦实际值追上机制值，这条例外就是过时的白名单条目。
                    assertTrue("$cell 列为例外（${deviations.getValue(cell)}），但值已 = 机制值 —— 删掉这条例外", actual != wanted)
                    usedDeviations += cell
                } else {
                    assertEquals(
                        "$cell 必须 = SKINS($skin).$key（android 值手改不许静默过；若是有意的手挑档，逐条加进 deviations）",
                        wanted, actual,
                    )
                }
            }
            val alpha = bufferAlphaOverrides[skin] ?: "CC"
            assertEquals(
                "$skin.barBufferFg 必须 = $alpha + SKINS($skin).text（湛蓝 F0 例外见 bufferAlphaOverrides）",
                alpha + skinField(skins, skin, "text"), roleOf(palettes, palette, "barBufferFg"),
            )
        }
        assertEquals("例外表里有没生效的条目（名字写错？角色不在判据内？）", deviations.keys, usedDeviations)
        assertEquals("检查格数 = 9 款 × 8 角色", 72, checked)
    }

    @Test
    fun `android palettes pin the mechanism roles against the theme generator`() {
        checkAndroidPalettes(
            repoFile("scripts/fcitx5_skins.py"),
            repoFile("android/app/src/main/kotlin/xyz/erik/opi/keyboard/Palette.kt"),
        )
    }

    /**
     * **自检（红路径的正控，常驻）**：三条红边 —— ①机制格手改 ②例外格过时 ③解析少角色 ——
     * 每条都必须当场红，且消息里报出「哪款哪个角色」。
     *
     * 变异在**内存**里做（content.replace），不碰仓里的文件：android 的 Palette.kt 是另一轨
     * 的在途改动 —— 2026-10-13 拿真文件做正控时，写进去的值被并发方几秒内盖回（mtime 上
     * 有一次不是我发的写、sha 回到原值），既不安全也不可复现。当年那次真文件正控（fresh.keyFg
     * 拧一位 ⇒ gradle 红并报出该格）就是本用例的第 ① 条，现在常驻化。
     *
     * 取法是**发现式**的：要拧的字面从当前文件里现读（值改版了照样拧得动），只有同一字面
     * 出现次数 ≠ 1（夹具漂了，拧下去会连带别的格子）才红 —— 不许静默换目标继续绿。
     */
    @Test
    fun `self check - every red path fires and names the cell`() {
        val py = repoFile("scripts/fcitx5_skins.py")
        val kt = repoFile("android/app/src/main/kotlin/xyz/erik/opi/keyboard/Palette.kt")
        val skins = parseSkins(py)
        val palettes = parsePalettes(kt)

        fun twist(text: String, old: String, new: String): String {
            assertEquals("自检夹具漂了：`$old` 出现次数不是 1（值改版了？取法跟着改）", 1, text.split(old).size - 1)
            return text.replace(old, new)
        }

        fun expectRed(mutated: String, cell: String) {
            val e = assertThrows("$cell 的红边没响 —— 门禁在静默放行", AssertionError::class.java) {
                checkAndroidPalettes(py, mutated)
            }
            assertTrue("红是红了，但消息里没报出 `$cell`：${e.message}", e.message!!.contains(cell))
        }

        // ① 机制格拧掉一位：改 fresh 的键字色（末位 +1，进位也无妨）。锚点带行首缩进 ——
        //    同一个值在别的角色上出现（fresh.panelFg 同值）是机制使然，锚点要钉到那一行。
        val keyFg = roleOf(palettes, "FreshPalette", "keyFg")
        val bumped = "FF%06X".format((keyFg.drop(2).toInt(16) + 1) and 0xFFFFFF)
        expectRed(twist(kt, "\n    keyFg = Color(0x$keyFg),", "\n    keyFg = Color(0x$bumped),"), "fresh.keyFg")

        // ② 例外格过时：把 cinnabar.barBg 拧回 SKINS 的 bg（= 机制值）⇒ 该例外条目必须报红。
        val barBg = roleOf(palettes, "CinnabarPalette", "barBg")
        val mech = "FF" + skinField(skins, "cinnabar", "bg")
        expectRed(twist(kt, "\n    barBg = Color(0x$barBg),", "\n    barBg = Color(0x$mech),"), "cinnabar.barBg")

        // ③ 解析少一个角色：删掉宣纸的副文字行 ⇒ 数量钉死那条必须红（防「格式一改、正则
        //    抓不到、用例空转变绿」—— 这条正是它要拦的形态）。
        val dropped = "    barDimFg = Color(0x${roleOf(palettes, "PaperPalette", "barDimFg")}),\n"
        expectRed(twist(kt, dropped, ""), "PaperPalette")
    }
}
