// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package io.opi.input.engine

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import io.opi.input.jni.OpiEngine

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
 * 引擎接口：EngineController 依赖抽象，JVM 测试用假引擎替换 JNI 实现
 * （OpiEngine 的 load/searchSymbols/symbolBlocks/symbolsInBlock 为面板/资产专用，不入接口）。
 */
interface OpiEngineApi {
    fun inputKey(ch: String): String
    fun backspace()
    fun clear()
    fun select(index: Int): String
    fun switchMode(mode: Int)
    fun setShift(on: Boolean)
    fun inputSpace(): String
    fun candidates(limit: Int): Array<String>?
    fun buffer(): String
    fun mode(): Int

    /** 删除一个用户词（连同频次）；非用户词/不存在无副作用。 */
    fun removeUserWord(text: String)
}

/**
 * 单一状态源：封装 OpiEngine，UI 经 mutableStateOf 订阅
 * （对齐 flutter engine_controller.dart：翻页 fetchLimit=64、8/页、buffer 变化重置页码）。
 */
class EngineController(
    private val api: OpiEngineApi = OpiEngine,
    /** 学习状态变更（选词/删用户词）后的落盘通知；默认空实现（JVM 测试/无持久化场景不感知）。 */
    private val onLearned: () -> Unit = {},
) {
    var buffer by mutableStateOf("")
        private set
    var mode by mutableStateOf(EngineMode.PINYIN)
        private set
    var candidates by mutableStateOf<List<String>>(emptyList())
        private set

    var shiftState by mutableStateOf(ShiftState.OFF)
        private set

    companion object {
        const val pageSize = 8
        const val fetchLimit = 64
    }

    // 候选翻页：引擎 candidates(limit) 无 offset，翻页纯客户端。
    private var _candidatePage by mutableStateOf(0)
    private var lastQuery: String? = null // buffer 变化才重置页码（shift 等操作不重置）

    init {
        refresh()
    }

    /** 引擎状态回读；buffer 变化重置候选页码，候选数缩水时钳制页码。 */
    fun refresh() {
        buffer = api.buffer()
        mode = EngineMode.fromInt(api.mode())
        candidates = api.candidates(fetchLimit)?.toList() ?: emptyList() // JNI 可能返回 null
        if (buffer != lastQuery) {
            lastQuery = buffer
            _candidatePage = 0
        }
        val maxPage = if (candidates.isEmpty()) 0 else (candidates.size - 1) / pageSize
        if (_candidatePage > maxPage) _candidatePage = maxPage
    }

    fun input(ch: String) {
        api.inputKey(ch)
        refresh()
    }

    fun backspace() {
        api.backspace()
        refresh()
    }

    fun clear() {
        api.clear()
        refresh()
    }

    fun select(index: Int): String {
        val text = api.select(index)
        refresh()
        // 选中即学到频次 → 通知落盘；空串 = 越界/引擎异常，学习状态没变，不必排一次写盘
        if (text.isNotEmpty()) onLearned()
        return text
    }

    fun switchMode(m: EngineMode) {
        api.switchMode(m.value)
        // ⇧ 只在 English 有意义：离开必须清，否则 LOCK/SINGLE 残留，切回 English 时字母全大写
        if (m != EngineMode.ENGLISH) resetShift()
        refresh()
    }

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

    fun shiftTap() {
        if (shiftState == ShiftState.OFF) {
            shiftState = ShiftState.SINGLE
            api.setShift(true)
        } else {
            shiftState = ShiftState.OFF
            api.setShift(false)
        }
        // shift 不影响 buffer/candidates，仅通知 UI（也不重置候选页码）
    }

    fun shiftLongPress() {
        shiftState = ShiftState.LOCK
        api.setShift(true)
    }

    /** single 态消费后复位（lock 不受影响）。 */
    fun consumeSingleShift() {
        if (shiftState == ShiftState.SINGLE) {
            shiftState = ShiftState.OFF
            api.setShift(false)
        }
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
