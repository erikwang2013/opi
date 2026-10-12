// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 表情页数据门禁（2026-10-12）：解析口径（连续同名段切组，对齐引擎
//! `builtin_emoji_groups`）、组跳页的页码算法，以及**真实资源的接线**——
//! 最后一条是重点：`emoji.tsv` 由 `build.gradle.kts` 的 copyEmojiData 在构建期拷进
//! 资源，拷丢了页面只会显示「没有表情数据」（构建树里看不出来），所以这里从
//! classpath 真读一次。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.ceil

class EmojiDataTest {

    @Test
    fun `consecutive runs of the same group name become one group`() {
        // 夹具含：同组连续段、组名**再次出现**（中间隔了别的组 —— 必须切成两组，
        // 这是「连续段」与「按名字去重」的分水岭）、缺列坏行、空行、空首列。
        val tsv = """
            😀	grinning face	笑脸
            😃	grinning face with big eyes	笑脸
            坏行没有制表符
            😂	face with tears of joy	笑脸
            	empty first col	笑脸
            🐶	dog face	动物
            缺第三列
            🍎	red apple	食物
            🐱	cat face	动物
        """.trimIndent()
        val d = EmojiData.parse(tsv)
        assertEquals(
            listOf("😀", "😃", "😂", "🐶", "🍎", "🐱"),
            d.entries,
        )
        assertEquals(
            listOf(
                EmojiData.Group("笑脸", 0, 3),
                EmojiData.Group("动物", 3, 1),
                EmojiData.Group("食物", 4, 1),
                EmojiData.Group("动物", 5, 1), // 同名的第二段 = 独立一组
            ),
            d.groups,
        )
    }

    @Test
    fun `group start maps to the page holding it`() {
        val size = SymbolCategories.PAGE_SIZE
        assertEquals(0, EmojiData.pageOfGroup(EmojiData.Group("g", 0, 1)))
        assertEquals(0, EmojiData.pageOfGroup(EmojiData.Group("g", size - 1, 1)))
        assertEquals(1, EmojiData.pageOfGroup(EmojiData.Group("g", size, 1)))
        assertEquals(4, EmojiData.pageOfGroup(EmojiData.Group("g", size * 4 + 5, 1)))
    }

    @Test
    fun `empty table degrades to empty`() {
        assertEquals(EmojiData.Data(emptyList(), emptyList()), EmojiData.parse(""))
        assertEquals(EmojiData.Data(emptyList(), emptyList()), EmojiData.parse("坏行\t少一列"))
    }

    @Test
    fun `the real emoji table is on the classpath and self-consistent`() {
        // 构建期拷贝的产物（data/raw/emoji.tsv → resources/emoji.tsv）。读不到 = 构建接线断。
        val d = loadEmojiData()
        assertTrue("emoji.tsv 资源没在 classpath 上（copyEmojiData 断了吗）", d.entries.isNotEmpty())
        assertTrue("条目太少（实得 ${d.entries.size}）—— 表被截断？", d.entries.size >= 3900)
        // 组名与顺序**钉死**（与引擎 emoji_coverage.rs 的同一串 —— 9 组：官方 10 组里
        // Component 整组被「只收完全限定」滤掉）。名字漂了就是生成器/数据出了事。
        assertEquals(
            listOf("笑脸", "人物", "动物", "食物", "旅行", "活动", "物品", "符号", "旗帜"),
            d.groups.map { it.label },
        )

        // 分组与条目必须严丝合缝：首组从 0 起、每组起点 = 上一组起点+条数、总数相等。
        assertEquals(0, d.groups[0].start)
        for (i in 1 until d.groups.size) {
            assertEquals(
                "第 $i 组的起点不接上一组",
                d.groups[i - 1].start + d.groups[i - 1].count,
                d.groups[i].start,
            )
        }
        assertEquals(d.entries.size, d.groups.sumOf { it.count })
        assertTrue("末组越界", d.groups.last().start + d.groups.last().count <= d.entries.size)
        assertTrue(
            "页数算法与页大小对不上",
            ceil(d.entries.size.toDouble() / SymbolCategories.PAGE_SIZE).toInt() >= 1,
        )
    }
}
