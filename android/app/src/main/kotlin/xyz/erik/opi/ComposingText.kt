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
import xyz.erik.opi.engine.composingText

/**
 * 组合串进字段（2026-10-10 用户裁决「需要」）：打字期间输入框里能看到拼音
 * （搜狗式）——此前组合串只画在自家候选栏，字段里空空；用户以为没打进去。
 * 文本用**显示读数**（`reading`：9 键 = `ni'hao` 这类分词拼音；其余 = 原字母）。
 * 英文模式字母直传不经过缓冲 ⇒ 无组合串，正确。读数空时的回退见
 * [EngineController.composingText]（9 键不回退数字缓冲，2026-10-11 修）。
 *
 * ⚠️ 缓冲空时**先 `setComposingText("")` 再 `finishComposingText`** —— 只 finish
 * 会把组合区里**最后那段文字落定留在字段里**（退格到空会剩半截拼音）。
 */
internal fun OpiImeService.pushComposingText() {
    val ic = currentInputConnection ?: return
    val text = engineController.composingText()
    if (text.isEmpty()) {
        ic.setComposingText("", 1)
        ic.finishComposingText()
        lastComposing = ""
    } else {
        ic.setComposingText(text, 1)
        lastComposing = text
    }
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
    if (lastComposing.isEmpty()) return
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
        ic.deleteSurroundingTextInCodePoints(lastComposing.length, 0)
    } else {
        ic.deleteSurroundingText(lastComposing.length, 0)
    }
    lastComposing = ""
}
