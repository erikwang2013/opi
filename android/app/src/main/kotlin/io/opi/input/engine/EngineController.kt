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
}

/**
 * 单一状态源：封装 OpiEngine，UI 经 mutableStateOf 订阅
 * （对齐 flutter engine_controller.dart：翻页 fetchLimit=64、8/页、buffer 变化重置页码）。
 */
class EngineController(private val api: OpiEngineApi = OpiEngine) {
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
        return text
    }

    fun switchMode(m: EngineMode) {
        api.switchMode(m.value)
        refresh()
    }

    fun inputSpace(): String {
        val text = api.inputSpace()
        refresh()
        return text
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
}
