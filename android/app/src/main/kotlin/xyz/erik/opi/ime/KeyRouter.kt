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
        // 英文模式**一律直传**（2026-10-09 二期）：语言切换不再清/提交缓冲 ⇒ 英文模式下
        // 缓冲可能**被保留**着（用户裁决「中文输入的时候切换英文，拼音保留可继续使用」）。
        // 那种状态里字母必须照打字面英文直传，绝不进引擎缓冲 —— 否则英文打不进去、
        // 全喂给了保留的旧拼音。保留的拼音等切回中文继续用（见 ImeState.cycleMode）。
        // 此前只在缓冲空时直传，非空走引擎（旧防御路径）；判据随语言切换语义一起改了。
        if (controller.mode == EngineMode.ENGLISH) {
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

    /**
     * 点 9 键左列第 i 行拼音 = 上屏第 i 条候选（2026-10-09 用户裁决「拼音要可选」
     * —— 落地为「点拼音 = 选那个词上屏」）。**全量**下标 —— 读数列是跨页的全量列表，
     * 与候选行的页内下标（[handleCandidate]）不是一个坐标系；全局 select 的下标空间
     * 与 candidates() 同源（engine.rs 的 `select` 门禁钉着），读数第 i 行即第 i 条。
     */
    fun handleReading(index: Int) {
        val text = controller.select(index)
        if (text.isNotEmpty()) commit(text)
    }
}
