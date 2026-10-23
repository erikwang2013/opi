// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

//! 引擎接口契约（2026-10-14 从 EngineController.kt 拆出：加双拼布局两方法后那个
//! 文件 517/500 越 500 行门禁）。同包移动，零引用变化 —— 拆出的理由是「接口是
//! 契约、实现是状态机」，两者本就不该挤在一个文件里。
//
// ⚠️ 接口里的每个方法都对应 JNI 侧一个 `OpiEngine` 出口（签名面由 jni_contract
// 门禁钉）；方法上的注释是**语义契约**（哨兵值、重读规则、清不清缓冲），别在实现
// 侧复述 —— 两处写必漂。

/**
 * 引擎接口：EngineController 依赖抽象，JVM 测试用假引擎替换 JNI 实现
 * （OpiEngine 的 load/searchSymbols/symbolBlocks/symbolsInBlock 为面板/资产专用，不入接口）。
 */
interface OpiEngineApi {
    fun inputKey(ch: String): String
    fun backspace()
    fun clear()
    fun select(index: Int): String

    /** 回车（缓冲非空时）该做什么：0=选首候选、1=清空、2=上屏原文（2026-10-14 接线；
     * 判据在引擎侧，见 `engine-core/src/engine_enter.rs`）。 */
    fun enterAction(): Int

    /** 编码提示（2026-10-14 接线）：当前缓冲可接续的键，拼成串；非五笔态 / 无接续 → 空串。
     * ⚠️ 引擎侧每次调用全量重算（O(26×前缀查询)）—— **只在缓冲变化时取一次**
     * （[EngineController.refresh] 里），别在每帧渲染里调。 */
    fun wubiNextKeys(): String
    fun switchMode(mode: Int)
    fun setShift(on: Boolean)
    fun inputSpace(): String
    fun candidates(limit: Int): Array<String>?
    fun buffer(): String

    /** 缓冲是否由**别的模式**带过来（语言切换保留缓冲时打标；清空/提交/回到来处即清）。
     * 英文分支的直传判据：携带缓冲直传、空缓冲或英文自己的缓冲进引擎（见 KeyRouter）。 */
    fun bufferIsCarried(): Boolean

    /** 候选拼音读数（**按读音去重保序**，2026-10-12 用户裁决「左列去重」；
     * 与最近一次 [candidates] 同源同一次排名；9 键左列用）。
     * 非 9/14 键方案、或缓冲已变 ⇒ 空。调用顺序：先 [candidates]。 */
    fun candidateReadings(): Array<String>?

    /** 钉读音（空串 = 解除）：9 键左列点一行拼音后**不确认**，候选收窄到**读音恰等**的
     * 条目（2026-10-16 裁决）。缓冲一变自动作废（引擎侧按缓冲配对校验；调用方还须在
     * 查看守里显式清）。 */
    fun pinReading(reading: String)

    /** 显示用的拼音读数（9 键 = 带分词的 `ni'hao`；其余方案与 [buffer] 相同）。
     * **行为判定不许用它** —— 用 [buffer]（理由见 OpiEngine.displayBuffer）。 */
    fun displayBuffer(): String

    /** 已确认但尚未提交的逐字片段（2026-10-17 逐字确认；显示串里已含，这里要长度）。 */
    fun pendingText(): String
    fun mode(): Int

    /** 英文自定义短语是否**实际可命中**（开关开且表非空，2026-10-24）。
     * 普通英文模式据此改走引擎缓冲；缺省 false 让既有假引擎不用改（生产实现是 JNI 出口）。 */
    fun englishPhrasesActive(): Boolean = false

    /** 删除一个用户词（连同频次）；非用户词/不存在无副作用。 */
    fun removeUserWord(text: String)

    /** 学习开关读侧。宠物用它区分「醒着」与「睡着了」。 */
    fun learnerEnabled(): Boolean

    /** 学习开关写侧（[EngineController.setLearner] 与偏好恢复共用）。 */
    fun setLearner(enabled: Boolean)

    /** 模糊音开关读侧（「属性设置」；写侧 = `OpiEngine.setFuzzy`）。 */
    fun fuzzyEnabled(): Boolean

    /** 模糊音**逐对**启用集（2026-10-13 用户裁决「可多选类似 z=zh」）。
     * 位序 = 引擎 `FUZZY_PAIRS` 数组序（zh/z=bit0 … f/h=bit7；`0xFF` = 全开）；
     * `0` = 全关。未装载 → 0（哨兵，同 [fuzzyEnabled] 一档）。 */
    fun fuzzyPairs(): Int

    /** 逐对写侧（[EngineController.setFuzzyPairs] 与偏好恢复共用）。 */
    fun setFuzzyPairs(mask: Int)

    /** 简拼开关读侧（同上）。 */
    fun jianpinEnabled(): Boolean

    /** 拼音纠错开关读侧（2026-10-13；写侧 = `OpiEngine.setCorrection`）。 */
    fun correctionEnabled(): Boolean

    /** 写侧（[EngineController.setFuzzy] / [EngineController.setJianpin] 与偏好恢复共用）。 */
    fun setFuzzy(on: Boolean)
    fun setJianpin(on: Boolean)

    /** 拼音纠错写侧（2026-10-13）。 */
    fun setCorrection(on: Boolean)

    /** 输入方案读侧（「输入模式」；写侧见 [EngineController.switchScheme]）。 */
    fun scheme(): Int

    /** 双拼布局读侧（0=自然码…5=拼音加加；写侧见 [EngineController.setShuangpinLayout]）。
     * 未装载 → 0（哨兵，同 [scheme] 一档）。 */
    fun shuangpinLayout(): Int

    /** 双拼布局写侧（2026-10-13；**不清缓冲**，与 [setScheme] 的区别）。 */
    fun setShuangpinLayout(index: Int)

    /** 全角开关读侧（**每次切模式后必须重读**，见 [EngineController.fullwidth] 的注释）。 */
    fun fullwidthState(): Boolean

    /** 全角↔半角Toggle（工具条月相按钮）；返回切完的新状态。 */
    fun toggleFullwidth(): Boolean

    /** 写侧：换方案**清缓冲** —— 调用方随后必须重读（[EngineController.switchScheme] 管这条）。 */
    fun setScheme(scheme: Int)

    /** 语言切换（中↔英/中↔繁）：换模式与方案但**保留缓冲**（判据见
     * OpiEngine.switchLanguage）。与 [switchMode]/[setScheme] 的区别只有不清缓冲 ——
     * 那两个仍归「输入模式」面板等明确要换方案的路径。 */
    fun switchLanguage(mode: Int, scheme: Int)
}
