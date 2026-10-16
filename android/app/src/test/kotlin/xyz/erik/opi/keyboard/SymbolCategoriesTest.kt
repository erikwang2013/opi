// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/** 符号页类目：解析容错 + **真资产门禁**（生成器静默产出空类目是最可能的失败形态）。 */
class SymbolCategoriesTest {

    @Test
    fun parsesAndKeepsOrderAndTokens() {
        val cats = SymbolCategories.parse(
            "1\t常用\t， 。 、\n2\t网络\t^_^ の ╮(╯▽╰)╭\n",
        )
        assertEquals(2, cats.size)
        assertEquals(SymbolCategories.Category(1, "常用", listOf("，", "。", "、")), cats[0])
        // 多字符 token 整块保留（组合序列不许被拆）
        assertEquals(listOf("^_^", "の", "╮(╯▽╰)╭"), cats[1].symbols)
    }

    @Test
    fun badLinesAreSkippedNotThrown() {
        val cats = SymbolCategories.parse(
            "1\t常用\t，\n坏行\nx\t序号非数字\t，\n3\t\t，\n4\t空符号\t \n5\t好\t。\n",
        )
        assertEquals(listOf("常用", "好"), cats.map { it.label })
    }

    /**
     * 真资产门禁：22 类目全在、序号 1..22 连续、每类非空、类内无重复 token。
     * （测试的工作目录 = `android/app`，路径相对模块根。）
     */
    @Test
    fun realAssetHasAllCategories() {
        val f = File("src/main/assets/symbol_categories.tsv")
        assertTrue("资产不存在：${f.absolutePath}", f.isFile)
        val cats = SymbolCategories.parse(f.readText())
        assertEquals("类目数（对齐用户裁决的 22 项）", 22, cats.size)
        assertEquals("序号必须 1..22 连续", (1..22).toList(), cats.map { it.index })
        for (c in cats) {
            assertTrue("类目「${c.label}」为空", c.symbols.isNotEmpty())
            assertEquals(
                "类目「${c.label}」有重复符号",
                c.symbols.size,
                c.symbols.toSet().size,
            )
        }
        // 抽样钉几个关键成员（防生成器数据被误改）
        val all = cats.flatMap { it.symbols }
        for (s in listOf("，", "？", "①", "★", "→", "ā", "ㄅ", "⼀", "А", "α", "À", "░", "ཀ")) {
            assertTrue("缺符号 $s", s in all)
        }
    }

    /**
     * 2026-10-16 用户裁决「符号中常用要加上数学符号」：**常用类目**必须含这批常用数学符号
     * （原有的 × ÷ ± ≈ ≠ ≤ ≥ √ ∞ π 之上再补的一组）。这批字面是「面板上真用得到」的
     * 确认集合 —— 与生成器 `scripts/gen_symbol_categories.py` 的「常用」清单同步。
     */
    @Test
    fun commonCategoryCarriesTheMathSymbols() {
        val f = File("src/main/assets/symbol_categories.tsv")
        val common = SymbolCategories.parse(f.readText()).first { it.label == "常用" }
        val math = listOf(
            "×", "÷", "±", "≈", "≠", "≤", "≥", "√", "∞", "π",
            "∵", "∴", "∝", "≌", "≡", "≢", "≅", "∼", "≪", "≫",
            "∠", "⊥", "∥", "⌒", "∈", "∉", "∋", "⊂", "⊃", "⊆",
            "⊇", "∪", "∩", "∅", "∀", "∃", "⊕", "⊗", "∫", "∬",
            "∮", "∑", "∏", "∂", "∇", "∆", "′", "″",
        )
        val missing = math.filter { it !in common.symbols }
        assertTrue("常用类目缺数学符号：$missing", missing.isEmpty())
    }

    /**
     * 2026-10-17 用户裁决：「％、＃、＄、＠、\、/、｛、｝、[、]、＝、＋、｜等等类似常用的，
     * 往前面两页放，不常见不常用的就往后面放」。**前两页（前 16 = 8×2）逐位钉死**。
     *
     * 两页 = 16 的依据：桌面两端符号浏览页每页 8（`router.rs` 的 `PAGE_SIZE`；安卓是 8 列
     * 网格，首屏即覆盖前 16+）。钉**逐位**而不只钉成员集合：用户要的是「放前面」，
     * 成员断言分辨不出「在但排到第 5 页」—— 那正是这条改动要防的失败形态。
     */
    @Test
    fun commonCategoryFrontTwoPagesFollowTheUserRuling() {
        val f = File("src/main/assets/symbol_categories.tsv")
        val common = SymbolCategories.parse(f.readText()).first { it.label == "常用" }
        val want = listOf(
            "，", "。", "、",
            "％", "＃", "＄", "＠", "＼",
            "／", "｛", "｝", "［", "］", "＝", "＋", "｜",
        )
        assertEquals("常用前两页（8×2）逐位不符", want, common.symbols.take(16))
    }

    /**
     * 2026-10-16 用户追加裁决（同日晚）：「加上数学、物理、化学符号，可以参考主流输入法的……
     * 类似 ％ ｛ ｝ [ ] 等等这些符号，不要重复」：常用补**全角标点族**（半角形在英文/中文
     * 类目里，不重复——这里要的是全角码位）与**物理/化学符号**。
     */
    @Test
    fun commonCategoryCarriesPhysicsChemistryAndFullwidthSymbols() {
        val f = File("src/main/assets/symbol_categories.tsv")
        val common = SymbolCategories.parse(f.readText()).first { it.label == "常用" }
        val more = listOf(
            // 全角标点族（主流输入法常用同款；与英文/中文类目的半角形是**不同码位**）
            "％", "＃", "＆", "＠", "＊", "／", "＼",
            "｛", "｝", "［", "］", "〔", "〕", "＜", "＞", "＝", "＋", "－", "＿", "＾", "｜",
            "「", "」", "『", "』",
            // 物理（欧姆/微/埃/华氏度）
            "Ω", "µ", "Å", "℉",
            // 化学（可逆反应）
            "⇌",
        )
        val missing = more.filter { it !in common.symbols }
        assertTrue("常用类目缺物理/化学/全角符号：$missing", missing.isEmpty())
    }
}
