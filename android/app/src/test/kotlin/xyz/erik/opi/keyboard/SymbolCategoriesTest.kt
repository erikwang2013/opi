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
}
