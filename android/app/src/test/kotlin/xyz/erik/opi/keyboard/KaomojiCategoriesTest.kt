// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import java.io.File

/** 颜文字分类：解析容错 + 累计下标 + 高亮反推（真资产测在数据入库后自动生效）。 */
class KaomojiCategoriesTest {

    @Test
    fun groupsByCategoryInFirstAppearanceOrderWithCumulativeStart() {
        val cats = KaomojiCategories.parse(
            "(＾▽＾)\t开心\n(╯°□°）╯︵ ┻━┻\t生气\n(￣▽￣)\t开心\n",
        )
        assertEquals(listOf("开心", "生气"), cats.map { it.label })
        // 同名分类并进第一处；start 是拼平后的累计下标
        assertEquals(listOf("(＾▽＾)", "(￣▽￣)"), cats[0].items)
        assertEquals(0, cats[0].start)
        assertEquals(listOf("(╯°□°）╯︵ ┻━┻"), cats[1].items)
        assertEquals(2, cats[1].start)
        assertEquals(
            listOf("(＾▽＾)", "(￣▽￣)", "(╯°□°）╯︵ ┻━┻"),
            KaomojiCategories.flatten(cats),
        )
    }

    @Test
    fun badLinesAreSkippedNotThrown() {
        // 缺列 / 空颜文字 / 空分类：跳过，页面少几组而不是崩 IME（判据同 SymbolCategories）
        val cats = KaomojiCategories.parse(
            "只有文字没有制表符\n\t空颜文字\n(・ω・)\t\n(๑•̀ㅂ•́)و✧\t加油\n",
        )
        assertEquals(listOf("加油"), cats.map { it.label })
        assertEquals(0, cats[0].start)
    }

    /** 分类标签里的 CRLF 不能带进分类名（Windows 生成的数据常见）。 */
    @Test
    fun crlfIsHandled() {
        val cats = KaomojiCategories.parse("(^_^)\t开心\r\n(>_<)\t生气\r\n")
        assertEquals(listOf("开心", "生气"), cats.map { it.label })
        assertEquals("(^_^)", cats[0].items.single())
    }

    @Test
    fun groupIndexOfPicksTheCategoryContainingTheIndex() {
        // 面板把解析结果映射成 EmojiGroup 后，与 Emoji 页共用同一支 groupIndexOf。
        val cats = listOf(
            KaomojiCategories.Category("开心", 0, listOf("a", "b")),
            KaomojiCategories.Category("生气", 2, listOf("c")),
            KaomojiCategories.Category("加油", 3, listOf("d", "e")),
        )
        val groups = cats.map { SymbolCatalog.EmojiGroup(it.label, it.start, it.items.size) }
        assertEquals(0, SymbolCatalog.groupIndexOf(groups, 0))
        assertEquals(0, SymbolCatalog.groupIndexOf(groups, 1))
        assertEquals(1, SymbolCatalog.groupIndexOf(groups, 2))
        assertEquals(2, SymbolCatalog.groupIndexOf(groups, 3))
        assertEquals("空表 → 不亮", -1, SymbolCatalog.groupIndexOf(emptyList(), 0))
    }

    /**
     * 真资产门禁：`kaomoji.tsv` 由数据侧生成入库；**尚未入库时跳过**（本地/CI 的
     * 过渡期不该红），入库后自动生效 —— 分类非空、start 连续、条目非空且不含制表符。
     * 测试工作目录 = `android/app`，路径相对模块根。
     */
    @Test
    fun realAssetParsesWhenPresent() {
        val f = File("src/main/assets/kaomoji.tsv")
        assumeTrue("kaomoji.tsv 尚未入库（数据侧生成中），跳过真资产门禁", f.isFile)
        val cats = KaomojiCategories.parse(f.readText())
        assertTrue("资产存在但解析出 0 个分类", cats.isNotEmpty())
        var start = 0
        for (c in cats) {
            assertEquals("分类「${c.label}」start 不连续", start, c.start)
            assertTrue("分类「${c.label}」为空", c.items.isNotEmpty())
            assertTrue(
                "分类「${c.label}」的颜文字含制表符（会破坏两列格式）",
                c.items.none { '\t' in it },
            )
            start += c.items.size
        }
    }
}
