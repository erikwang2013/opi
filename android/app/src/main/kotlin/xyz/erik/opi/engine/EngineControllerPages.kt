// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

// 候选翻页（2026-11-08 从 EngineController 拆出守 500 行门禁）。
// 扩展函数与成员函数同包调用，UI/测试不用改 import。

val EngineController.candidatePage: Int get() = candidatePageIndex

val EngineController.candidatePageCount: Int get() = (candidates.size + pageSize - 1) / pageSize

val EngineController.pageCandidates: List<String> get() {
    val start = candidatePageIndex * pageSize
    if (start >= candidates.size) return emptyList()
    val end = minOf(start + pageSize, candidates.size)
    return candidates.subList(start, end)
}

fun EngineController.nextPage() {
    // 到已抓取的末页时先加深一页（增量翻页），再前进；候选真没了就停在原地。
    if (candidatePageIndex + 1 >= candidatePageCount) {
        ensureCandidatePages(candidatePageCount + 1)
    }
    if (candidatePageIndex < candidatePageCount - 1) candidatePageIndex++
}

fun EngineController.prevPage() {
    if (candidatePageIndex > 0) candidatePageIndex--
}

/** 屏内下标 → 全局下标：翻页在本地切片（引擎 page 恒 0），别改走 selectPage。 */
fun EngineController.selectFromPage(indexInPage: Int): String =
    select(candidatePageIndex * pageSize + indexInPage)
