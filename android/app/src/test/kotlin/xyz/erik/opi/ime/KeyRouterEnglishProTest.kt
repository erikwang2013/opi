// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import org.junit.Assert.assertEquals
import org.junit.Test
import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.engine.EngineMode
import xyz.erik.opi.engine.FakeEngine

/** 英文普通模式：字母/数字直出，不进引擎、不产生词库候选（2026-10-19）。 */
class KeyRouterEnglishProTest {
    @Test
    fun englishNormalModeCommitsDirectlyWithoutEngineMatching() {
        val fake = FakeEngine().apply { mode = EngineMode.ENGLISH.value }
        val commits = mutableListOf<String>()
        val ctrl = EngineController(fake)
        val r = KeyRouter(
            ctrl,
            commit = { commits += it },
            deleteBackward = {},
            performEnter = {},
            englishPro = { false },
        )
        r.handleKey("a")
        r.handleKey("1")
        assertEquals("普通模式不查词库、不进引擎缓冲", emptyList<String>(), fake.inputCalls)
        assertEquals(listOf("a", "1"), commits)
        assertEquals("缓冲保持空", "", ctrl.buffer)
    }
}
