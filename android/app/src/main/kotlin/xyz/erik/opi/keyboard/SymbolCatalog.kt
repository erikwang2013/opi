// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.runtime.mutableStateListOf
import xyz.erik.opi.jni.OpiEngine

/** 符号查询接口（OpiEngine 的面板专用 JNI；JVM 测试注入假实现）。 */
interface SymbolApi {
    fun searchSymbols(keyword: String): Array<String>?

    /** 表情**浏览**表（OpenMoji 常用子集，面板分组序）；与 symbols.tsv 的 emoji 列
     * 是两条路 —— 那是搜索、这是浏览（详见引擎侧 `api::emoji_symbol_texts`）。 */
    fun emojiSymbols(): Array<String>?

    /** 表情**分类条**：JSON `[{name,start,count}]`（下标对齐 [emojiSymbols]）。 */
    fun emojiGroups(): String?
    fun symbolBlocks(): String
    fun symbolsInBlock(id: Short): Array<String>?
}

/**
 * 符号数据层（对齐 flutter symbol_catalog.dart）：缓存 FFI 查询（每查询一次），
 * 内存级最近使用（M5 不落盘）。
 *
 * emoji 与否**由引擎判定**（`emojiSymbols()`），本类不持有任何码位启发式 —— 见 [isEmoji]。
 */
class SymbolCatalog(
    private val api: SymbolApi = OpiEngine,
    /**
     * 「本机字体画不画得出这个字形」—— 判据见 [canRenderGlyph]（测试注入 `{ true }`）。
     * **只对单码位条目生效**（多码位序列的判定不可靠，理由见那个函数）。
     */
    private val canRender: (String) -> Boolean = { true },
) {

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

    /**
     * 表情页 = 引擎的**浏览表**（`emojiSymbols()`，OpenMoji 常用子集，面板分组序：笑脸在前、符号在后），
     * **不再从 `all` 里过滤**。
     *
     * 2026-10-09 换源（用户裁决「表情中常用主流 emoji 不全」）：旧写法取
     * 「符号表 ∩ emoji 集」—— 符号表有意不收麻将/带圈等低码位块（见 gen_symbols.py），
     * 于是国旗、🀄、❤️（VS16 形）、ZWJ 家庭这些一律进不了表情页。浏览要「全」，
     * 集合就不该由符号表决定；引擎的表本身来自 OpenMoji 常用子集
     * （筛选规则与源 pin 见 `scripts/gen_emoji_kaomoji.py`）。
     * **不能用 `by lazy`** —— 它只算一次，首次若全空就永久空。
     */
    val emoji: List<String> get() = emojiData.alive

    /**
     * emoji 判定：**查引擎给的表**。
     *
     * 这里曾是一个「含代理对（非 BMP）」的启发式 —— 它当时与引擎的判据逐条同构，
     * 于是看起来对；2026-09-28 引擎换成 UTS#51 `Emoji` 属性后，它静默漏掉 164 条
     * BMP 真 emoji（☺ U+263A、♥ U+2665…）并放行 265 条非 emoji（补充平面里的图形符号，
     * 如 🞀 U+1F780）。**任何在宿主侧重算引擎判据的写法都会重演这一幕**，
     * 所以判据只留一份：引擎的 `emoji` 标志，经 [SymbolApi.emojiSymbols] 透出来。
     */
    fun isEmoji(text: String): Boolean = emojiData.aliveSet.contains(text)

    /**
     * 引擎口径的表情数据：**原始表 + 前缀存活数一次取回**，表情页（[emoji]）、
     * isEmoji 的每帧查（[aliveSet]）与分类条下标重算（[aliveUpTo]）共用同一份。
     *
     * 2026-10-30：原先 [emoji] 与 [emojiGroups] 各拉一次 `emojiSymbols()`、各跑一遍
     * 逐条 [canRender] —— 首帧把 689 条 `Paint.hasGlyph` 探了两遍。
     * **空结果不写进缓存**，理由同 [common]：空 = 没拿到（引擎未就绪 / FFI 抖一下），
     * 不是「真的没有」，否则「表情」页会一直空到重启进程。
     */
    private class EmojiData(val raw: List<String>, val alive: List<String>, val aliveUpTo: IntArray) {
        val aliveSet: Set<String> = alive.toHashSet()
    }

    private var _emojiData: EmojiData? = null

    private val emojiData: EmojiData
        get() {
            _emojiData?.takeIf { it.alive.isNotEmpty() }?.let { return it }
            val raw = api.emojiSymbols()?.toList() ?: emptyList()
            val alive = ArrayList<String>(raw.size)
            // aliveUpTo[i] = raw 的 [0, i) 里通过了 canRender 的条数。
            val aliveUpTo = IntArray(raw.size + 1)
            for (i in raw.indices) {
                aliveUpTo[i] = alive.size
                if (canRender(raw[i])) alive += raw[i]
            }
            aliveUpTo[raw.size] = alive.size
            val data = EmojiData(raw, alive, aliveUpTo)
            if (alive.isNotEmpty()) _emojiData = data
            return data
        }

    fun search(q: String): List<String> {
        if (q.trim().isEmpty()) return all
        return api.searchSymbols(q)?.toList() ?: emptyList()
    }

    /** 表情分类条的一项：`start` 是 [emoji] 里的下标（点它 = 网格滚到那里）。 */
    data class EmojiGroup(val name: String, val start: Int, val count: Int)

    private var _emojiGroups: List<EmojiGroup>? = null

    /**
     * 表情分类（官方 9 组；2026-10-12 用户裁决「中间是表情分类」）。
     * 空结果不写进缓存（同 [common]：空 = 没拿到，不是「真的没有」）。
     */
    val emojiGroups: List<EmojiGroup> get() {
        _emojiGroups?.takeIf { it.isNotEmpty() }?.let { return it }
        val raw = parseEmojiGroups(api.emojiGroups() ?: "")
        // ⚠️ `start` 必须是**过滤后**列表里的下标（过滤会摘掉若干条，原下标会整体偏前）
        // —— 点分类条按 start/count 切片，不重算就会切错组。
        // 存活统计与 [emoji] 共用（同一次取表、同一次字形探测，见 [emojiData]）。
        val data = emojiData
        // 表情列表拿不到（引擎未就绪）⇒ **不重算**：没有列表就没有「过滤后下标」可言，
        // 硬算会把所有组的 start 压成 0（那比不显示分类更坏）。
        val list = if (data.raw.isEmpty()) {
            raw
        } else {
            // start 与 count **一起**重算：count 是分类条读屏文案（「笑脸（N 个）」），
            // 不跟着减就会把摘掉的条数也报进去。
            raw.map { g ->
                val from = g.start.coerceIn(0, data.raw.size)
                val to = (g.start + g.count).coerceIn(0, data.raw.size)
                g.copy(
                    start = data.aliveUpTo[from],
                    count = data.aliveUpTo[to] - data.aliveUpTo[from],
                )
            }
        }
        return list.also { if (it.isNotEmpty()) _emojiGroups = it }
    }

    val recents: List<String> get() = _recent.toList()

    fun recordRecent(text: String) {
        _recent.remove(text)
        _recent.add(0, text)
        while (_recent.size > maxRecents) _recent.removeAt(_recent.size - 1)
    }

    companion object {
        const val maxRecents = 50

        /** Android 端的缺省构造：把 [canRenderGlyph] 接到真实字体上（探针 Paint 只需一个）。
         * 放在这里而不是服务里：字形判定的**全部知识**留在本文件（服务那边一行）。 */
        fun androidDefault(): SymbolCatalog {
            val paint = android.graphics.Paint()
            return SymbolCatalog(canRender = { canRenderGlyph(paint, it) })
        }

        /**
         * 「本机字体画不画得出这个字形」（`Paint.hasGlyph`，API 23+）。
         *
         * 来由（2026-10-12 用户报障「有个表情显示是长方形框」）：数据里有 U+1FAEB
         * 「cracking face」（Unicode 17.0），而设备系统字体（Noto Color Emoji）没覆盖
         * ⇒ 网格里画出一个空心矩形。数据本身没错（新机型/新字体会显示），错的只是把它
         * 硬画出来。
         *
         * ⚠️ **只判单码位**：`hasGlyph` 对多码位序列问的是「整串连字有没有字形」，
         * 旗帜（🇨🇳 = 两个区域指示符）/家庭（ZWJ）/键帽这类会**误判成没有**，把本来
         * 能显示的整类藏掉 —— 那比留一个空心框更坏。多码位一律放行。
         */
        fun canRenderGlyph(paint: android.graphics.Paint, text: String): Boolean =
            text.codePointCount(0, text.length) > 1 || paint.hasGlyph(text)

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

        /** 解析 emojiGroups() JSON（serde 固定输出 `[{"name","start","count"}]`）。 */
        fun parseEmojiGroups(json: String): List<EmojiGroup> {
            val out = mutableListOf<EmojiGroup>()
            for (m in blockRe.findAll(json)) {
                val s = m.value
                val name = nameRe.find(s)?.groupValues?.getOrNull(1) ?: ""
                out += EmojiGroup(
                    name = name.replace("\\\"", "\""),
                    start = startRe.find(s)?.groupValues?.getOrNull(1)?.toIntOrNull() ?: 0,
                    count = countRe.find(s)?.groupValues?.getOrNull(1)?.toIntOrNull() ?: 0,
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
private val countRe = Regex(""""count":(\d+)""")
