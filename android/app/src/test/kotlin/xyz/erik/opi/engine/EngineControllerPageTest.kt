// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

import org.junit.Assert.assertEquals
import org.junit.After
import org.junit.Before
import org.junit.Test

// 分页换算单测（2026-10-28 自 EngineControllerTest 拆出：加本组用例后源文件越 500 行）。
class EngineControllerPageTest {
    private fun cands(n: Int) = Array(n) { "c$it" }

    @Before fun allowAssociation() { AssociationGate.enabled = true }
    @After fun resetGate() { AssociationGate.enabled = false }

    /** 增量翻页只加深不收缩（2026-11-08「先识别 2 页，然后增量翻页」）。 */
    @Test
    fun ensureCandidatePagesOnlyGrows() {
        val fake = FakeEngine()
        val ctrl = EngineController(fake)
        assertEquals(2 * fake.pageSizeValue, fake.lastLimit)
        ctrl.ensureCandidatePages(3)
        assertEquals(3 * fake.pageSizeValue, fake.lastLimit)
        ctrl.ensureCandidatePages(1) // 不缩回
        assertEquals(3 * fake.pageSizeValue, fake.lastLimit)
        // 新查询回到首屏深度。
        fake.buf = "ab"
        ctrl.refresh()
        assertEquals(2 * fake.pageSizeValue, fake.lastLimit)
    }

    /** 页内下标 → 全局下标 = page*pageSize+k（page0/中途页/末页/页内越界都钉住）。 */
    @Test
    fun selectFromPageMapsPageLocalIndexToGlobalIndex() {
        val fake = FakeEngine().apply {
            pageSizeValue = 5
            cands = cands(12) // 页0=0..4、页1=5..9、末页=10..11
        }
        val ctrl = EngineController(fake)
        ctrl.selectFromPage(0)
        ctrl.selectFromPage(4)
        ctrl.nextPage()
        ctrl.selectFromPage(0)
        ctrl.selectFromPage(4)
        ctrl.nextPage()
        ctrl.selectFromPage(0)
        ctrl.selectFromPage(1)
        ctrl.selectFromPage(2) // 末页只有 2 项：全局下标仍为 10+2（越界由引擎判空）
        assertEquals(listOf(0, 4, 5, 9, 10, 11, 12), fake.selectCalls)
    }
}
