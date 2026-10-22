// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.engine.FakeEngine
import xyz.erik.opi.engine.composingText

/** 长按 ⌫ = 清空整段组合（任务 2026-10-23）。从 KeyRouterTest 拆出守 500 行。 */
class KeyRouterLongPressTest {
    private class Channel {
        val commits = mutableListOf<String>()
        var backspaceCount = 0
        var enterCount = 0
    }

    private fun router(fake: FakeEngine, ch: Channel): Pair<EngineController, KeyRouter> {
        val ctrl = EngineController(fake)
        return ctrl to KeyRouter(
            ctrl,
            commit = { ch.commits += it },
            deleteBackward = { ch.backspaceCount++ },
            performEnter = {},
        )
    }

    // ---- 长按退格 = 清空整段组合（任务 2026-10-23）----

    @Test
    fun backspaceLongPressClearsWholeBufferWithoutCommitting() {
        val fake = FakeEngine().apply {
            buf = "nihao"
            cands = arrayOf("你好", "你号")
        }
        val ch = Channel()
        val (ctrl, r) = router(fake, ch)
        r.handleBackspaceLongPress()
        assertEquals("清空走引擎 clear（不是逐字退格）", 1, fake.clearCalls)
        assertEquals("不许逐字退格", 0, fake.backspaceCalls)
        assertEquals("不许提交任何文本", emptyList<String>(), ch.commits)
        assertEquals("候选随缓冲一起清空", emptyList<String>(), ctrl.candidates)
        assertEquals("", ctrl.buffer)
    }

    @Test
    fun backspaceLongPressClearsPendingSegmentsWithoutDeletingEditorText() {
        // 逐字确认态（2026-10-17）：缓冲可能已空、片段还在候选区 —— 长按也必须清掉
        // 这个组合态，而**不是**去删宿主编辑器里的正文。
        val fake = FakeEngine().apply {
            buf = ""
            pending = "你好"
            cands = arrayOf("你好世界")
        }
        val ch = Channel()
        val (ctrl, r) = router(fake, ch)
        r.handleBackspaceLongPress()
        assertEquals(1, fake.clearCalls)
        assertEquals("不许删宿主正文", 0, ch.backspaceCount)
        assertEquals("片段也随 clear 清空", "", ctrl.pendingText)
        assertTrue("不许提交半截组合", ch.commits.isEmpty())
    }

    @Test
    fun backspaceLongPressWithEmptyCompositionFallsBackToSingleDelete() {
        // 空组合：允许退掉宿主里**一个**字符（与短按同义），但绝不允许「删光编辑器」。
        val fake = FakeEngine()
        val ch = Channel()
        val (_, r) = router(fake, ch)
        r.handleBackspaceLongPress()
        assertEquals("缓冲空时不许调 clear", 0, fake.clearCalls)
        assertEquals("退回单字删除", 1, ch.backspaceCount)
        assertEquals(0, fake.backspaceCalls)
        assertTrue(ch.commits.isEmpty())
    }

    @Test
    fun backspaceLongPressRefreshSyncsTheHostCompositionArea() {
        // 清空后 refresh 必须走到组合串同步点（服务层借此收掉字段里可能残留的组合区）。
        val fake = FakeEngine().apply { buf = "nihao"; displayBuf = "ni'hao" }
        var synced = 0
        val ctrl = EngineController(fake, onBufferSynced = { synced++ })
        val ch = Channel()
        val r = KeyRouter(
            ctrl,
            commit = { ch.commits += it },
            deleteBackward = { ch.backspaceCount++ },
            performEnter = { ch.enterCount++ },
        )
        // 构造器自己会 refresh 一次（sync 一次）—— 只数清空带来的那一次。
        val before = synced
        r.handleBackspaceLongPress()
        assertEquals("clear 的 refresh 必须触发 onBufferSynced", before + 1, synced)
        assertEquals(1, fake.clearCalls)
        assertEquals("同步点读到的是已清空的组合串", "", ctrl.composingText())
    }

}
