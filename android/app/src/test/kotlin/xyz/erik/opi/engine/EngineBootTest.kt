// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.concurrent.Executor

/**
 * [EngineBoot] 的线程契约（2026-10-28）：装载在 io、就绪回调经 post 回主线程、
 * `loading` 只在就绪回调**跑完**后落；装载抛异常也不许把键盘永远卡在骨架上。
 * 两个队列都手动放行 —— 不依赖真实线程与时序。
 */
class EngineBootTest {

    private class Queue : Executor {
        val tasks = mutableListOf<Runnable>()
        override fun execute(command: Runnable) {
            tasks += command
        }
    }

    @Test
    fun loadingDropsOnlyAfterThePostedReadyCallback() {
        val io = Queue()
        val posted = mutableListOf<Runnable>()
        var result: Boolean? = null
        val boot = EngineBoot(io) { posted += it }

        boot.start(load = { true }, onReady = { result = it })
        assertTrue("首帧必须是骨架", boot.loading)
        assertEquals(1, io.tasks.size)

        io.tasks.removeAt(0).run()
        assertNull("ready 还没回主线程：结果不能提前生效", result)
        assertTrue("ready 还没回主线程，不能提前组合真实键盘", boot.loading)
        assertEquals(1, posted.size)

        posted.removeAt(0).run()
        assertEquals(true, result)
        assertFalse("ready 跑完必须落 loading", boot.loading)
    }

    @Test
    fun bootThrowIsDegradedAndStillDropsLoading() {
        val io = Queue()
        val posted = mutableListOf<Runnable>()
        var result: Boolean? = null
        val boot = EngineBoot(io) { posted += it }

        boot.start(load = { throw IllegalStateException("so missing") }, onReady = { result = it })
        io.tasks.removeAt(0).run()
        posted.removeAt(0).run()

        assertTrue("装载抛异常按降级（false）处理，不是崩", result == false)
        assertFalse(boot.loading)
    }

    @Test
    fun secondStartIsIgnored() {
        val io = Queue()
        val boot = EngineBoot(io) {}

        boot.start(load = { true }, onReady = {})
        boot.start(load = { true }, onReady = {}) // 视图重建再调一次：不许重装

        assertEquals(1, io.tasks.size)
    }

    @Test
    fun readyThrowStillDropsLoading() {
        // onReady 抛出去在生产里会崩（调用方没接），本用例只钉 finally 那一半
        val io = Queue()
        val posted = mutableListOf<Runnable>()
        val boot = EngineBoot(io) { posted += it }

        boot.start(load = { true }) { throw IllegalStateException("ui gone") }
        io.tasks.removeAt(0).run()
        try {
            posted.removeAt(0).run()
        } catch (e: IllegalStateException) {
            // 预期
        }

        assertFalse(boot.loading)
    }
}
