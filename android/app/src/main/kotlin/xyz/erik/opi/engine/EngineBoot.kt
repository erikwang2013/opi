// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.util.concurrent.Executor

/**
 * 引擎装载的线程壳（2026-10-28）：慢活（词库拷贝/哈希 + Rust install）放后台，
 * 完成后回主线程切 [loading]。
 *
 * [loading] 初值 true：键盘首帧先画骨架，装载完成前**不组合真实键盘** —— 否则按键会
 * 落在「马上被 install 整体重置」的引擎上，击键静默丢失（install 会清缓冲/方案/模式）。
 */
class EngineBoot(
    private val io: Executor,
    private val post: (Runnable) -> Unit,
) {
    var loading by mutableStateOf(true)
        private set

    private var started = false

    /**
     * 只跑一次（主线程调用）。[load] 在 io 线程跑、返回词库装载结果；[onReady] 由
     * [post] 派回主线程执行。无论 [load] 抛什么都按降级（false）走 —— [loading]
     * 必须落，否则键盘永远停在骨架上。
     */
    fun start(load: () -> Boolean, onReady: (Boolean) -> Unit) {
        if (started) return
        started = true
        io.execute {
            val ok = try {
                load()
            } catch (t: Throwable) {
                false
            }
            post {
                try {
                    onReady(ok)
                } finally {
                    loading = false
                }
            }
        }
    }
}
