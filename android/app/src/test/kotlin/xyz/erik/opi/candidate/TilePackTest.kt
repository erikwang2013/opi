// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 平铺候选的**分页打包器**（2026-10-16 用户裁决「替换键盘平铺出来……上下滑动翻页」，
 * 布局对齐参考图 2：短词一行 4 个、长词一行 2 个）。
 *
 * 纯函数先行：行列的贪心规则与页边界在这里钉死，Compose 只负责画
 * （渲染以模拟器截屏核验，同本仓移动端改动的既有口径）。
 */
class TilePackTest {
    private fun texts(vararg t: String) = t.toList()

    @Test
    fun fourShortWordsFillOneRow() {
        val pages = tilePages(texts("你说", "迷说", "你顺", "米说"))
        assertEquals(1, pages.size)
        assertEquals(1, pages[0].size)
        assertEquals(
            listOf(TileCell(0, 1), TileCell(1, 1), TileCell(2, 1), TileCell(3, 1)),
            pages[0][0],
        )
    }

    @Test
    fun longWordTakesTwoUnitsSoTwoPerRow() {
        val pages = tilePages(texts("你好好休息吧", "你好好上课吧"))
        assertEquals(listOf(listOf(TileCell(0, 2), TileCell(1, 2))), pages[0])
    }

    @Test
    fun rowPacksGreedilyWithoutReordering() {
        // 长(2) + 短(1) + 短(1) = 4 个单位铺满一行；下一条短词另起一行。
        val pages = tilePages(texts("你好好休息吧", "你说", "迷说", "米说"))
        assertEquals(
            listOf(listOf(TileCell(0, 2), TileCell(1, 1), TileCell(2, 1)), listOf(TileCell(3, 1))),
            pages[0],
        )
    }

    @Test
    fun pageBreaksEveryFourRows() {
        // 9 条两字短词：每行 4 个 ⇒ 3 行；页容量 4 行 ⇒ 单页放得下。
        val pages = tilePages(List(9) { "字${'a' + it}" })
        assertEquals(1, pages.size)
        assertEquals(3, pages[0].size)
        // 17 条：5 行 ⇒ 第二页只有 1 行（夹具刻意都用两字，别把「十」号长度的混进来）。
        val pages2 = tilePages(List(17) { "字${'a' + it}" })
        assertEquals(2, pages2.size)
        assertEquals(4, pages2[0].size)
        assertEquals(1, pages2[1].size)
        assertEquals(TileCell(16, 1), pages2[1][0][0])
    }

    @Test
    fun emptyListYieldsNoPages() {
        assertEquals(0, tilePages(emptyList()).size)
    }
}
