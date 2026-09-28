// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.engine.EngineMode
import xyz.erik.opi.engine.ShiftState

/**
 * 按键分流：buffer 非空走引擎，buffer 空走直传（对齐 flutter ime_router.dart）。
 * 通道动作由 OpiImeService 注入（commitWithRetry/deleteBackward/performEnter）。
 */
class KeyRouter(
    val controller: EngineController,
    private val commit: (String) -> Unit,
    private val deleteBackward: () -> Unit,
    private val performEnter: () -> Unit,
) {
    /**
     * 目录项直提（符号/表情面板：目录项不是键盘键码，引擎没有对应入口）。
     * **数字面板不走这里** —— 它的 `,` `.` 是键盘键码，走 [handleKey] 由引擎出文本。
     */
    fun commitText(text: String) = commit(text)

    /**
     * 键盘键（字母盘与数字面板共用）。引擎的返回值必须提交：标点键由引擎层标点表
     * 出文本（中文模式 `,` → `，`、半角 → 原样交回），丢弃返回值就等于吞键。
     */
    fun handleKey(ch: String) {
        if (controller.mode == EngineMode.ENGLISH && controller.buffer.isEmpty()) {
            // ⇧ 直传大写：直传路径绕过引擎，需本侧转大写。
            if (controller.shiftState != ShiftState.OFF) {
                commit(ch.uppercase())
                controller.consumeSingleShift()
            } else {
                commit(ch)
            }
            return
        }
        val text = controller.input(ch)
        if (text.isNotEmpty()) commit(text)
    }

    fun handleSpace() {
        if (controller.buffer.isNotEmpty()) {
            val text = controller.inputSpace()
            if (text.isNotEmpty()) commit(text)
        } else {
            commit(" ")
        }
    }

    fun handleBackspace() {
        if (controller.buffer.isNotEmpty()) controller.backspace()
        else deleteBackward()
    }

    fun handleEnter() {
        if (controller.buffer.isNotEmpty()) {
            val text = controller.select(0)
            if (text.isNotEmpty()) commit(text)
        } else {
            performEnter()
        }
    }

    /** 屏内下标 → selectFromPage（翻页后的绝对下标）。 */
    fun handleCandidate(indexInPage: Int) {
        val text = controller.selectFromPage(indexInPage)
        if (text.isNotEmpty()) commit(text)
    }
}
