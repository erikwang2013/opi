// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.engine.EngineMode
import xyz.erik.opi.engine.ShiftState

/**
 * 按键分流：buffer 非空走引擎，buffer 空走直传（对齐 flutter ime_router.dart）。
 * 通道动作由 OpiImeService 注入（commitWithRetry/deleteBackward/performEnter/
 * syncComposing）。
 */
class KeyRouter(
    val controller: EngineController,
    private val commit: (String) -> Unit,
    private val deleteBackward: () -> Unit,
    private val performEnter: () -> Unit,
    private val syncComposing: () -> Unit = {},
    /** 英文专业模式（默认 true = 保持既有调用方/测试的前缀联想行为；生产注入 [ImeState.englishPro]）。 */
    private val englishPro: () -> Boolean = { true },
) {
    /**
     * 引擎驱动的提交收尾：提交后**重新推一次组合串**（服务侧 = pushComposingText）。
     *
     * 为什么必须有（2026-10-13 用户报障「连按一个键出省略号，确认后把所有内容替换」）：
     * 长缓冲无整词命中时候选层回退到「最长可匹配**前缀**」——选中只消耗那个前缀，
     * **后缀留在引擎缓冲里接着编**（`engine_segment.rs`）。而提交路径的 `dropComposing`
     * 会把「当时的组合串」整段删掉 —— 那一拍组合串显示的恰恰是**剩余缓冲**（例：13 个
     * `6` 选「默默」后，剩余 `666666666` 的预编辑 `mo'mo·····`），于是剩余拼音从字段里
     * 消失（引擎里还在、候选还在）＝ 用户看到的「确认把全部内容替换了」。
     * 提交后立刻重推：字段变成「**已上屏词 + 剩余拼音**（继续匹配）」；下一拍的
     * `dropComposing` 删的就恰好是这条组合串本身 —— 长度天然对齐，不再错位。
     *
     * ⚠️ 只在**引擎驱动**的提交点调用：数字直提与英文**携带缓冲**的直传不推 ——
     * 后者缓冲是「切英文时保留的中文拼音」（`handleKey` 英文分支的裁决），推出去会
     * 把它复活成预编辑；有单测钉住（`englishDirectPathNeverResyncs`）。英文自己的
     * 缓冲走引擎，提交点照常收尾。
     */
    private fun commitAndResync(text: String) {
        commit(text)
        syncComposing()
    }

    /**
     * 目录项直提（符号/表情面板：目录项不是键盘键码，引擎没有对应入口）。
     * **数字面板不走这里** —— 它的 `,` `.` 是键盘键码，走 [handleKey] 由引擎出文本。
     * 面板提交（如「面板打开时提交 pending buffer」）之后同样收尾：此刻缓冲通常已空，
     * 重推 = 清组合区，无副作用。
     */
    fun commitText(text: String) = commitAndResync(text)

    /**
     * 键盘键（字母盘与数字面板共用）。引擎的返回值必须提交：标点键由引擎层标点表
     * 出文本（中文模式 `,` → `，`、半角 → 原样交回），丢弃返回值就等于吞键。
     */
    fun handleKey(ch: String) {
        if (controller.mode == EngineMode.ENGLISH) {
            // 携带缓冲（中→英保留的拼音，2026-10-09 用户裁决「拼音保留可继续使用」）：
            // 保持旧直传 —— 字母进文档、缓冲原样留着切回中文继续用；绝不喂给英文
            // 前缀匹配（否则 `a` 会拼成 `nia`，英文打不出来、拼音也被毁掉）。
            // ⇧ 直传大写、全角由本侧宽化（直传绕过引擎，规则只能镜像，语义源见
            // EngineController.fullwidthize 的头注）。
            if (controller.bufferCarried) {
                if (controller.shiftState != ShiftState.OFF) {
                    commit(controller.fullwidthize(ch.uppercase()))
                    controller.consumeSingleShift()
                } else {
                    commit(controller.fullwidthize(ch))
                }
                return
            }
            // 普通模式（2026-10-19，默认）：字母/数字都直出，**不进引擎、不做单词
            // 匹配**；⇧ 语义与携带缓冲直传一致。专业模式才走下面的词库前缀联想。
            if (!englishPro()) {
                if (controller.shiftState != ShiftState.OFF) {
                    commit(controller.fullwidthize(ch.uppercase()))
                    controller.consumeSingleShift()
                } else {
                    commit(controller.fullwidthize(ch))
                }
                return
            }
            // 英文自有缓冲 + 数字（2026-10-19 英文键盘顶部数字行）：**直接上屏**，
            // 不喂引擎 —— 引擎的英文数字是「选词键」，会把 0-9 吞成候选选择。
            // 全角规则与其它直传点一致。携带缓冲的数字走上一条（语义不变）。
            if (ch.length == 1 && ch[0].isDigit()) {
                commit(controller.fullwidthize(ch))
                return
            }
            // 空缓冲/英文自己的缓冲：进引擎 —— 字母入缓冲、候选 = 前缀匹配的最高频
            // 词（2026-10-18 单词联想；空缓冲不再直传）。⇧ 的 single 用掉即复位。
            val text = controller.input(ch)
            if (ch.length == 1 && ch[0].isLetter()) controller.consumeSingleShift()
            if (text.isNotEmpty()) commitAndResync(text)
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
        if (text.isNotEmpty()) commitAndResync(text)
    }

    fun handleSpace() {
        if (controller.buffer.isNotEmpty() || controller.pendingText.isNotEmpty()) {
            val text = controller.inputSpace()
            if (text.isNotEmpty()) commitAndResync(text)
        } else {
            // 空格**不宽化**（有意）：全角在拼音模式默认开，宽它会把「中文模式按空格」
            // 换成 U+3000 —— 见 engine-core `Engine::fullwidthize` 头注。
            commit(" ")
        }
    }

    fun handleBackspace() {
        if (controller.buffer.isNotEmpty() || controller.pendingText.isNotEmpty()) controller.backspace()
        else deleteBackward()
    }

    /**
     * 长按退格：清空**整段组合输入**（引擎缓冲 + 已确认未提交的逐字片段），不提交任何文本。
     *
     * 缓冲为空时退回与短按相同的**单字删除** —— 绝不在这里做「删光编辑器内容」：
     * 那会把用户已上屏的正文一起删掉（2026-10-13 过量退格的同类事故）。
     *
     * 清空走 [EngineController.clear]：它同时重读候选并触发服务层的组合区收尾
     * （`onBufferSynced` → `pushComposingText`），所以宿主字段里不会留下旧组合串。
     */
    fun handleBackspaceLongPress() {
        val composing = controller.buffer.isNotEmpty() ||
            controller.pendingText.isNotEmpty() ||
            controller.candidates.isNotEmpty()
        if (composing) controller.clear()
        else deleteBackward()
    }

    fun handleEnter() {
        if (controller.buffer.isEmpty() && controller.pendingText.isEmpty()) {
            performEnter()
            return
        }
        if (controller.buffer.isEmpty()) {
            // 逐字态收尾（2026-10-17）：片段还在候选区 —— 回车 = 提交它们（同空格）。
            val text = controller.inputSpace()
            if (text.isNotEmpty()) commitAndResync(text)
            return
        }
        // 判据在引擎（2026-10-14 接线）：五笔读「回车行为」设置位 —— 1=清空、2=上屏原文
        // （读缓冲原样），0/其余 = 选首候选（既有行为）。**本端不自己判**，
        // 见 engine-core `engine_enter.rs`（同一语义抄多份的教训）。
        when (controller.enterAction()) {
            1 -> controller.clear()
            2 -> {
                val raw = controller.buffer
                controller.clear()
                if (raw.isNotEmpty()) commitAndResync(raw)
            }
            else -> {
                val text = controller.select(0)
                if (text.isNotEmpty()) commitAndResync(text)
            }
        }
    }

    /** 屏内下标 → selectFromPage（翻页后的绝对下标）。 */
    fun handleCandidate(indexInPage: Int) {
        val text = controller.selectFromPage(indexInPage)
        if (text.isNotEmpty()) commitAndResync(text)
    }

    /**
     * 平铺候选视图的选词：**全局下标**（该视图不分页序，直接吃 `controller.candidates`
     * 的全量列表 —— 见 `CandidateTiledView` 的接线）。折叠行那条 [handleCandidate]
     * 是页内下标 + pageSize 换算，两条别混。
     */
    fun handleCandidateAbsolute(index: Int) {
        val text = controller.select(index)
        if (text.isNotEmpty()) commitAndResync(text)
    }
}
