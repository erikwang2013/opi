// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.engine.FakeEngine

/**
 * 长按 ⌫ = **按住连续删除**（2026-10-23 用户裁决）。
 *
 * 重复节奏由键盘层的 `KeyButton.repeatOnLongPress` 驱动；路由器每一拍只做一次
 * 普通退格（组合态退引擎，组合空后退宿主一个字符），**没有 clear-all**。
 * 这里钉住“每一拍”的语义，防止以后又退化成一次性清空。
 */
class KeyRouterLongPressTest {
    private class Channel {
        val commits = mutableListOf<String>()
        var backspaceCount = 0
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

    @Test
    fun eachRepeatTickDeletesOneCompositionCharacter() {
        val fake = FakeEngine().apply {
            buf = "nihao"
            cands = arrayOf("你好", "你号")
        }
        val ch = Channel()
        val (ctrl, r) = router(fake, ch)

        repeat(3) { r.handleBackspace() }

        assertEquals("三拍 = 三次引擎退格", 3, fake.backspaceCalls)
        assertEquals("不许一次性 clear", 0, fake.clearCalls)
        assertEquals("ni", ctrl.buffer)
        assertEquals("不许删宿主正文", 0, ch.backspaceCount)
        assertTrue("不许提交半截组合", ch.commits.isEmpty())
    }

    @Test
    fun eachTickDeletesOneHostCharacterWhenCompositionIsEmpty() {
        val fake = FakeEngine()
        val ch = Channel()
        val (_, r) = router(fake, ch)

        repeat(4) { r.handleBackspace() }

        assertEquals("空组合：每拍退宿主一个字符", 4, ch.backspaceCount)
        assertEquals("不许调引擎 clear", 0, fake.clearCalls)
        assertEquals("不许调引擎退格", 0, fake.backspaceCalls)
        assertTrue(ch.commits.isEmpty())
    }
}
