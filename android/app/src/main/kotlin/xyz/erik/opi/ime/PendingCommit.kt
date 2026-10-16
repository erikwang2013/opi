// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

// 2026-10-16 从 ImeState.kt 拆出（500 行门禁：平铺候选视图的裁决函数顶破了它）。
// 纯搬移，一字未动 —— 本类与 ImeState 无耦合（独立单测在 PendingCommitTest）。

/**
 * 输入连接暂不可用时的延迟重试（OpiImeService.commitWithRetry 的排程部分）。
 *
 * 投递与撤销必须落在**同一处**，所以调度器由字段持有：原先两边都现取
 * `window.window.decorView`，而 IME 窗口在旋转/配置变化时会被整棵重建
 * （见 OpiImeService.setInputView 的长注释）—— 撤销时取到的 decorView 已经不是
 * 投递时那个，`removeCallbacks` 撤不掉挂在旧 root 上的 action，50ms 后它照样执行，
 * 把上一次的候选写进了**新的**编辑框。字段持有的 Handler 不随窗口重建而改变。
 */
class PendingCommit(
    private val debouncer: Debouncer = HandlerDebouncer(),
) {
    private var cancel: (() -> Unit)? = null

    /** 排一次重试；已有排队中的先撤（两次重试叠加 = 同一个词被提交两遍）。 */
    fun schedule(retry: () -> Unit) {
        cancel()
        cancel = debouncer.schedule(RETRY_MS) {
            cancel = null
            retry()
        }
    }

    fun cancel() {
        cancel?.invoke()
        cancel = null
    }

    companion object {
        /** 重试延迟：给窗口拿到 IC 的时间，短到用户察觉不到。 */
        const val RETRY_MS = 50L
    }
}
