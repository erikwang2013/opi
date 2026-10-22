// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 平铺读数格数门禁（[tiledReadingCells]）：竖屏 5 格不变，横屏 4 格，分屏 3 格。
 * 高度取自 1080×2400@2.625（键区 + 候选格约 264dp、横屏约 185dp）。
 */
class CandidateTiledSizingTest {

    @Test
    fun portraitKeepsFiveCells() {
        assertEquals(5, tiledReadingCells(264f))
    }

    @Test
    fun landscapeUsesFourCells() {
        assertEquals(4, tiledReadingCells(185f))
    }

    @Test
    fun splitScreenUsesThreeCells() {
        assertEquals(3, tiledReadingCells(120f))
    }

    @Test
    fun thresholdsAreInclusive() {
        assertEquals(5, tiledReadingCells(220f))
        assertEquals(4, tiledReadingCells(160f))
    }
}
