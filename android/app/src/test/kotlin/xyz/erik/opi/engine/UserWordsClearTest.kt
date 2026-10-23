// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 「清除用户词库」两半的顺序（任务 2026-10-23）：app 设置页与键盘内设置面板共用
 * [clearUserWords] —— 先落盘、后引擎内存。
 *
 * 落盘那半用 [UserWordStoreTestBase.directIo] 同步执行，才能断言「引擎清空时文件已不在」
 * （生产里删除排在共享 io 线程，调用返回时可能尚未 rename/delete）。
 */
class UserWordsClearTest : UserWordStoreTestBase() {

    @Test
    fun clearsPersistedFileBeforeClearingEngineMemory() {
        val f = target().apply { writeText("""{"ni":3}""") }
        val seen = mutableListOf<String>()
        var fileExistedAtEngineClear: Boolean? = null

        clearUserWords(store(f)) {
            seen += "engine"
            fileExistedAtEngineClear = f.exists()
        }

        assertEquals("引擎内存清理必须被调用一次", listOf("engine"), seen)
        assertEquals(
            "落盘删除必须排在引擎清空之前（否则 IME 的在途保存会把旧词写回）",
            false,
            fileExistedAtEngineClear,
        )
        assertFalse("清除后落盘文件必须消失", f.exists())
    }

    @Test
    fun engineFailurePropagatesButPersistedHalfIsAlreadyCleared() {
        val f = target().apply { writeText("""{"ni":3}""") }
        val failure = IllegalStateException("dead .so")

        val thrown = try {
            clearUserWords(store(f)) { throw failure }
            null
        } catch (e: Throwable) {
            e
        }

        assertTrue("引擎失败必须交给 UI 报错（不得静默）", thrown === failure)
        assertFalse("落盘那半仍须先清掉", f.exists())
    }
}
