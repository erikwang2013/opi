// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import xyz.erik.opi.jni.OpiEngine

/** 引擎模式（JNI mode() 返回值：0=Pinyin 1=English 2=Number 3=Symbol 4=Traditional）。 */
enum class EngineMode(val value: Int) {
    PINYIN(0), ENGLISH(1), NUMBER(2), SYMBOL(3), TRADITIONAL(4);

    companion object {
        fun fromInt(v: Int) = entries.firstOrNull { it.value == v } ?: PINYIN
    }
}

/** ⇧ 状态：off / single（下个字母大写后自动复位）/ lock（持续大写）。 */
enum class ShiftState { OFF, SINGLE, LOCK }

/**
 * 输入方案（JNI `scheme()` 返回值）：0=全键盘 1=拼音9键 2=拼音14键 3=笔画 4=双拼 5=五笔。
 *
 * 「输入模式」= 方案 × 模式的组合（拼音9键 = NINE + PINYIN）；繁体模式是**模式**轴上的
 * 事（[EngineMode.TRADITIONAL]），与方案正交 —— 9 键也能切繁体。
 * 双拼的**布局**（自然码/微软/…）在 [OpiEngineApi.shuangpinLayout] 上，与本枚举正交。
 * ⚠️ 整数表是**协议**：opi-ffi 的 `scheme_from_int`、`OpiEngine.kt` 的注释、
 * 这里三处逐值对应（jni_contract 只保签名不保语义）。
 */
enum class InputScheme(val value: Int) {
    FULL(0), NINE(1), FOURTEEN(2), STROKE(3), SHUANGPIN(4), WUBI(5);

    companion object {
        fun fromInt(v: Int) = entries.firstOrNull { it.value == v } ?: FULL
    }
}

// 引擎接口（[OpiEngineApi]）在 OpiEngineApi.kt —— 同包契约文件，500 行门禁拆出。

/**
 * 单一状态源：封装 OpiEngine，UI 经 mutableStateOf 订阅
 * （翻页 8/页、buffer 变化重置页码；2026-10-26 起 fetchLimit=512，让「壹」这类
 * 排在第 79/155 位的精确单字可通过翻页到达 —— 此前 64 条只有 8 页）。
 */
class EngineController(
    private val api: OpiEngineApi = OpiEngine,
    /**
     * 每次 [refresh] 收尾（组合串状态的单一同步点）后回调：服务层借此把组合串
     * 推给输入框（setComposingText/finishComposingText，2026-10-10 用户裁决「需要」）。
     * 放在**最后**：此刻 buffer/reading 已就位。默认空实现（JVM 测试不感知）。
     * ⚠️ 参数顺序：它排在 onLearned **之前** —— 既有测试/调用方大量使用
     * `EngineController(fake) { learned++ }` 的**尾随 lambda**（绑最后一个参数），
     * 把新参数放最后会把那些 lambda 静默改绑到这里（8 个测试当场红，改序即修）。
     */
    private val onBufferSynced: () -> Unit = {},
    /** 学习状态变更（选词/删用户词）后的落盘通知；默认空实现（JVM 测试/无持久化场景不感知）。 */
    private val onLearned: () -> Unit = {},
) {
    var buffer by mutableStateOf("")
        private set

    /**
     * 缓冲是否由**别的模式**带过来（引擎真值，随 [refresh] 重读）：`switchLanguage`
     * 保留缓冲时打标，清空/提交/回到来处即清。英文分支的直传判据（见 KeyRouter）：
     * 携带缓冲保持旧直传，空缓冲或英文自己的缓冲进引擎。
     */
    var bufferCarried by mutableStateOf(false)
        private set

    /**
     * 显示用的拼音读数（9 键方案的左列 + 读屏播报用它；见 [OpiEngineApi.displayBuffer]）。
     * **只给显示**：判「缓冲有没有东西」一律用 [buffer]（[buffer] 是数字串，不能当空判据）。
     */
    var reading by mutableStateOf("")
        private set

    var mode by mutableStateOf(EngineMode.PINYIN)
        private set
    var candidates by mutableStateOf<List<String>>(emptyList())
        private set

    /** 候选拼音读数（已分词），**按读音去重保序**（首现序 = 该读音最好那条候选的
     * 名次）—— 9 键左列用（2026-10-09 用户裁决「一列多个、可滑动查看」；
     * 2026-10-12 用户裁决「左列去重」，去重发生在引擎出口）。 */
    var candidateReadings by mutableStateOf<List<String>>(emptyList())

    /** 已确认但未提交的逐字片段（2026-10-17）：显示串里已含（displayBuffer 前置）；
     *  2026-10-23 起只供读屏/状态对照，不再参与「·」覆盖率换算（引擎已补全显示串）。 */
    var pendingText by mutableStateOf("")
        private set

    /** 钉读音（9 键左列点一行拼音；空串 = 未钉）。候选收窄到**读音恰等**的条目、**不上屏**。 */
    var pinnedReading by mutableStateOf("")
        private set

    /** 钉它时的缓冲快照（缓冲一变即解除，见 [refresh] 的看守）。 */
    private var pinBuffer: String? = null

    var shiftState by mutableStateOf(ShiftState.OFF)
        private set

    /**
     * 学习开关（引擎真值，随 [refresh] 重读）。
     *
     * 宠物在候选栏待命位用它区分 IDLE/SLEEPY：设置页把开关一关，IME 那边的小欧不能再
     * 笑嘻嘻地装作在记事（[xyz.erik.opi.pet.petMood] 的 SLEEPY 就是为这件事存在的）。
     * 之所以走这里而不是让 UI 直接读 OpiEngine：引擎状态一律经本类订阅（单一状态源）。
     */
    var learnerEnabled by mutableStateOf(true)
        private set

    /** 模糊音 / 简拼开关（引擎真值，随 [refresh] 重读；设置页写、候选栏读）。 */
    var fuzzyEnabled by mutableStateOf(true)
        private set
    var jianpinEnabled by mutableStateOf(true)
        private set

    /**
     * 全角开关（引擎真值，随 [refresh] 重读；工具条月相按钮切）。
     * ⚠️ **必须在 refresh 里重读**：切模式把它重置为该模式默认值（拼音/繁体全角、
     * 英文/数字/符号半角）—— 与 `OpiEngine.fullwidthState` 头注同一条理由。
     */
    var fullwidth by mutableStateOf(false)
        private set

    /**
     * 输入方案（引擎真值，随 [refresh] 重读）。键盘按它换布局、路由器按它分流。
     */
    var inputScheme by mutableStateOf(InputScheme.FULL)
        private set

    /**
     * 词库降级：装载失败，引擎正跑在内置 35 词库上（[xyz.erik.opi.pet.petMood] 据此
     * 画折断的天线 DEGRADED）。
     *
     * 与 [learnerEnabled] 有一处**刻意的不同**：这个状态**没有可重读的源**——引擎没有
     * 对应的 JNI 出口，真值只存在于装载那一刻的返回值里（`EngineLoader.load(context)`
     * 的 Boolean），所以 [refresh] **不重读也不重置它**，只能由宿主回报一次。
     * 别把 `dictionaryDegraded = false` 挪进 refresh：那样下一次按键（每次输入都
     * refresh）就会把降级抹掉，宠物只闪一帧断天线。
     *
     * 边界：false 只说明**词库**这一层没出问题。so 缺失/ABI 不匹配是另一个失败域，
     * 那种情形 EngineLoader 根本没跑（见它头注），本状态表示不了。
     */
    var dictionaryDegraded by mutableStateOf(false)
        private set

    /**
     * 回报一次词库装载结果，参数即 `EngineLoader.load(context)` 的返回值。
     * 后者为 false 就是降级（资产缺失/损坏/写盘失败 → 回退内置词库）。
     * 可重复调用，以最近一次为准（装载幂等，成功即意味着引擎已换回完整词库）。
     */
    fun reportDictionaryLoad(ok: Boolean) {
        dictionaryDegraded = !ok
    }

    companion object {
        const val pageSize = 8
        const val fetchLimit = 512
    }

    // 候选翻页：引擎 candidates(limit) 无 offset，翻页纯客户端。
    /** 编码提示（五笔候选栏显示剩余键；2026-10-14 接线）—— 由 [refresh] 计算。 */
    var wubiHint by mutableStateOf("")
        private set

    private var _candidatePage by mutableStateOf(0)
    private var lastQuery: String? = null // buffer 变化才重置页码（shift 等操作不重置）

    init {
        refresh()
    }

    /** 引擎状态回读；buffer 变化重置候选页码，候选数缩水时钳制页码。 */
    fun refresh() {
        buffer = api.buffer()
        bufferCarried = api.bufferIsCarried()
        // 钉读音的缓冲看守：缓冲一走到别处即解除。引擎侧同样按缓冲配对校验（双保险），
        // 但**不带这层的话「变了又变回」会让旧钉复活**（退格再打同一串）—— 见
        // engine-core 的 `pinned_reading` 字段注释，那条边界就靠这里收。
        if (pinBuffer != null && buffer != pinBuffer) {
            api.pinReading("")
            pinnedReading = ""
            pinBuffer = null
        }
        mode = EngineMode.fromInt(api.mode())
        learnerEnabled = api.learnerEnabled()
        fuzzyEnabled = api.fuzzyEnabled()
        jianpinEnabled = api.jianpinEnabled()
        fullwidth = api.fullwidthState() // 切模式会重置它 ⇒ 每次刷新都重读
        inputScheme = InputScheme.fromInt(api.scheme())
        // 编码提示（2026-10-14 接线）：**只在五笔态、缓冲非空时**问引擎，且只在这里问
        // 一次（引擎侧每次调用是全量重算，别在每帧渲染里调 —— 见接口文档）。
        // 非五笔态恒空：省掉那 O(26×前缀查询)，也避免别的方案栏上挂一条永远为空的提示。
        wubiHint = if (inputScheme == InputScheme.WUBI && buffer.isNotEmpty()) {
            api.wubiNextKeys()
        } else {
            ""
        }
        candidates = api.candidates(fetchLimit)?.toList() ?: emptyList() // JNI 可能返回 null
        // 读数（引擎出口已按读音去重保序）：与 candidates 同一次排名的副产物
        // （引擎侧同一对缓存），**必须紧跟 candidates 之后读**（同一读序契约，
        // 见本函数尾 reading 的注释）。
        candidateReadings = api.candidateReadings()?.toList() ?: emptyList()
        if (buffer != lastQuery) {
            lastQuery = buffer
            _candidatePage = 0
        }
        val maxPage = if (candidates.isEmpty()) 0 else (candidates.size - 1) / pageSize
        if (_candidatePage > maxPage) _candidatePage = maxPage
        // 读数（9 键左列）**必须最后读**：真引擎的读数是 `candidates()` 的副产物
        // （`scheme_reading` 缓存，且与算它时的缓冲绑存 —— 缓冲一变即作废报空）。
        // 先读会拿到空串或上一条缓冲的旧值：2026-10-09 用户报障「确定候选词后左列
        // 不恢复标点」的 Kotlin 侧根因。**别把这个赋值挪回前面**（有单测钉次序）。
        reading = api.displayBuffer()
        // 2026-10-24 用户报障「9 键输入 hou 顶部显示 468」：displayBuffer 偶发为空时
        // 不能把数字缓冲当拼音显示。取候选读音首条兜底（它就是 9 键的拼音读数）。
        if (reading.isEmpty() && inputScheme == InputScheme.NINE) {
            reading = candidateReadings.firstOrNull().orEmpty()
        }
        pendingText = api.pendingText()
        // 组合串同步点（必须最后）：服务层在此把「字段里的组合串」推给输入框。
        onBufferSynced()
    }

    /**
     * 输入单键，返回**引擎要上屏的文本**（空串 = 无提交）；调用方负责提交。
     * 此前返回值被丢弃：字母/数字键确实无提交，但标点键有（引擎层标点表）。
     * 数字面板的 `,` `.` 与全角开关都靠这条回到引擎，见 [KeyRouter.handleKey]。
     */
    fun input(ch: String): String {
        val text = api.inputKey(ch)
        refresh()
        return text
    }

    fun backspace() {
        api.backspace()
        refresh()
    }

    fun clear() {
        api.clear()
        refresh()
    }

    /**
     * 回车（缓冲非空时）该做什么 —— 0=选首候选、1=清空、2=上屏原文（2026-10-14 接线）。
     * 判据在引擎（五笔读「回车行为」设置位；非五笔 / 空缓冲恒 0）—— 本端**只消费**，
     * 不自己判（同一语义抄多份的教训见 engine-core `engine_enter.rs` 头注）。
     */
    fun enterAction(): Int = api.enterAction()

    fun select(index: Int): String {
        val text = api.select(index)
        refresh()
        // 选中即学到频次 → 通知落盘；空串 = 越界/引擎异常，学习状态没变，不必排一次写盘
        if (text.isNotEmpty()) onLearned()
        return text
    }

    /** 英文短语是否实际可命中（开关开 + 表非空，2026-10-24）：普通英文据此改走缓冲。 */
    fun englishPhrasesActive(): Boolean = api.englishPhrasesActive()

    fun switchMode(m: EngineMode) {
        api.switchMode(m.value)
        syncShiftAfterModeSwitch(m)
        refresh()
    }

    /**
     * 换输入方案（「输入模式」菜单）。引擎侧**清缓冲**（换键盘布局后旧缓冲没有意义，
     * 见 engine-core 的 `Engine::set_scheme`），所以这里跟着 refresh 重读
     * buffer/候选/页码。
     */
    fun switchScheme(s: InputScheme) {
        api.setScheme(s.value)
        refresh()
    }

    /**
     * 语言切换（中↔英/中↔繁）：换模式与方案但**保留缓冲** —— 用户裁决 2026-10-09
     * 「中文输入的时候切换英文，拼音保留可继续使用」。⚠️ 与 [switchMode]+[switchScheme]
     * 只差不清缓冲；用户可见的「输入模式」面板切换仍走那两个（且切换前已 commit）。
     */
    fun switchLanguage(mode: EngineMode, scheme: InputScheme) {
        api.switchLanguage(mode.value, scheme.value)
        syncShiftAfterModeSwitch(mode)
        refresh()
    }

    /**
     * 切换后的 ⇧ 收尾（两条切换路共用，四端同一条语义）：
     * - **离开** English：清前端三态，否则 LOCK/SINGLE 残留，切回 English 时字母全大写；
     * - **进入** English：JNI 切换刚把引擎 shift 清掉，而前端 SINGLE/LOCK 还在 ——
     *   必须重推给引擎，否则高亮着却打出小写。单次态仍由 [consumeSingleShift] 消费。
     */
    private fun syncShiftAfterModeSwitch(m: EngineMode) {
        if (m != EngineMode.ENGLISH) {
            resetShift()
        } else if (shiftState != ShiftState.OFF) {
            api.setShift(true)
        }
    }

    /**
     * 钉/解钉一个读音（空串 = 解除）。2026-10-09 用户裁决：9 键左列点一行拼音，
     * **不确认**，候选列表收窄到该读音。缓冲一变自动解除（[refresh] 的看守）。
     * 过滤是**读音恰等**（2026-10-16 裁决「选择的是字」）：读音更长的词与延伸读音的
     * 字都不进（引擎侧四处过滤同语义，注在 `dictionary.rs::scheme_entries_scan`）。
     */
    fun pinReading(reading: String) {
        api.pinReading(reading)
        pinnedReading = reading
        pinBuffer = if (reading.isEmpty()) null else buffer
        refresh()
    }

    /** 点左列一行：同一读音再点 = 解除，否则钉上（都不上屏）。 */
    fun togglePinnedReading(reading: String) {
        if (reading.isEmpty()) return
        pinReading(if (pinnedReading == reading) "" else reading)
    }

    // ---- 「属性设置」三个开关的写侧（设置页与偏好恢复共用；写引擎 + 重读）----
    //
    // 此前设置页直接调 OpiEngine.setXxx + 各自 refresh，两处（页/服务恢复）各写一份
    // 必漂；统一到这里，单一状态源的约定才完整（读侧一直在本类）。

    fun setLearner(enabled: Boolean) {
        api.setLearner(enabled)
        refresh()
    }

    fun setFuzzy(on: Boolean) {
        api.setFuzzy(on)
        refresh()
    }

    /** 逐对模糊音（位序见 [OpiEngineApi.fuzzyPairs]；越界位由 FFI 侧钳掉）。 */
    fun setFuzzyPairs(mask: Int) {
        api.setFuzzyPairs(mask)
        refresh()
    }

    fun setJianpin(on: Boolean) {
        api.setJianpin(on)
        refresh()
    }

    /** 拼音纠错（2026-10-13）。 */
    fun setCorrection(on: Boolean) {
        api.setCorrection(on)
        refresh()
    }

    /** 双拼布局（2026-10-13；0=自然码…5=拼音加加，见 [OpiEngine.shuangpinLayout]）。
     * 与 [switchScheme] 的区别：**不清缓冲**，刷新只为按新布局重解已有键序。 */
    fun setShuangpinLayout(index: Int) {
        api.setShuangpinLayout(index)
        refresh()
    }

    /** 双拼布局读侧（真值回读；设置页的显示源仍是偏好，这条供回环与测试）。 */
    fun shuangpinLayout(): Int = api.shuangpinLayout()

    fun inputSpace(): String {
        val text = api.inputSpace()
        refresh()
        // 拼音模式空格 = 选中首候选，同样改学习状态（英文模式只提交 buffer，空串则不动）
        if (text.isNotEmpty()) onLearned()
        return text
    }

    /**
     * 长按候选删用户词。下标语义与 [selectFromPage] 一致：**页内下标**（翻页后由
     * 页号换算绝对下标），不是全列表下标。越界/非用户词无副作用（引擎只删用户词表里
     * 的条目），删除后刷新候选栏。
     */
    fun removeUserWord(indexInPage: Int) {
        val text = pageCandidates.getOrNull(indexInPage) ?: return
        try {
            api.removeUserWord(text)
        } catch (e: Throwable) {
            // 新增的 JNI 出口若未被 so 注册（RegisterNatives 缺失）会在调用点抛
            // UnsatisfiedLinkError —— 误触长按不该崩 IME，也不谎报删除（不刷新）
            return
        }
        refresh()
        onLearned() // 删词也改学习状态：不落盘的话下次启动它又被 import 回来
    }

    // ---- 候选翻页 ----

    val candidatePage: Int get() = _candidatePage

    val candidatePageCount: Int get() = (candidates.size + pageSize - 1) / pageSize

    val pageCandidates: List<String> get() {
        val start = _candidatePage * pageSize
        if (start >= candidates.size) return emptyList()
        val end = minOf(start + pageSize, candidates.size)
        return candidates.subList(start, end)
    }

    fun nextPage() {
        if (_candidatePage < candidatePageCount - 1) _candidatePage++
    }

    fun prevPage() {
        if (_candidatePage > 0) _candidatePage--
    }

    /** 屏内下标 i → 绝对下标 page*8+i。 */
    fun selectFromPage(indexInPage: Int): String = select(_candidatePage * pageSize + indexInPage)

    // ---- ⇧ 状态机 ----

    /**
     * ⇧ **单击 = 锁定大写；再点一次取消**（2026-10-15 用户裁决「切换到英文，让英文字母
     * 大写的按钮，点击了，就要锁定大写，再次点击取消大写」——对 2026-10-10「点 = 单次、
     * 长按 = 锁定」那条的改判）。
     *
     * 三态里「单次大写」（SINGLE）没被砍掉，挪到 [shiftLongPress]（换手势，不丢能力）——
     * `consumeSingleShift` 与字母键面的大写显示都按状态走，两处不用改。
     */
    fun shiftTap() {
        if (shiftState == ShiftState.LOCK) {
            shiftState = ShiftState.OFF
            api.setShift(false)
        } else {
            shiftState = ShiftState.LOCK
            api.setShift(true)
        }
        // shift 不影响 buffer/candidates，仅通知 UI（也不重置候选页码）
    }

    /** ⇧ 长按 = **单次大写**（SINGLE）：下一个字母大写后由 [consumeSingleShift] 复位。 */
    fun shiftLongPress() {
        shiftState = ShiftState.SINGLE
        api.setShift(true)
    }

    /** single 态消费后复位（lock 不受影响）。 */
    fun consumeSingleShift() {
        if (shiftState == ShiftState.SINGLE) {
            shiftState = ShiftState.OFF
            api.setShift(false)
        }
    }

    // ---- 全角 / 半角 ----

    /** 切换全角（**Android 已无 UI 调用方**：2026-10-13 用户裁决「移动端功能区不需要
     * 全角、半角切换功能」移除月相按钮 —— 能力保留，桌面状态栏仍用同一引擎出口，
     * [fullwidthize] 的开关就是 [fullwidth]）。引擎返回**切完的新状态**，照抄 + 全量刷新。 */
    fun toggleFullwidth() {
        fullwidth = api.toggleFullwidth()
        refresh()
    }

    /**
     * 全角化待上屏文本（**宽档**：ASCII 可打印平移 +0xFEE0；空格与非 ASCII 恒等）。
     * 开关关 ⇒ 原样返回。
     *
     * 为什么要在本类里再写一遍映射：**英文/数字模式下的字母数字各端都是直传**，
     * 不进引擎（[xyz.erik.opi.ime.KeyRouter] 的英文直传与数字面板直提），引擎收不到
     * 那些字符 —— 规则搬不过来，只能镜像；语义源 = engine-core 的
     * `punctuation::ascii_fullwidth_wide` 与 `Engine::fullwidthize`（那边的头注写着
     * 「宿主直传点的唯一入口」）。⚠️ 改一边必须改另一边：`FullwidthTextTest` 现读
     * `punctuation.rs` 的常数与范围，两边的用例集相同。
     */
    fun fullwidthize(text: String): String =
        if (!fullwidth) text else text.map(::widenAscii).joinToString("")

    /**
     * 无论 single/lock 一律复位（UI 与引擎同步）。模式切换/输入目标变更时调用：
     * shiftVisible 只能隐藏 ⇧ 按钮，清不掉状态——中→繁→英 长按锁定后离开再切回 English，
     * ⇧ 仍高亮锁定、字母全大写。
     */
    fun resetShift() {
        if (shiftState == ShiftState.OFF) return
        shiftState = ShiftState.OFF
        api.setShift(false)
    }
}

/**
 * 全角宽档的单字符映射（**镜像** engine-core 的 `punctuation::ascii_fullwidth_wide`）：
 * ASCII 可打印（0x21..0x7E：字母/数字/标点，**除空格**）平移 +0xFEE0；其余原样。
 * ⚠️ 空格：全角空格是 U+3000，不在那条连续段上（0x20+0xFEE0 = U+FF00，未分配），
 * 且引擎侧**有意不宽化空格**（见 `Engine::fullwidthize` 头注）—— 这里保持一致。
 * 锁同步：`FullwidthTextTest` 现读 `punctuation.rs` 的常数与范围。
 */
internal fun widenAscii(ch: Char): Char =
    if (ch.code in 0x21..0x7E) (ch.code + 0xFEE0).toChar() else ch
