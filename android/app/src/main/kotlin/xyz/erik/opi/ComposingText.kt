// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 组合串进字段的两个助手（2026-10-13 从 `OpiImeService.kt` **搬出** —— 那边撞 500 行
//! 门禁；**扩展函数**，本仓同惯例：它们读的就是 service 的三个成员
//! （`currentInputConnection` / `engineController` / `lastComposing`），语义上仍是那个
//! 类的一部分。内容一字未改；后两个成员随之升成 `internal`（拆分裂代价，同 Rust 侧
//! `engine_readings.rs` 等拆出时的可见性注记）。

package xyz.erik.opi

import android.os.Build
import android.view.inputmethod.InputConnection

/**
 * 组合串收尾（**2026-10-15 用户裁决改判**：拼音不再进输入框 —— 它画在候选行上方
 * 那一行里，见 `ime/ComposingBar.kt`。本条原先的语义「打字期间输入框里能看到拼音」
 * 是 2026-10-10 的裁决，已被取代；`docs/install/android.md` 有订正留痕）。
 *
 * 现在这条只做一件事：**把可能残留的组合区收干净**（防御性 —— 正常路径下我们从不
 * 往里写，`lastComposing` 恒空；但别的输入法切走留下的、或本端历史版本写进去的组合区
 * 不该在提交后忽然落定成普通文本）。
 * ⚠️ 收尾仍要**先 `setComposingText("")` 再 `finishComposingText`** —— 只 finish 会把
 * 组合区里最后那段文字**落定留在字段里**（这是 2026-10-10 实测到的形态）。
 */
internal fun OpiImeService.pushComposingText() {
    val ic = currentInputConnection ?: return
    if (lastComposing.isEmpty()) return
    ic.setComposingText("", 1)
    ic.finishComposingText()
    lastComposing = ""
}

/**
 * 提交前把上次推的组合串从字段里抹掉（2026-10-10 实测「ni'hao你好」的修法）。
 *
 * 次序成因：选词路径是「引擎 select（缓冲清空）→ refresh（组合区判空 ⇒ 落定成普通
 * 文本）→ 调用方 commitText」——commitText 到得比组合区落定晚，替换不了它，只能追加，
 * 字段里留下拼音痕迹。先把 lastComposing 那串删掉，**两种状态都得到「替换」的净效果**
 * （活跃时删的就是组合区、已落定时删的是刚落定的那串）。删除按码点（同退格一条纪律）。
 */
internal fun OpiImeService.dropComposing(ic: InputConnection) {
    // 2026-10-27：任何提交都会触发 onUpdateSelection，先记「这是自我提交」，
    // 否则选区变化会把刚建好的联想收掉。必须在早退之前（正常路径 lastComposing 恒空）。
    selfCommitGuard.noteSelfCommit()
    if (lastComposing.isEmpty()) return
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
        ic.deleteSurroundingTextInCodePoints(lastComposing.length, 0)
    } else {
        ic.deleteSurroundingText(lastComposing.length, 0)
    }
    lastComposing = ""
}
