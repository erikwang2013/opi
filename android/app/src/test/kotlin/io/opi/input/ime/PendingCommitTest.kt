// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package io.opi.input.ime

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 提交重试的撤销契约（纯 JVM，注入假调度器）。
 *
 * 被考察的不变式：**撤销必须打在投递的那个调度器上**。原先投递与撤销都现取
 * `window.window.decorView`，而 IME 窗口在旋转/配置变化时整棵重建，撤销时取到的
 * decorView 已不是投递时那个 → removeCallbacks 撤不掉旧 root 上的 action。
 * 这里断言的是"撤销确实作用到排队中的那一次投递"这个可观测推论。
 */
class PendingCommitTest {

    /** 假调度器：手动 fire，并统计取消次数（取消次数即"撤销是否打到正确目标"）。 */
    private class FakeScheduler : Debouncer {
        var schedules = 0
            private set
        var cancels = 0
            private set
        var lastDelayMs = -1L
            private set
        private var current: (() -> Unit)? = null

        override fun schedule(delayMs: Long, action: () -> Unit): () -> Unit {
            schedules++
            lastDelayMs = delayMs
            current = action
            return {
                if (current === action) {
                    current = null
                    cancels++
                }
            }
        }

        /** 调度窗口到点：执行排队中的回调。 */
        fun fire() {
            val a = current
            current = null
            a?.invoke()
        }
    }

    @Test
    fun retryRunsOnlyWhenSchedulerFires() {
        val sched = FakeScheduler()
        var commits = 0

        PendingCommit(sched).schedule { commits++ }

        assertEquals(0, commits) // 不是立即执行
        sched.fire()
        assertEquals(1, commits)
    }

    @Test
    fun retryDelayMatchesConstant() {
        val sched = FakeScheduler()

        PendingCommit(sched).schedule {}

        assertEquals(PendingCommit.RETRY_MS, sched.lastDelayMs)
    }

    @Test
    fun secondScheduleCancelsTheFirstPendingRetry() {
        // 两次重试叠加 = 同一个词被提交两遍
        val sched = FakeScheduler()
        val order = mutableListOf<String>()

        val p = PendingCommit(sched)
        p.schedule { order += "first" }
        p.schedule { order += "second" }
        sched.fire()

        assertEquals(listOf("second"), order)
    }

    @Test
    fun cancelGoesThroughTheSchedulerThatReceivedTheSchedule() {
        // 撤销必须打到**投递时**的那个调度器上；打偏了就撤不掉（旋转重建 decorView 的场景）
        val sched = FakeScheduler()

        val p = PendingCommit(sched)
        p.schedule {}
        p.cancel()

        assertEquals(1, sched.schedules)
        assertEquals(1, sched.cancels) // 撤销确实落在排队中的那一次投递上
    }

    @Test
    fun cancelledRetryNeverRuns() {
        val sched = FakeScheduler()
        var commits = 0

        val p = PendingCommit(sched)
        p.schedule { commits++ }
        p.cancel()
        sched.fire() // 即使调度器仍然到点

        assertEquals(0, commits) // 输入目标已经换了，不能再把上一次的候选写进去
    }

    @Test
    fun cancelWithoutPendingRetryIsNoop() {
        val sched = FakeScheduler()

        PendingCommit(sched).cancel()
        PendingCommit(sched).cancel() // onStartInput/onFinishInputView/onDestroy 都会调

        assertEquals(0, sched.cancels)
    }

    @Test
    fun cancelAfterFireDoesNotDisturbLaterSchedule() {
        val sched = FakeScheduler()
        var commits = 0

        val p = PendingCommit(sched)
        p.schedule { commits++ }
        sched.fire()
        p.cancel() // 已经执行完的重试不该被"撤销"影响
        p.schedule { commits++ }
        sched.fire()

        assertEquals(2, commits)
    }
}
