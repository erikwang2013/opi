// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi

import android.view.inputmethod.EditorInfo
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 回车决策（2026-10-10 用户报障「换行无效」的订正）：NO_ENTER_ACTION 优先于 action 位。
 * 纯函数，边界逐条钉。
 */
class EnterDecisionTest {

    @Test
    fun `no-enter-action forces a newline even with a stale action`() {
        // 真实世界的多行编辑器常两样都设：NO_ENTER_ACTION + 一个残留 action。
        // 旧逻辑只看 action ⇒ 把回车发成动作而不是换行。这条就是报障的机制。
        val opts = EditorInfo.IME_FLAG_NO_ENTER_ACTION or EditorInfo.IME_ACTION_SEND
        assertEquals(EnterDecision.Newline, enterDecision(opts))
    }

    @Test
    fun `declared action performs the action`() {
        assertEquals(
            EnterDecision.PerformAction(EditorInfo.IME_ACTION_SEARCH),
            enterDecision(EditorInfo.IME_ACTION_SEARCH),
        )
        assertEquals(
            EnterDecision.PerformAction(EditorInfo.IME_ACTION_SEND),
            enterDecision(EditorInfo.IME_ACTION_SEND),
        )
    }

    @Test
    fun `no action falls back to newline`() {
        assertEquals(EnterDecision.Newline, enterDecision(0))
        assertEquals(
            EnterDecision.Newline,
            enterDecision(EditorInfo.IME_ACTION_NONE),
        )
    }

    @Test
    fun `other imeOptions flags do not break action extraction`() {
        // action 位之外的 flag（如 NO_FULLSCREEN）不该干扰判定：单独出现 → Newline；
        // 与 action 并存 → 执行 action。
        assertEquals(EnterDecision.Newline, enterDecision(EditorInfo.IME_FLAG_NO_FULLSCREEN))
        assertEquals(
            EnterDecision.PerformAction(EditorInfo.IME_ACTION_GO),
            enterDecision(EditorInfo.IME_FLAG_NO_FULLSCREEN or EditorInfo.IME_ACTION_GO),
        )
    }
}
