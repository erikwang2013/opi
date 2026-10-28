// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

//! 假引擎（2026-10-09 自 EngineControllerTest.kt 拆出：加「读数/缓冲两出口」用例后
//! 源文件 506 行越了 500 行门禁）。**同包**：引用方（KeyRouterTest 等）不用改 import。

/**
 * 假引擎：模拟 Rust 引擎状态（buffer/candidates/mode/shift），JVM 测试无需 JNI。
 *
 * ⚠️ `buf` 缺省空却可以直接给 `cands` —— 真引擎里「空缓冲 + 候选」只属于联想态，
 * 而联想门禁默认关（[AssociationGate]）⇒ refresh 会清空这些候选。非联想用例要在
 * `@Before` 里开 `AssociationGate.enabled`（见 EngineControllerTest）。
 */
class FakeEngine : OpiEngineApi {
    var buf = ""
    var mode = EngineMode.PINYIN.value
    var cands: Array<String>? = null
    var lastLimit = -1

    /** 每页候选数（真引擎：`router::PAGE_SIZE`，由 JNI `pageSize()` 读）。缺省 8 = 生产值。 */
    var pageSizeValue = 8
    override fun pageSize(): Int = pageSizeValue
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

    /** 英文短语可命中（真引擎：开关开 + 表非空）。 */
    var englishActive = false
    override fun englishPhrasesActive(): Boolean = englishActive

    /** 引擎侧 ⇧（真引擎 composer 英文模式按它决定大小写；single 由调用方消费）。 */
    var shiftOn = false

    override fun inputKey(ch: String): String {
        inputCalls += ch
        val out = inputResults[ch] ?: ""
        if (out.isEmpty()) {
            val text = if (shiftOn && ch.length == 1 && ch[0].isLetter()) ch.uppercase() else ch
            buf += text
        } else {
            buf = ""
        }
        return out
    }

    override fun backspace() {
        backspaceCalls++
        if (buf.isNotEmpty()) buf = buf.dropLast(1)
        if (buf.isEmpty()) carriedFrom = null
    }

    override fun clear() {
        clearCalls++
        buf = ""
        carriedFrom = null
        cands = null // 真引擎的候选由缓冲派生：清缓冲 ⇒ 候选空
        // 逐字片段与显示读数同属组合态（真引擎的 clear 一并清掉）：
        // 长按退格「清空整段组合」的用例要靠它验证，不留假残留。
        pending = ""
        displayBuf = null
    }

    /** 回车判据（2026-10-14）：测试按需设 0/1/2 —— 缺省 0 = 既有行为（选首候选）。 */
    var enterActionValue = 0
    var enterActionCalls = 0

    override fun enterAction(): Int {
        enterActionCalls++
        return enterActionValue
    }

    /** 编码提示（2026-10-14）：测试按需设；缺省空串。 */
    var wubiNextKeysValue = ""
    var wubiNextKeysCalls = 0

    override fun wubiNextKeys(): String {
        wubiNextKeysCalls++
        return wubiNextKeysValue
    }

    /** 每次 select 的全局下标（钉 selectFromPage 的 page*size+k 换算）。 */
    val selectCalls = mutableListOf<Int>()

    override fun select(index: Int): String {
        selectCalls += index
        val r = selectResults[index] ?: ""
        if (r.isNotEmpty()) buf = ""
        return r
    }

    override fun switchMode(mode: Int) {
        switchCalls += mode
        this.mode = mode
        shiftOn = false // 真引擎：composer.switch_mode 清 shift（前端三态要重推）
    }

    override fun setShift(on: Boolean) {
        shiftCalls += on
        shiftOn = on
        // 真引擎 set_shift 清联想（2026-10-27 Task 12）；空缓冲时候选只来自联想。
        if (buf.isEmpty()) cands = null
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

    /** 携带标记（真引擎：`Engine::buffer_is_carried`；语义逐条镜像）。 */
    var carriedFrom: EngineMode? = null
    override fun bufferIsCarried(): Boolean = carriedFrom != null

    /** 语言切换（真引擎：换模式+方案但保留缓冲）。记调用便于断言「没走清缓冲那条路」。 */
    val switchLanguageCalls = mutableListOf<Pair<Int, Int>>()
    override fun switchLanguage(mode: Int, scheme: Int) {
        switchLanguageCalls += mode to scheme
        val old = EngineMode.fromInt(this.mode)
        val next = EngineMode.fromInt(mode)
        carriedFrom = when {
            buf.isEmpty() -> null
            carriedFrom == next -> null
            carriedFrom == null && next != old -> old
            else -> carriedFrom
        }
        this.mode = mode
        schemeValue = scheme
        shiftOn = false // 真引擎：switch_mode_keep_buffer 同样清 shift
    }

    /** 候选拼音读数（真引擎：与最近一次 candidates() 同源；出口已按读音去重保序）。 */
    var candReadings: Array<String>? = null

    /** 读出口被问了几次（钉「非 9/14/双拼不调 JNI」这条省法）。 */
    var candidateReadingsCalls = 0
        private set

    override fun candidateReadings(): Array<String>? {
        candidateReadingsCalls++
        return candReadings
    }

    /** 钉读音（真引擎：候选收窄到**读音恰等**的条目；空串 = 解除）。 */
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

    /** 已确认的逐字片段（2026-10-17）：真引擎里它已前置进 displayBuffer；
     *  假引擎分开存，测试按需设（「·」覆盖率换算要用它的长度）。 */
    var pending = ""

    override fun pendingText(): String = pending

    override fun mode(): Int = mode

    override fun learnerEnabled(): Boolean = learnerOn

    override fun fuzzyEnabled(): Boolean = fuzzyOn

    /** 逐对掩码（2026-10-13）：缺省全开，与真引擎 `FUZZY_ALL` 同值。 */
    var fuzzyPairsValue: Int = 0xFF

    override fun fuzzyPairs(): Int = fuzzyPairsValue

    /** 拼音纠错开关（2026-10-13）：假侧缺省开（同真引擎缺省）。 */
    var correctionOn: Boolean = true

    override fun correctionEnabled(): Boolean = correctionOn

    override fun setCorrection(on: Boolean) {
        correctionOn = on
    }

    override fun setFuzzyPairs(mask: Int) {
        fuzzyPairsValue = mask and 0xFF
    }

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

    /** 双拼布局（真引擎：shuangpinLayout()/setShuangpinLayout()；缺省 0=自然码）。 */
    var shuangpinLayoutValue = 0

    override fun shuangpinLayout(): Int = shuangpinLayoutValue

    override fun setShuangpinLayout(index: Int) {
        shuangpinLayoutValue = index
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
