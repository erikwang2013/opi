// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.candidate

/**
 * 平铺候选的**分页打包器**（2026-10-16 用户裁决「左右翻页改成替换键盘平铺出来……上下滑动
 * 翻页」）：布局对齐参考图 —— 短词一行 4 个、长词一行 2 个，贪心**不重排**（次序 = 候选
 * 名次，重排会把好词挪走）。纯函数，Compose 只负责画（见 `CandidateTiledView`）。
 */
internal data class TileCell(val index: Int, val units: Int)

/** 每条候选占的横向单位：≤2 字 = 1（一行 4 个），更长 = 2（一行 2 个）。 */
internal fun tileUnits(text: String): Int = if (text.length <= 2) 1 else 2

/** 一行铺满多少单位。 */
internal const val UNITS_PER_ROW = 4

/** 一页几行（平铺区的行数节奏）。 */
internal const val ROWS_PER_PAGE = 4

/**
 * 候选文本 → 「页 → 行 → [TileCell]」。`TileCell.index` 是**全局下标**
 * （`controller.candidates` 的下标，选中走 [xyz.erik.opi.ime.KeyRouter.handleCandidateAbsolute]）。
 */
internal fun tilePages(texts: List<String>): List<List<List<TileCell>>> {
    if (texts.isEmpty()) return emptyList()
    val rows = mutableListOf<MutableList<TileCell>>()
    var row = mutableListOf<TileCell>()
    var used = 0
    texts.forEachIndexed { i, t ->
        val u = tileUnits(t)
        if (used + u > UNITS_PER_ROW) {
            rows += row
            row = mutableListOf()
            used = 0
        }
        row += TileCell(i, u)
        used += u
    }
    if (row.isNotEmpty()) rows += row
    return rows.chunked(ROWS_PER_PAGE)
}
