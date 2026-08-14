package io.opi.input.ime

import io.opi.input.engine.EngineController
import io.opi.input.engine.EngineMode
import io.opi.input.engine.ShiftState

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
    /** 面板提交统一入口（数字/符号/表情直传，不经引擎）。 */
    fun commitText(text: String) = commit(text)

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
        controller.input(ch)
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
