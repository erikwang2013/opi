// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import xyz.erik.opi.pet.PetMood

/**
 * 「自有」= 小欧形象，提交的是对应 emoji（见 [PetEmoji] 头注）。
 *
 * 映射在跨端共享的 [xyz.erik.opi.pet.PetEmojiMap] 里按枚举键建立；本测试的「除
 * 状态外每个形态都要有映射」一条会覆盖新追加的形态 —— 漏配会在这里红，而不是在
 * 真机上少两格宠物。
 */
class PetEmojiTest {

    @Test
    fun existingSixFormsMapToTheirEmoji() {
        // 既有六形态（docs/opi-pet-forms.svg 的笑/欢乐/哭/沮丧/惊恐/愤怒）
        assertEquals("😊", PetEmoji.emojiFor(PetMood.IDLE))
        assertEquals("😄", PetEmoji.emojiFor(PetMood.WAITING))
        assertNotEquals(
            "笑与欢乐不能提交同一个 emoji",
            PetEmoji.emojiFor(PetMood.IDLE),
            PetEmoji.emojiFor(PetMood.WAITING),
        )
        assertEquals("😞", PetEmoji.emojiFor(PetMood.PUZZLED))
        assertEquals("😭", PetEmoji.emojiFor(PetMood.CRYING))
        assertEquals("😱", PetEmoji.emojiFor(PetMood.TERRIFIED))
        assertEquals("😠", PetEmoji.emojiFor(PetMood.ANGRY))
    }

    @Test
    fun sleepyAndDegradedAreInputStatesNotForms() {
        // 不记事 / 引擎降级是状态，不是用户可挑的表情形态 —— 不进「自有」网格。
        assertNull(PetEmoji.emojiFor(PetMood.SLEEPY))
        assertNull(PetEmoji.emojiFor(PetMood.DEGRADED))
        val moods = PetEmoji.forms.map { it.mood }
        assertTrue(PetMood.SLEEPY !in moods)
        assertTrue(PetMood.DEGRADED !in moods)
    }

    /**
     * 除两个状态外，枚举里的**每个**形态都必须有映射。新六形态落地后这条自动扩到
     * 12 枚；映射名与枚举名对不上时这里会红（按名字映射的代价，用测试兜住）。
     */
    @Test
    fun everyNonStateMoodHasAForm() {
        val missing = PetMood.entries
            .filter { it != PetMood.SLEEPY && it != PetMood.DEGRADED }
            .filter { PetEmoji.emojiFor(it) == null }
        assertTrue("这些 PetMood 没有「自有」映射（名字写错？）：$missing", missing.isEmpty())
    }

    /** 网格顺序 = 枚举顺序（枚举顺序是跨端契约，不能按映射表书写顺序排）。 */
    @Test
    fun formsFollowEnumOrderAndAreLabelled() {
        val expected = PetMood.entries.filter {
            it != PetMood.SLEEPY && it != PetMood.DEGRADED
        }
        assertEquals(expected, PetEmoji.forms.map { it.mood })
        for (f in PetEmoji.forms) {
            assertTrue("形态 ${f.mood} 缺无障碍标签", f.label.isNotEmpty())
            assertTrue("形态 ${f.mood} 缺 emoji", f.emoji.isNotEmpty())
        }
    }

    /**
     * 分类条 = 「自有」(start=0) + 引擎分组；引擎 start 相对表情列表，前插小欧后
     * 整体后移。高亮反推与 Emoji 页共用 [SymbolCatalog.groupIndexOf]。
     */
    @Test
    fun categoriesPrependOwnGroupAndShiftEngineStarts() {
        val own = PetEmoji.forms.size
        val groups = PetEmoji.categories(
            listOf(
                SymbolCatalog.EmojiGroup("笑脸", 0, 172),
                SymbolCatalog.EmojiGroup("人物", 172, 2430),
            ),
        )
        assertEquals(SymbolCatalog.EmojiGroup("自有", 0, own), groups[0])
        assertEquals(own, groups[1].start)
        assertEquals(own + 172, groups[2].start)
        assertEquals("引擎组条数不动", 172, groups[1].count)

        assertEquals("网格第 0 项 = 自有", 0, SymbolCatalog.groupIndexOf(groups, 0))
        assertEquals("自有最后一格仍是自有", 0, SymbolCatalog.groupIndexOf(groups, own - 1))
        assertEquals("自有之后进第一组", 1, SymbolCatalog.groupIndexOf(groups, own))
        assertEquals("第二组边界", 2, SymbolCatalog.groupIndexOf(groups, own + 172))
    }
}
