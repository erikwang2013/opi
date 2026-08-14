package io.opi.input.keyboard

import io.opi.input.jni.OpiEngine

/** 符号查询接口（OpiEngine 的面板专用 JNI；JVM 测试注入假实现）。 */
interface SymbolApi {
    fun searchSymbols(keyword: String): Array<String>?
    fun symbolBlocks(): String
    fun symbolsInBlock(id: Short): Array<String>?
}

/**
 * 符号数据层（对齐 flutter symbol_catalog.dart）：缓存 FFI 查询（每查询一次），
 * 内存级最近使用（M5 不落盘）。JNI 只回文本数组，emoji 标记由码点推断。
 */
class SymbolCatalog(private val api: SymbolApi = OpiEngine) {

    /** 符号块（symbolBlocks() JSON 解析，serde 输出固定 schema）。 */
    data class Block(val id: Short, val start: Int, val end: Int, val name: String, val common: Boolean)

    private var _common: List<String>? = null
    private var _all: List<String>? = null
    private val _recent = mutableListOf<String>()

    /** 常用 = 全块 symbolsInBlock 并集（按块序），按 text 去重。 */
    val common: List<String> get() {
        if (_common == null) {
            val seen = LinkedHashSet<String>()
            for (b in parseBlocks(api.symbolBlocks())) {
                for (text in api.symbolsInBlock(b.id) ?: emptyArray()) seen.add(text)
            }
            _common = seen.toList()
        }
        return _common!!
    }

    /** 全部 = searchSymbols('')：空关键字时引擎返回全部条目。 */
    val all: List<String> get() {
        if (_all == null) _all = api.searchSymbols("")?.toList() ?: emptyList()
        return _all!!
    }

    /** 表情 = 全量按 emoji 过滤。 */
    val emoji: List<String> get() = all.filter(::isEmoji)

    fun search(q: String): List<String> {
        if (q.trim().isEmpty()) return all
        return api.searchSymbols(q)?.toList() ?: emptyList()
    }

    val recents: List<String> get() = _recent.toList()

    fun recordRecent(text: String) {
        _recent.remove(text)
        _recent.add(0, text)
        while (_recent.size > maxRecents) _recent.removeAt(_recent.size - 1)
    }

    companion object {
        const val maxRecents = 50

        /** emoji 判定：含代理对（非 BMP 字符）。JNI 只回文本，取不到引擎 emoji 标记。 */
        fun isEmoji(text: String): Boolean = text.any { it.isSurrogate() }

        /** 解析 symbolBlocks() JSON（serde 固定输出 `[{"id","start","end","name","common"}]`）。 */
        fun parseBlocks(json: String): List<Block> {
            val out = mutableListOf<Block>()
            for (m in blockRe.findAll(json)) {
                val s = m.value
                out += Block(
                    id = idRe.find(s)?.groupValues?.getOrNull(1)?.toShortOrNull() ?: 0,
                    start = startRe.find(s)?.groupValues?.getOrNull(1)?.toIntOrNull() ?: 0,
                    end = endRe.find(s)?.groupValues?.getOrNull(1)?.toIntOrNull() ?: 0,
                    name = (nameRe.find(s)?.groupValues?.getOrNull(1) ?: "").replace("\\\"", "\""),
                    common = commonRe.find(s)?.groupValues?.getOrNull(1) == "true",
                )
            }
            return out
        }
    }
}

// 固定 schema 专用解析（仅消费引擎 serde 输出，不引 org.json：JVM 测试无 android.jar 运行时）
private val blockRe = Regex("""\{[^{}]*}""")
private val idRe = Regex(""""id":(\d+)""")
private val startRe = Regex(""""start":(\d+)""")
private val endRe = Regex(""""end":(\d+)""")
private val nameRe = Regex(""""name":"((?:[^"\\]|\\.)*)"""")
private val commonRe = Regex(""""common":(true|false)""")
