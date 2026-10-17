// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 顶部快捷条（2026-10-17 用户裁决「默认顶部好空」→ 顶部共用槽位：输入中拼音、空闲常用语）：
 * 学习最近上屏的词/短语，最前最新；只有「短、且含汉字」的提交才进条。
 */
class QuickPhrasesTest {
    @Test
    fun defaultsFillTheStripOnFirstRun() {
        val q = QuickPhrases(QuickPhrases.DEFAULT)
        assertTrue("首启不能是空的（顶部又空回去了）", q.items.isNotEmpty())
        assertEquals("重复项要被清洗", q.items.size, q.items.toSet().size)
    }

    @Test
    fun recordMovesTheLatestToFront() {
        val q = QuickPhrases(QuickPhrases.DEFAULT)
        q.record("晚上开会")
        assertEquals("晚上开会", q.items.first())
        q.record("明天见")
        assertEquals("明天见", q.items.first())
        q.record("晚上开会") // 再打一次：回到最前，不产生重复
        assertEquals("晚上开会", q.items.first())
        assertEquals(q.items.size, q.items.toSet().size)
    }

    @Test
    fun recordCapsTheList() {
        val q = QuickPhrases(QuickPhrases.DEFAULT)
        repeat(QuickPhrases.MAX + 5) { i -> q.record("短语编号$i") }
        assertEquals(QuickPhrases.MAX, q.items.size)
        assertEquals("短语编号${QuickPhrases.MAX + 4}", q.items.first())
    }

    @Test
    fun recordRejectsNoise() {
        val q = QuickPhrases(QuickPhrases.DEFAULT)
        val before = q.items
        // 单字、纯拉丁、空白、过长、含分隔符 —— 都不进条（快捷条是「词/短语」的位置）
        assertFalse(q.record("好"))
        assertFalse(q.record("hello"))
        assertFalse(q.record("  "))
        assertFalse(q.record("这是一条非常非常非常长的句子超过上限"))
        assertFalse(q.record("夹" + QuickPhrases.SEP + "带"))
        assertEquals(before, q.items)
    }

    @Test
    fun removeDropsTheItem() {
        val q = QuickPhrases(QuickPhrases.DEFAULT)
        val victim = q.items.first()
        q.remove(victim)
        assertFalse(q.items.contains(victim))
    }

    @Test
    fun onChangeFiresOnlyOnRealChanges() {
        var changes = 0
        val q = QuickPhrases(QuickPhrases.DEFAULT)
        q.onChanged = { changes++ }
        q.record("晚上开会") // 真的变了
        assertEquals(1, changes)
        q.record("hello") // 被拒：不该落盘
        assertEquals(1, changes)
        q.record("晚上开会") // 已经在最前：值没变，不该落盘
        assertEquals(1, changes)
    }
}
