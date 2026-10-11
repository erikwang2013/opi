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
            // ⇧ 直传大写：直传路径绕过引擎，需本侧转大写；全角由本侧宽化
            // （2026-10-12 用户裁决「字母、数字也要」——直传绕过引擎，规则只能镜像，
            // 语义源见 EngineController.fullwidthize 的头注）。
            if (controller.shiftState != ShiftState.OFF) {
                commit(controller.fullwidthize(ch.uppercase()))
                controller.consumeSingleShift()
            } else {
                commit(controller.fullwidthize(ch))
            }
            return
        }
        // 数字面板（Number 模式）：**数字点一下立即上屏**（2026-10-10 用户报障
        // 「切换到数字键盘点击数字没输入，再切回拼音才出来」——引擎 Number 模式原为
        // 「数字攒进缓冲、收尾时原样提交」（composer.rs 的 Mode::Number 分支），在
        // 数字面板上表现为点了没反应）。数字直提零损失（标点档只管标点，字母数字
        // 在引擎里不映射）；全角开时由本侧宽化（数字同属直提路径）—— `,`/`.` 仍走
        // 引擎，标点表与全角那两档在它们身上生效。
        // ⚠️ 引擎侧的 Number 缓冲语义不动（那是引擎自己的设计），只是本面板不再用它。
        if (controller.mode == EngineMode.NUMBER && ch.length == 1 && ch[0].isDigit()) {
            commit(controller.fullwidthize(ch))
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
            // 空格**不宽化**（有意）：全角在拼音模式默认开，宽它会把「中文模式按空格」
            // 换成 U+3000 —— 见 engine-core `Engine::fullwidthize` 头注。
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
