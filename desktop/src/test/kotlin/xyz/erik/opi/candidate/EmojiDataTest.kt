// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 表情页数据门禁（2026-10-12）：解析口径（连续同名段切组，对齐引擎
//! `builtin_emoji_groups`）、分类切片算法，以及**真实资源的接线**——
//! 最后一条是重点：`emoji.tsv` 由 `build.gradle.kts` 的 copyEmojiData 在构建期拷进
//! 资源，拷丢了页面只会显示「没有表情数据」（构建树里看不出来），所以这里从
//! classpath 真读一次。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
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
    fun `group slice shows exactly the selected group`() {
        // 2026-10-30 报障：组的 start 不落在网格行首时，「跳到 start 所在页」会把上一组
        // 的尾巴带进首行。切片必须严丝合缝 = [start, start+count)。
        val groups = listOf(
            EmojiData.Group("笑脸", 0, 3),
            EmojiData.Group("人物", 3, 2),
            EmojiData.Group("动物", 5, 1),
        )
        val all = listOf("a", "b", "c", "d", "e", "f")
        assertEquals(listOf("a", "b", "c"), EmojiData.groupSlice(all, groups, 0))
        assertEquals(listOf("d", "e"), EmojiData.groupSlice(all, groups, 1))
        assertEquals(listOf("f"), EmojiData.groupSlice(all, groups, 2))
    }

    @Test
    fun `group slice clamps bounds and falls back on a bad index`() {
        val all = listOf("a", "b", "c")
        // count / start 越过表尾 → 夹紧，不抛越界。
        assertEquals(listOf("b", "c"), EmojiData.groupSlice(all, listOf(EmojiData.Group("g", 1, 99)), 0))
        assertEquals(emptyList<String>(), EmojiData.groupSlice(all, listOf(EmojiData.Group("g", 3, 5)), 0))
        // 组表为空 / 下标越界（引擎未就绪）→ 整表兜底，页面不至于空。
        assertEquals(all, EmojiData.groupSlice(all, emptyList(), 0))
        assertEquals(all, EmojiData.groupSlice(all, listOf(EmojiData.Group("g", 0, 3)), 1))
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
        assertTrue(
            "条目数 ${d.entries.size} 不在 400–700 —— 表被截断或筛选失控？",
            d.entries.size in 400..700,
        )
        // 组名与顺序**钉死**（与引擎 emoji_coverage.rs 的同一串 —— 9 组：OpenMoji
        // 常用子集，动物里的 plant-* 单列成「植物」，不再有旗帜组）。名字漂了
        // 就是生成器/数据出了事。
        assertEquals(
            listOf("笑脸", "人物", "动物", "植物", "食物", "旅行", "活动", "物品", "符号"),
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

    // ---------- 字形过滤（2026-10-13，口径对齐 Android `SymbolCatalog.emojiGroups`） ----------

    @Test
    fun `filter keeps order and remaps group start and count together`() {
        val d = EmojiData.parse(
            "😀\ta\t笑脸\n😃\tb\t笑脸\n🐶\tc\t动物\n🍎\td\t食物\n",
        )
        val f = EmojiData.filter(d) { it != "😃" && it != "🍎" }
        assertEquals(listOf("😀", "🐶"), f.entries)
        // start 与 count **一起**重算：start 是过滤后列表里的下标（组条点击 = groupSlice），
        // count 是组条文案里报的个数 —— 只重算一个就是「点 A 组跳到 B 组」或「个数多报」。
        assertEquals(
            listOf(
                EmojiData.Group("笑脸", 0, 1),
                EmojiData.Group("动物", 1, 1),
                EmojiData.Group("食物", 2, 0),
            ),
            f.groups,
        )
    }

    @Test
    fun `filter maps a fully dropped group to zero count at the previous group's start`() {
        // 整组被摘空（本机一个新码位表情都没有）：该组退化成 count = 0、start 与下一组
        // 相同 —— 组条上它还是个格位（与 Android 同），只是点它跳到的是下一组。
        val d = EmojiData.parse("😀\ta\t笑脸\n🐶\tb\t动物\n🍎\tc\t食物\n")
        val f = EmojiData.filter(d) { it != "🐶" }
        assertEquals(listOf("😀", "🍎"), f.entries)
        assertEquals(
            listOf(
                EmojiData.Group("笑脸", 0, 1),
                EmojiData.Group("动物", 1, 0),
                EmojiData.Group("食物", 1, 1),
            ),
            f.groups,
        )
    }

    @Test
    fun `filter is idempotent and hands back the same table when nothing is dropped`() {
        val d = EmojiData.parse("😀\ta\t笑脸\n🐶\tb\t动物\n")
        assertSame("一条没摘该原样返回", d, EmojiData.filter(d) { true })
        val once = EmojiData.filter(d) { it != "🐶" }
        assertEquals(once, EmojiData.filter(once) { it != "🐶" })
    }

    @Test
    fun `filter emptying the table degrades to the empty table`() {
        // 全表被摘空 ⇒ 空 Data（页面走既有的「没有表情数据」降级）；此时组条没有意义。
        val d = EmojiData.parse("😀\ta\t笑脸\n")
        assertEquals(EmojiData.Data(emptyList(), emptyList()), EmojiData.filter(d) { false })
    }

    @Test
    fun `filtering the real table keeps the group arithmetic intact`() {
        // 真表 + **确定性**谓词（隔一条摘一条，与字体无关）：重算后的分组仍必须严丝合缝
        // —— 这是「点组条切出哪一组」的全部依据。谓词带计数器：顺带钉住
        // 「每条只问一次、按表序问」（问两次会让这个计数器错位，实现真的变了才红）。
        val d = loadEmojiData()
        assertTrue("真表没读到", d.entries.isNotEmpty())
        var n = -1
        val f = EmojiData.filter(d) { n++; n % 2 == 0 }
        assertEquals("每条只问一次且按序", d.entries.size - 1, n)
        assertEquals((d.entries.size + 1) / 2, f.entries.size)
        assertEquals("保序（是原表的子序列）", d.entries.filterIndexed { i, _ -> i % 2 == 0 }, f.entries)
        assertEquals(0, f.groups[0].start)
        for (k in 1 until f.groups.size) {
            assertEquals("第 $k 组的起点不接上一组", f.groups[k - 1].start + f.groups[k - 1].count, f.groups[k].start)
        }
        assertEquals(f.entries.size, f.groups.sumOf { it.count })
        assertTrue("末组越界", f.groups.last().start + f.groups.last().count <= f.entries.size)
    }

    // ---------- 表情页分类（2026-10-18 用户裁决：自有是独立子页） ----------

    /** Emoji 分类条/网格**不含「自有」**：自有已提为独立顶级分类（见 PetStickerPage）。 */
    @Test
    fun `emoji categories never contain the own group`() {
        val d = loadEmojiData()
        assertTrue("表情表没读到", d.entries.isNotEmpty())
        assertTrue("自有混进了 Emoji 分类条", d.groups.none { it.label == "自有" })
        // 分类条仍覆盖整张表：首组 0 起、组间无缝、总和 = 条目数。
        assertEquals(0, d.groups.first().start)
        assertEquals(d.entries.size, d.groups.sumOf { it.count })
    }

    /** 整组被字形过滤摘空：点该分类切片为空（页面走空态，不借用邻组条目）。 */
    @Test
    fun `a group emptied by filtering slices to an empty list`() {
        val d = EmojiData.parse("😀\ta\t笑脸\n🐶\tb\t动物\n🍎\tc\t食物\n")
        val f = EmojiData.filter(d) { it != "🐶" }
        assertEquals(listOf("😀"), EmojiData.groupSlice(f.entries, f.groups, 0))
        assertEquals(emptyList<String>(), EmojiData.groupSlice(f.entries, f.groups, 1))
        assertEquals(listOf("🍎"), EmojiData.groupSlice(f.entries, f.groups, 2))
    }

    /**
     * 真表逐组切片：与组条 count 一致、互不重叠、并集 = 全表。顺带钉住夹具里确实存在
     * 「start 不是页大小整数倍」的组 —— 否则这条测试再也暴露不出错位 bug。
     */
    @Test
    fun `the real table slices exactly into its groups`() {
        val d = loadEmojiData()
        assertTrue("表情表没读到", d.entries.isNotEmpty())
        assertTrue(
            "所有组的 start 都正好在页首，夹具已无法暴露「跳到 start 所在页」的错位",
            d.groups.any { it.start % SymbolCategories.PAGE_SIZE != 0 },
        )
        val seen = ArrayList<String>()
        for (i in d.groups.indices) {
            val g = d.groups[i]
            val slice = EmojiData.groupSlice(d.entries, d.groups, i)
            assertEquals("第 $i 组（${g.label}）切片条数与组条不符", g.count, slice.size)
            seen.addAll(slice)
        }
        assertEquals("各组切片并集必须等于全表（无重叠无遗漏）", d.entries, seen)
    }
}
