// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 2026-10-18 用户裁决：「自有」是**独立顶级子页**，不与 Emoji 同组。
 * 分类条是纯函数（[emojiCategoryGroups]），这里钉住两页的边界。
 */
class EmojiTabsTest {

    private val engine = listOf(
        SymbolCatalog.EmojiGroup("笑脸", 0, 172),
        SymbolCatalog.EmojiGroup("人物", 172, 2430),
    )
    private val kaomoji = listOf(
        KaomojiCategories.Category("开心", 0, listOf("(＾▽＾)", "(*^▽^*)")),
        KaomojiCategories.Category("难过", 2, listOf("(；ω；)")),
    )

    @Test
    fun tabsAreSelfFirstThenEmojiThenKaomoji() {
        // 顺序 = 顶栏顺序；自有排第一（用户裁决），默认子页仍是 Emoji（见 EmojiPanel）。
        assertEquals(
            listOf("自有", "Emoji", "颜文字"),
            EmojiSubTab.entries.map { it.label },
        )
    }

    @Test
    fun emojiTabUsesEngineGroupsOnlyAndHasNoOwnGroup() {
        assertEquals(engine, emojiCategoryGroups(EmojiSubTab.Emoji, engine, kaomoji))
        assertTrue(
            "Emoji 分类条不得再出现「自有」",
            emojiCategoryGroups(EmojiSubTab.Emoji, engine, kaomoji).none { it.name == "自有" },
        )
    }

    @Test
    fun ownTabHasNoCategoryBar() {
        assertTrue(
            "自有是单段图片页，不显示分类条",
            emojiCategoryGroups(EmojiSubTab.Pet, engine, kaomoji).isEmpty(),
        )
    }

    @Test
    fun kaomojiTabMapsCategoriesWithCumulativeStarts() {
        assertEquals(
            listOf(
                SymbolCatalog.EmojiGroup("开心", 0, 2),
                SymbolCatalog.EmojiGroup("难过", 2, 1),
            ),
            emojiCategoryGroups(EmojiSubTab.Kaomoji, engine, kaomoji),
        )
    }

    @Test
    fun groupSliceShowsOnlyTheSelectedGroup() {
        // 2026-10-30 用户报障：点后一个分类显示前一个 —— 组的 start 不在行首，
        // 滚过去会露出上一组尾巴。改为切片后必须**只有**选中组的条目。
        val all = listOf("a", "b", "c", "d", "e")
        val groups = listOf(
            SymbolCatalog.EmojiGroup("一", 0, 3),
            SymbolCatalog.EmojiGroup("二", 3, 2),
        )
        assertEquals(listOf("a", "b", "c"), groupSlice(all, groups, 0))
        assertEquals(listOf("d", "e"), groupSlice(all, groups, 1))
    }

    @Test
    fun groupSliceFallsBackToWholeTableWhenGroupsMissingOrOutOfRange() {
        // 引擎未就绪 / 重载后组数变少：整表兜底，分类下标越界不能切出空页。
        val all = listOf("a", "b")
        assertEquals(all, groupSlice(all, emptyList(), 0))
        assertEquals(all, groupSlice(all, listOf(SymbolCatalog.EmojiGroup("一", 0, 2)), 3))
        // count 溢出不越界（start+count 夹到表尾）。
        assertEquals(all, groupSlice(all, listOf(SymbolCatalog.EmojiGroup("一", 0, 99)), 0))
    }
}
