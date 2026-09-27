// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package io.opi.input.keyboard

import androidx.compose.runtime.mutableStateListOf
import io.opi.input.jni.OpiEngine

/** 符号查询接口（OpiEngine 的面板专用 JNI；JVM 测试注入假实现）。 */
interface SymbolApi {
    fun searchSymbols(keyword: String): Array<String>?
    fun emojiSymbols(): Array<String>?
    fun symbolBlocks(): String
    fun symbolsInBlock(id: Short): Array<String>?
}

/**
 * 符号数据层（对齐 flutter symbol_catalog.dart）：缓存 FFI 查询（每查询一次），
 * 内存级最近使用（M5 不落盘）。
 *
 * emoji 与否**由引擎判定**（`emojiSymbols()`），本类不持有任何码位启发式 —— 见 [isEmoji]。
 */
class SymbolCatalog(private val api: SymbolApi = OpiEngine) {

    /** 符号块（symbolBlocks() JSON 解析，serde 输出固定 schema）。 */
    data class Block(val id: Short, val start: Int, val end: Int, val name: String, val common: Boolean)

    private var _common: List<String>? = null
    private var _all: List<String>? = null
    // 必须是快照可观察状态：SymbolPanel 在组合中读 recents，普通 MutableList 写入
    // 不产生通知 → recordRecent 后「最近使用」那一行不出现（首次点 emoji 时可见）。
    private val _recent = mutableStateListOf<String>()

    /**
     * 常用 = 各 **common 块** 的 `symbolsInBlock` 并集（按块序），按 text 去重。
     *
     * ⚠️ **空结果不写进缓存**：`_common` 一旦被写成空列表就再也不会重算，而「第一次访问
     * 时引擎还没就绪 / FFI 抖一下」都会返回空 —— 症状是**三个标签从此永远全空**，
     * 且要重启进程才恢复。空 = 没拿到，不是「真的没有」，所以留着重试。
     */
    val common: List<String> get() {
        _common?.takeIf { it.isNotEmpty() }?.let { return it }
        val seen = LinkedHashSet<String>()
        // 只取 common 块：引擎的 symbol_blocks() 已经按 common 过滤过，这里再过一次标志位，
        // 免得将来有人把过滤挪走时「常用」又变回「全部」。
        for (b in parseBlocks(api.symbolBlocks()).filter { it.common }) {
            for (text in api.symbolsInBlock(b.id) ?: emptyArray()) seen.add(text)
        }
        return seen.toList().also { if (it.isNotEmpty()) _common = it }
    }

    /** 全部 = searchSymbols('')：空关键字时引擎返回全部条目。空结果同样不缓存。 */
    val all: List<String> get() {
        _all?.takeIf { it.isNotEmpty() }?.let { return it }
        val list = api.searchSymbols("")?.toList() ?: emptyList()
        return list.also { if (it.isNotEmpty()) _all = it }
    }

    /** 表情 = 全量里**引擎判定为 emoji** 的那些。**不能用 `by lazy`** —— 它只算一次，首次若全空就永久空。 */
    val emoji: List<String> get() = all.filter(::isEmoji)

    /**
     * emoji 判定：**查引擎给的表**。
     *
     * 这里曾是一个「含代理对（非 BMP）」的启发式 —— 它当时与引擎的判据逐条同构，
     * 于是看起来对；2026-09-28 引擎换成 UTS#51 `Emoji` 属性后，它静默漏掉 164 条
     * BMP 真 emoji（☺ U+263A、♥ U+2665…）并放行 265 条非 emoji（补充平面里的图形符号，
     * 如 🞀 U+1F780）。**任何在宿主侧重算引擎判据的写法都会重演这一幕**，
     * 所以判据只留一份：引擎的 `emoji` 标志，经 [SymbolApi.emojiSymbols] 透出来。
     */
    fun isEmoji(text: String): Boolean = emojiSet.contains(text)

    private var _emoji: Set<String>? = null

    /**
     * 引擎口径的表情集。**空结果不写进缓存**，理由同 [common]：空 = 没拿到（引擎未就绪 /
     * FFI 抖一下），不是「真的没有」，否则「表情」页会一直空到重启进程。
     */
    private val emojiSet: Set<String>
        get() {
            _emoji?.takeIf { it.isNotEmpty() }?.let { return it }
            val s = api.emojiSymbols()?.toSet() ?: emptySet()
            return s.also { if (it.isNotEmpty()) _emoji = it }
        }

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
// 注意：Android 用 ICU 正则（PatternNative），`\{[^{}]*}` 的末尾 `}` 被当量化符解析抛
// PatternSyntaxException（OpenJDK 接受、JVM 测试抓不到）→ 必须转义为 `\}`。
private val blockRe = Regex("""\{[^{}]*\}""")
private val idRe = Regex(""""id":(\d+)""")
private val startRe = Regex(""""start":(\d+)""")
private val endRe = Regex(""""end":(\d+)""")
private val nameRe = Regex(""""name":"((?:[^"\\]|\\.)*)"""")
private val commonRe = Regex(""""common":(true|false)""")
