// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import android.os.SystemClock
import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.engine.associationActive

/**
 * 自我提交守卫（Spec §5.2）：区分「我们提交文本引起的选区变化」与「用户/应用引起的
 * 选区变化」。后者要收起联想；前者不能 —— 提交后引擎刚建好联想，而 InputConnection
 * 的 `onUpdateSelection` 就在提交后同一拍回调，收掉会让联想永远闪一下即消失。
 *
 * 窗口 200ms（含边界）：连续提交各自续窗（每次 [noteSelfCommit] 覆盖起点）。
 * 从 `OpiImeService` 搬出（那边恰 500 行；语义仍是 service 的一部分）。
 */
internal class ImeCommitGuard(
    private val windowMs: Long = 200,
    private val now: () -> Long = { SystemClock.uptimeMillis() },
) {
    /** 选区非空（`onUpdateSelection` 缓存，2026-10-26）：⌫ 不再每次 getSelectedText。 */
    internal var selectionNonEmpty = false

    private var lastSelfCommit: Long? = null

    fun noteSelfCommit() {
        lastSelfCommit = now()
    }

    fun inSelfCommitWindow(): Boolean {
        val t = lastSelfCommit ?: return false
        return now() - t <= windowMs
    }
}

/**
 * 选区变化：先更新 ⌫ 的选区缓存；非自我提交窗口内且处于联想态时收起联想
 * （用户挪光标/点别处即放弃这条联想，Spec §5.2）。
 */
internal fun ImeCommitGuard.onSelectionChanged(
    ctrl: EngineController,
    newSelStart: Int,
    newSelEnd: Int,
) {
    selectionNonEmpty = newSelStart >= 0 && newSelEnd >= 0 && newSelStart != newSelEnd
    if (!inSelfCommitWindow() && ctrl.associationActive) ctrl.clear()
}
