// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 防抖调度（2026-10-31 从 ImeState.kt 拆出：加背景主题后那边 509 行撞 500 行
//! 门禁；同包引用不变）。生产 = 主线程 Handler；JVM 测试注入假时钟。

package xyz.erik.opi.ime

import android.os.Handler
import android.os.Looper

/** 防抖调度抽象：生产用主线程 Handler，JVM 测试注入假时钟（确定性，无 Thread.sleep）。 */
fun interface Debouncer {
    /** delayMs 后执行 action；返回取消函数。 */
    fun schedule(delayMs: Long, action: () -> Unit): () -> Unit
}

/** 生产实现：主线程 Handler.postDelayed（复用单个 Handler，不在每次调度时新建）。 */
class HandlerDebouncer : Debouncer {
    private val handler = Handler(Looper.getMainLooper())

    override fun schedule(delayMs: Long, action: () -> Unit): () -> Unit {
        val r = Runnable { action() }
        handler.postDelayed(r, delayMs)
        return { handler.removeCallbacks(r) }
    }
}
