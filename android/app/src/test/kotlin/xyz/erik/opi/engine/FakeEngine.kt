// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

//! 假引擎（2026-10-09 自 EngineControllerTest.kt 拆出：加「读数/缓冲两出口」用例后
//! 源文件 506 行越了 500 行门禁）。**同包**：引用方（KeyRouterTest 等）不用改 import。

/** 假引擎：模拟 Rust 引擎状态（buffer/candidates/mode/shift），JVM 测试无需 JNI。 */
class FakeEngine : OpiEngineApi {
    var buf = ""
    var mode = EngineMode.PINYIN.value
    var cands: Array<String>? = null
    var lastLimit = -1
    val shiftCalls = mutableListOf<Boolean>()
    val inputCalls = mutableListOf<String>()
    var spaceResult = ""
    val selectResults = mutableMapOf<Int, String>()
    var backspaceCalls = 0
    var clearCalls = 0
    val switchCalls = mutableListOf<Int>()
    val removed = mutableListOf<String>()
    var throwOnRemove: Throwable? = null

    /** 学习开关（真引擎：learnerEnabled()/setLearner()）。 */
    // ⚠️ 名字带 On 后缀：`var learner` 的属性 setter 在 JVM 上与接口方法
    // `setLearner(Z)` **签名相同**（Platform declaration clash），改名是唯一出路。
    var learnerOn = true

    /** 「属性设置」的两个新开关（真引擎：fuzzyEnabled()/jianpinEnabled()）。 */
    var fuzzyOn = true
    var jianpinOn = true

    /** 键 → 引擎要上屏的文本（真引擎：标点键有文本、字母/数字键没有）。 */
    var inputResults = mutableMapOf<String, String>()

    override fun inputKey(ch: String): String {
        inputCalls += ch
        val out = inputResults[ch] ?: ""
        if (out.isEmpty()) buf += ch else buf = ""
        return out
    }

    override fun backspace() {
        backspaceCalls++
        if (buf.isNotEmpty()) buf = buf.dropLast(1)
    }

    override fun clear() {
        clearCalls++
        buf = ""
    }

    override fun select(index: Int): String {
        val r = selectResults[index] ?: ""
        if (r.isNotEmpty()) buf = ""
        return r
    }

    override fun switchMode(mode: Int) {
        switchCalls += mode
        this.mode = mode
    }

    override fun setShift(on: Boolean) {
        shiftCalls += on
    }

    override fun inputSpace(): String {
        if (spaceResult.isNotEmpty()) buf = ""
        return spaceResult
    }

    override fun candidates(limit: Int): Array<String>? {
        lastLimit = limit
        // 真引擎契约（engine.rs 的 scheme_reading 缓存）：读数是取候选的**副产物**，
        // 且与算它时的缓冲绑存 —— 缓冲一变即作废（报空串）。启用 model 时按此模拟。
        readingModel?.let { cachedReading = if (buf.isEmpty()) "" else it(buf) }
        return cands
    }

    override fun buffer(): String = buf

    /** 语言切换（真引擎：换模式+方案但保留缓冲）。记调用便于断言「没走清缓冲那条路」。 */
    val switchLanguageCalls = mutableListOf<Pair<Int, Int>>()
    override fun switchLanguage(mode: Int, scheme: Int) {
        switchLanguageCalls += mode to scheme
        this.mode = mode
        schemeValue = scheme
    }

    /** 候选拼音读数（真引擎：与最近一次 candidates() 同源；出口已按读音去重保序）。 */
    var candReadings: Array<String>? = null
    override fun candidateReadings(): Array<String>? = candReadings

    /** 钉读音（真引擎：候选收窄到该读音一族；空串 = 解除）。 */
    val pinCalls = mutableListOf<String>()
    override fun pinReading(reading: String) {
        pinCalls += reading
    }

    /** 显示读数（真引擎：9 键 = 带分词的读音）。默认与 buf 同步，测试要区分时单设。 */
    var displayBuf: String? = null

    /**
     * 读数真契约的模型（buf → 读数），null = 不启用（老用例行为不变）。
     * 启用后 [displayBuffer] 只报「上一次 [candidates] 时按当时缓冲算出的」读数 ——
     * 专供钉 `refresh()` 读序的用例（先读读数再取候选 = 拿到旧值/空串）。
     */
    var readingModel: ((String) -> String)? = null
    private var cachedReading = ""

    override fun displayBuffer(): String =
        displayBuf ?: (if (readingModel != null) cachedReading else buf)

    override fun mode(): Int = mode

    override fun learnerEnabled(): Boolean = learnerOn

    override fun fuzzyEnabled(): Boolean = fuzzyOn

    override fun jianpinEnabled(): Boolean = jianpinOn

    override fun setLearner(enabled: Boolean) {
        learnerOn = enabled
    }

    override fun setFuzzy(on: Boolean) {
        fuzzyOn = on
    }

    override fun setJianpin(on: Boolean) {
        jianpinOn = on
    }

    /** 输入方案（真引擎：scheme()/setScheme()）。 */
    var schemeValue = InputScheme.FULL.value
    val schemeCalls = mutableListOf<Int>()

    override fun scheme(): Int = schemeValue

    override fun setScheme(scheme: Int) {
        schemeCalls += scheme
        this.schemeValue = scheme
    }

    /** 全角开关（真引擎：fullwidthState()/toggleFullwidth()；切模式重置是引擎的事）。 */
    var fullwidthValue = false

    override fun fullwidthState(): Boolean = fullwidthValue

    override fun toggleFullwidth(): Boolean {
        fullwidthValue = !fullwidthValue
        return fullwidthValue
    }

    /** 删用户词：记录文本；真引擎删完候选表里就没有它了（这里同样摘掉）。 */
    override fun removeUserWord(text: String) {
        throwOnRemove?.let { throw it }
        removed += text
        cands = cands?.filter { it != text }?.toTypedArray()
    }
}
