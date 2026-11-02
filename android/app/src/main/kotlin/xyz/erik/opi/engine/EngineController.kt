// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import xyz.erik.opi.jni.OpiEngine

// 引擎模式 / ⇧ / 输入方案在 EngineTypes.kt；接口在 OpiEngineApi.kt（同包，500 行门禁拆出）。

/**
 * 单一状态源：封装 OpiEngine，UI 经 mutableStateOf 订阅（按引擎页大小翻页、buffer
 * 变化重置页码；fetchLimit=512，让「壹」这类排在第 79/155 位的单字可翻到）。
 */
class EngineController(
    internal val api: OpiEngineApi = OpiEngine,
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
        internal set

    var mode by mutableStateOf(EngineMode.PINYIN)
        private set
    var candidates by mutableStateOf<List<String>>(emptyList())
        internal set

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

    // 普通栏只取一页；512 条只为平铺全量，或旧 so 的读音列（它是 candidates 的副产物）。
    private var tiledFetch = false
    private val fetchLimit: Int get() = if (tiledFetch || needsCandidateReadings(mode, inputScheme)) FETCH_LIMIT_FULL else pageSize

    /** 快照回传的候选窗口：折叠一页、平铺 512；读数深度由 Rust 按方案自己加深。 */
    private val snapshotLimit: Int get() = if (tiledFetch) FETCH_LIMIT_FULL else pageSize

    /** 旧 so / 假引擎无快照出口：失败一次后不再重试，后续都走逐出口路径。 */
    private var snapshotBroken = false
    val pageSize: Int = api.pageSize() // 每页候选数：引擎 `router::PAGE_SIZE`，别写死 8

    /** 编码提示（五笔候选栏显示剩余键；2026-10-14 接线）—— 由 [refresh] 计算。 */
    var wubiHint by mutableStateOf("")
        private set

    private var _candidatePage by mutableStateOf(0)
    private var lastQuery: String? = null // buffer 变化才重置页码（shift 等操作不重置）

    init {
        refresh()
    }

    /**
     * 引擎状态回读；buffer 变化重置页码。**快照优先**（性能项 1）：一次 JNI 取回全部
     * 读侧状态；失败/旧 so 安全回退 [refreshLegacy]。两条路都保持
     * candidates → candidateReadings → displayBuffer 的读序。
     */
    fun refresh() {
        val snapshot = stateSnapshotOrNull()
        if (snapshot != null) {
            buffer = snapshot.buffer
            bufferCarried = snapshot.bufferCarried
            clearStalePin()
            mode = snapshot.mode
            learnerEnabled = snapshot.learnerEnabled
            fuzzyEnabled = snapshot.fuzzyEnabled
            jianpinEnabled = snapshot.jianpinEnabled
            fullwidth = snapshot.fullwidth
            inputScheme = snapshot.scheme
            wubiHint = snapshot.wubiHint
            candidates = snapshot.candidates
            candidateReadings = snapshot.candidateReadings
            reading = snapshot.displayBuffer
            pendingText = snapshot.pendingText
        } else {
            refreshLegacy()
        }
        finishRefresh()
    }

    /** 逐出口回退路径（旧 so 的唯一路径）。读序契约与快照相同，别调换顺序。 */
    private fun refreshLegacy() {
        buffer = api.buffer()
        bufferCarried = api.bufferIsCarried()
        clearStalePin()
        mode = EngineMode.fromInt(api.mode())
        learnerEnabled = api.learnerEnabled()
        fuzzyEnabled = api.fuzzyEnabled()
        jianpinEnabled = api.jianpinEnabled()
        fullwidth = api.fullwidthState() // 切模式会重置它 ⇒ 每次刷新都重读
        inputScheme = InputScheme.fromInt(api.scheme())
        // 编码提示只在五笔态、缓冲非空时问（引擎每次全量重算，别在每帧渲染里调）。
        wubiHint = if (inputScheme == InputScheme.WUBI && buffer.isNotEmpty()) {
            api.wubiNextKeys()
        } else {
            ""
        }
        candidates = api.candidates(fetchLimit)?.toList() ?: emptyList() // JNI 可能返回 null
        // 读数与 candidates 同一次排名的副产物（引擎同一对缓存），**必须紧跟其后读**。
        candidateReadings = if (needsCandidateReadings(mode, inputScheme)) {
            api.candidateReadings()?.toList() ?: emptyList()
        } else {
            emptyList()
        }
        // displayBuffer 也读这份缓存，**必须最后读**（先读会拿到空串/旧值）。
        reading = api.displayBuffer()
        pendingText = api.pendingText()
    }

    /** 快照出口：抛异常 / 非空畸形载荷才永久回退；空数组 = 未装载，下次再试。 */
    private fun stateSnapshotOrNull(): EngineSnapshot? {
        if (snapshotBroken) return null
        val raw = try {
            api.stateSnapshot(snapshotLimit)
        } catch (e: Throwable) {
            snapshotBroken = true // 旧 so / ABI 不符：不崩输入法，也不每次击键都抛
            return null
        }
        if (raw.isNullOrEmpty()) return null
        val parsed = parseSnapshot(raw)
        if (parsed == null) snapshotBroken = true
        return parsed
    }

    /** 钉读音的缓冲看守（不带这层「变了又变回」会让旧钉复活）。 */
    private fun clearStalePin() {
        if (pinBuffer != null && buffer != pinBuffer) {
            api.pinReading("")
            pinnedReading = ""
            pinBuffer = null
        }
    }

    /** 两条刷新路的共同收尾：页码、9 键读数兜底、联想门禁、组合串同步点。 */
    private fun finishRefresh() {
        if (buffer != lastQuery) {
            lastQuery = buffer
            _candidatePage = 0
        }
        val maxPage = if (candidates.isEmpty()) 0 else (candidates.size - 1) / pageSize
        if (_candidatePage > maxPage) _candidatePage = maxPage
        // 9 键 displayBuffer 偶发为空时不能把数字缓冲当拼音显示（2026-10-24 报障）。
        if (reading.isEmpty() && inputScheme == InputScheme.NINE) {
            reading = candidateReadings.firstOrNull().orEmpty()
        }
        gateAssociation() // 智能联想默认关闭：空缓冲候选（联想项）不推给 UI
        onBufferSynced() // 组合串同步点（必须最后）
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

    /** 平铺候选展开/收拢：展开要全量（滑动翻页），收拢回一页。 */
    internal fun setTiledFetch(on: Boolean) { if (tiledFetch != on) { tiledFetch = on; refresh() } }
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

    /** 屏内下标 → 全局下标：翻页在本地切片（引擎 page 恒 0），别改走 selectPage。 */
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
        // 2026-10-27：引擎 set_shift 清联想 ⇒ 必须重读候选（原先只动 UI 三态）。
        refresh()
    }

    /** ⇧ 长按 = **单次大写**（SINGLE）：下一个字母大写后由 [consumeSingleShift] 复位。 */
    fun shiftLongPress() {
        shiftState = ShiftState.SINGLE
        api.setShift(true)
        refresh()
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
