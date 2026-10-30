// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 颜文字数据门禁（2026-10-18）：解析口径逐行镜像 Android `KaomojiCategories`，
//! 外加**真实资源的接线** —— `kaomoji.tsv` 由 `build.gradle.kts` 的 `copyKaomojiData`
//! 在构建期拷进资源，拷丢了页面只会显示「没有颜文字数据」（构建树里看不出来）。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class KaomojiDataTest {

    @Test
    fun `groups by category in first appearance order with cumulative start`() {
        val cats = KaomojiData.parse(
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
            KaomojiData.flatten(cats),
        )
    }

    @Test
    fun `bad lines are skipped not thrown`() {
        // 缺列 / 空颜文字 / 空分类：跳过，页面少几组而不是打不开（判据同 SymbolCategories）
        val cats = KaomojiData.parse(
            "只有文字没有制表符\n\t空颜文字\n(・ω・)\t\n(๑•̀ㅂ•́)و✧\t加油\n",
        )
        assertEquals(listOf("加油"), cats.map { it.label })
        assertEquals(0, cats[0].start)
    }

    /** 分类标签里的 CRLF 不能带进分类名（Windows 生成的数据常见）。 */
    @Test
    fun `crlf is handled`() {
        val cats = KaomojiData.parse("(^_^)\t开心\r\n(>_<)\t生气\r\n")
        assertEquals(listOf("开心", "生气"), cats.map { it.label })
        assertEquals("(^_^)", cats[0].items.single())
    }

    /**
     * 真资源门禁：classpath 上的 `kaomoji.tsv` 必须解析出**全部分类**，且 start 连续、
     * 条目不含制表符。分类名/组序钉死 —— 漂了就是生成器/数据出了事。
     */
    @Test
    fun `the real kaomoji table is on the classpath and self-consistent`() {
        val cats = loadKaomojiData()
        assertTrue("kaomoji.tsv 资源没在 classpath 上（copyKaomojiData 断了吗）", cats.isNotEmpty())
        assertEquals(
            listOf("开心", "难过", "生气", "惊讶", "害羞", "动作", "其他"),
            cats.map { it.label },
        )
        assertTrue("条目太少（实得 ${cats.sumOf { it.items.size }}）—— 表被截断？", cats.sumOf { it.items.size } >= 200)
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
        assertEquals("flatten 总条目与分类累计不一致", start, KaomojiData.flatten(cats).size)
    }

    /** 分类 → 分页器的组：start/count 与拼平列表对齐（点分类条 = 按组切片）。 */
    @Test
    fun `categories convert to groups whose starts match the flattened list`() {
        val cats = KaomojiData.parse("a\t开心\nb\t开心\nc\t生气\n")
        val groups = cats.map { EmojiData.Group(it.label, it.start, it.items.size) }
        assertEquals(
            listOf(EmojiData.Group("开心", 0, 2), EmojiData.Group("生气", 2, 1)),
            groups,
        )
        assertEquals(2, KaomojiData.flatten(cats).indexOf("c"))
    }
}
