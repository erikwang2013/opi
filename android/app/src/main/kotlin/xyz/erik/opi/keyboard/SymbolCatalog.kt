// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.runtime.mutableStateListOf
import xyz.erik.opi.jni.OpiEngine

/** 符号查询接口（OpiEngine 的面板专用 JNI；JVM 测试注入假实现）。 */
interface SymbolApi {
    fun searchSymbols(keyword: String): Array<String>?

    /** 表情**浏览**全量（官方分组序，含多码位序列）；与 symbols.tsv 的 emoji 列
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
     * 表情页 = 引擎的**浏览全量**（`emojiSymbols()`，官方分组序：笑脸在前、旗帜在后），
     * **不再从 `all` 里过滤**。
     *
     * 2026-10-09 换源（用户裁决「表情中常用主流 emoji 不全」）：旧写法取
     * 「符号表 ∩ emoji 集」—— 符号表有意不收麻将/带圈等低码位块（见 gen_symbols.py），
     * 于是国旗、🀄、❤️（VS16 形）、ZWJ 家庭这些一律进不了表情页。浏览要「全」，
     * 集合就不该由符号表决定；引擎的表本身来自 UTS#51 `emoji-test.txt`。
     * **不能用 `by lazy`** —— 它只算一次，首次若全空就永久空。
     */
    val emoji: List<String> get() = emojiData.first

    /**
     * emoji 判定：**查引擎给的表**。
     *
     * 这里曾是一个「含代理对（非 BMP）」的启发式 —— 它当时与引擎的判据逐条同构，
     * 于是看起来对；2026-09-28 引擎换成 UTS#51 `Emoji` 属性后，它静默漏掉 164 条
     * BMP 真 emoji（☺ U+263A、♥ U+2665…）并放行 265 条非 emoji（补充平面里的图形符号，
     * 如 🞀 U+1F780）。**任何在宿主侧重算引擎判据的写法都会重演这一幕**，
     * 所以判据只留一份：引擎的 `emoji` 标志，经 [SymbolApi.emojiSymbols] 透出来。
     */
    fun isEmoji(text: String): Boolean = emojiData.second.contains(text)

    private var _emojiData: Pair<List<String>, Set<String>>? = null

    /**
     * 引擎口径的表情集：**列表与集合同一次取回**（列表给表情页、集合给 isEmoji 的
     * 每帧查），一次 FFI 两次用 —— 「每条符号各打一次 FFI」会让面板滚动卡顿，
     * 而分开缓存会让首帧打两次（两者都有测试钉着）。
     * **空结果不写进缓存**，理由同 [common]：空 = 没拿到（引擎未就绪 / FFI 抖一下），
     * 不是「真的没有」，否则「表情」页会一直空到重启进程。
     */
    private val emojiData: Pair<List<String>, Set<String>>
        get() {
            _emojiData?.takeIf { it.first.isNotEmpty() }?.let { return it }
            val list = (api.emojiSymbols()?.toList() ?: emptyList()).filter(canRender)
            val pair = list to list.toHashSet()
            if (list.isNotEmpty()) _emojiData = pair
            return pair
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
        // —— 点分类条是 `grid.scrollToItem(组.start)`，不重算就会滚错组。
        // 前缀存活数：raw 的 [0, start) 里有多少条通过了同一套 [canRender]。
        val rawList = api.emojiSymbols()?.toList() ?: emptyList()
        // 表情列表拿不到（引擎未就绪）⇒ **不重算**：没有列表就没有「过滤后下标」可言，
        // 硬算会把所有组的 start 压成 0（那比不显示分类更坏）。
        val list = if (rawList.isEmpty()) {
            raw
        } else {
            val aliveUpTo = IntArray(rawList.size + 1)
            var alive = 0
            for (i in rawList.indices) {
                aliveUpTo[i] = alive
                if (canRender(rawList[i])) alive++
            }
            aliveUpTo[rawList.size] = alive
            // start 与 count **一起**重算：count 是分类条读屏文案（「笑脸（N 个）」），
            // 不跟着减就会把摘掉的条数也报进去。
            raw.map { g ->
                val from = g.start.coerceIn(0, rawList.size)
                val to = (g.start + g.count).coerceIn(0, rawList.size)
                g.copy(start = aliveUpTo[from], count = aliveUpTo[to] - aliveUpTo[from])
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

        /**
         * 网格下标 → 分类条要亮第几项（纯函数，单测钉住）。
         * 分组**连续覆盖全表**由引擎侧门禁保证（`emoji_groups_cover_the_table_contiguously`），
         * 所以这里取「最后一个 start ≤ index 的组」即可；空表/负数 → -1（不亮）。
         */
        fun groupIndexOf(groups: List<EmojiGroup>, index: Int): Int {
            for (i in groups.indices.reversed()) {
                if (index >= groups[i].start) return i
            }
            return -1
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
