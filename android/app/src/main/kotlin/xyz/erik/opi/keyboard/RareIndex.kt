// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

/**
 * 生僻字索引：`assets/rare_index.tsv`（`字<TAB>部首号<TAB>总笔画数`，由
 * `scripts/gen_rare_index.py` 从 Unicode Unihan 生成、**入库**）的解析与查询。
 *
 * 解析是**纯函数**（`RareIndexTest` 用内联夹具钉住），读资产由调用方做
 * （面板在第一次打开时读，别在服务启动时读 1.1MB）。
 *
 * 数据形态：`Map<部首号(1..214), Map<总笔画数, List<字>>>`，字序 = 表序（码点升序，
 * 常用字天然靠前）。列表**直接复用**解析时的顺序，不在 UI 层再排。
 */
object RareIndex {

    /** 康熙部首数上限（Unihan kRSUnicode 的取值范围）。 */
    const val MAX_RADICAL = 214

    /**
     * 解析 TSV。坏行（列数不对/非数字/空行）**跳过而不抛** —— 资产是生成物，但
     * 用户设备上的文件可能被替换/截断，面板该降级成空网格而不是崩 IME。
     */
    fun parse(text: String): Map<Int, Map<Int, List<String>>> {
        val out = LinkedHashMap<Int, MutableMap<Int, MutableList<String>>>()
        for (line in text.lineSequence()) {
            if (line.isBlank()) continue
            val parts = line.split('\t')
            if (parts.size != 3 || parts[0].codePointCount(0, parts[0].length) != 1) continue
            val radical = parts[1].toIntOrNull() ?: continue
            val strokes = parts[2].toIntOrNull() ?: continue
            if (radical !in 1..MAX_RADICAL || strokes <= 0) continue
            out.getOrPut(radical) { LinkedHashMap() }
                .getOrPut(strokes) { mutableListOf() }
                .add(parts[0])
        }
        return out
    }

    /**
     * 部首号 → 康熙部首字形。Unicode 的 Kangxi Radicals 区（U+2F00..U+2FD5）与
     * 部首号 1..214 **一一对应**（U+2F00 = 一 = 部首 1，U+2FD5 = 龠 = 部首 214），
     * 故直接算码点。⚠️ 字体缺字时显示豆腐块 —— 面板另有「部首 N」的读屏名兜底，
     * 视觉上换字体是设备侧的事。
     */
    fun radicalGlyph(radical: Int): String =
        String(Character.toChars(0x2F00 + (radical - 1).coerceIn(0, MAX_RADICAL - 1)))

    /** 某部首下可选的笔画数（升序）——面板第二级筛选的来源。 */
    fun strokeCounts(index: Map<Int, Map<Int, List<String>>>, radical: Int): List<Int> =
        index[radical]?.keys?.sorted() ?: emptyList()

    /** 某部首 + 笔画数下的字（无 → 空表）。 */
    fun chars(
        index: Map<Int, Map<Int, List<String>>>,
        radical: Int,
        strokes: Int,
    ): List<String> = index[radical]?.get(strokes) ?: emptyList()
}
