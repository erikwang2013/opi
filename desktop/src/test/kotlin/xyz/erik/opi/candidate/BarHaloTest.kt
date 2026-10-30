// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏「错落溢出」窗口几何的门禁（2026-10-30）：边距/窗口尺寸/岛屿锚点/
//! AWT 裁剪形状。判据都是纯几何 —— 不需要真开窗口，CI headless 也能跑。
//!
//! 关键不变量：
//! * 窗口 = 条 + 两侧边距，尺寸随 StatusBarScale 等比；
//! * 每个岛屿中心都在裁剪形状里，且咬进条体（不出现悬空装饰）；
//! * 形状不越出窗口，锚点之间留空（透明区是真的空，不是画了一块）；
//! * 极窄条不抛异常、所有锚点收在窗口中心。

package xyz.erik.opi.candidate

import xyz.erik.opi.scene.HALO_BOTTOM_ANCHORS
import xyz.erik.opi.scene.HALO_TOP_ANCHORS
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class BarHaloTest {

    private val halo = 22f
    private val barHeight = 42f
    private val barWidth = 360f
    private val windowWidth = barWidth + 2f * halo
    private val windowHeight = barHeight + 2f * halo

    @Test
    fun `window size is the bar plus the halo`() {
        assertEquals(statusBarWidthDp(10, 100, false) + 2f * halo, barWindowWidthDp(10, 100, false), 1e-3f)
        assertEquals(barHeight + 2f * halo, barWindowHeightDp(100, false), 1e-3f)
        // 统一缩放：窗口跟着条一起乘。
        assertEquals(
            barWindowWidthDp(10, 150, false),
            statusBarWidthDp(10, 150, false) + 2f * barHalo(150, false).value,
            1e-3f,
        )
        // Windows 紧凑系数也乘到边距上（否则 Windows 的边距会比 Linux 大一圈）。
        assertEquals(
            barWindowHeightDp(100, true),
            barMetrics(100, true).height.value + 2f * barHalo(100, true).value,
            1e-3f,
        )
        assertTrue(barWindowWidthDp(10, 150, false) > barWindowWidthDp(10, 100, false))
    }

    @Test
    fun `island anchors stay inside the window and keep their order`() {
        val xs = haloAnchorXs(windowWidth, halo, HALO_TOP_ANCHORS)
        assertEquals(HALO_TOP_ANCHORS.size, xs.size)
        val margin = halo * 1.35f + 1f
        for (x in xs) {
            assertTrue("锚点越出窗口左边：$x", x >= margin - 1e-3f)
            assertTrue("锚点越出窗口右边：$x", x <= windowWidth - margin + 1e-3f)
        }
        assertEquals("比例锚点的顺序不该被打乱", xs.sorted(), xs)
    }

    @Test
    fun `very narrow bar collapses anchors to the center instead of throwing`() {
        // 极端：只剩模式键（条宽可能 ~100dp），边距比窗口一半还宽。
        val narrow = 60f
        val xs = haloAnchorXs(narrow, halo, HALO_TOP_ANCHORS)
        assertEquals(HALO_TOP_ANCHORS.size, xs.size)
        assertTrue(xs.all { it == narrow / 2f })
        // 形状本身仍可构造，且不越界。
        val area = barWindowShape(narrow, halo * 2f + barHeight, halo, barHeight, 0f)
        assertEquals(narrow / 2f, area.bounds.centerX.toFloat(), 1e-3f)
    }

    @Test
    fun `islands overlap the bar and stay inside the window`() {
        val islands = haloIslands(windowWidth, windowHeight, halo, barHeight)
        assertEquals(HALO_TOP_ANCHORS.size + HALO_BOTTOM_ANCHORS.size, islands.size)
        val barTop = halo
        val barBottom = halo + barHeight
        for (i in islands) {
            assertTrue("岛屿越出左边：$i", i.cx - i.rx >= -1e-3f)
            assertTrue("岛屿越出右边：$i", i.cx + i.rx <= windowWidth + 1e-3f)
            assertTrue("岛屿越出上边：$i", i.cy - i.ry >= -1e-3f)
            assertTrue("岛屿越出下边：$i", i.cy + i.ry <= windowHeight + 1e-3f)
        }
        // 上半 islands 的底沿咬进条体、下半的顶沿咬进条体。
        val top = islands.take(HALO_TOP_ANCHORS.size)
        val bottom = islands.drop(HALO_TOP_ANCHORS.size)
        assertTrue(top.all { it.cy + it.ry > barTop })
        assertTrue(bottom.all { it.cy - it.ry < barBottom })
    }

    @Test
    fun `shape contains the bar and every island, but not the transparent gaps`() {
        val area = barWindowShape(windowWidth, windowHeight, halo, barHeight, 0f)
        assertTrue("条体中心该在形状里", area.contains((windowWidth / 2f).toDouble(), (halo + barHeight / 2f).toDouble()))
        for (i in haloIslands(windowWidth, windowHeight, halo, barHeight)) {
            assertTrue("岛屿中心该在形状里：$i", area.contains(i.cx.toDouble(), i.cy.toDouble()))
        }
        // 两个顶部岛之间的窗口顶边应当是透明的（否则遮罩等于没做）。
        val gapX = (haloAnchorXs(windowWidth, halo, HALO_TOP_ANCHORS)[0] +
            haloAnchorXs(windowWidth, halo, HALO_TOP_ANCHORS)[1]) / 2f
        assertFalse("岛与岛之间不该有形状", area.contains(gapX.toDouble(), 1.0))
        // 形状不越出窗口。
        assertTrue(area.bounds.minX >= -0.5)
        assertTrue(area.bounds.minY >= -0.5)
        assertTrue(area.bounds.maxX <= windowWidth + 0.5)
        assertTrue(area.bounds.maxY <= windowHeight + 0.5)
    }

    @Test
    fun `windows rounded bar keeps its corners clipped`() {
        val r = 4f
        val rounded = barWindowShape(windowWidth, windowHeight, halo, barHeight, r)
        // 左下角内侧 0.5px：不在任何椭圆岛下面，能真正量到条的圆角。
        val cornerX = (halo + 0.5f).toDouble()
        val cornerY = (halo + barHeight - 0.5f).toDouble()
        assertFalse(rounded.contains(cornerX, cornerY))
        assertTrue(rounded.contains((halo + r).toDouble(), (halo + barHeight - r).toDouble()))
        // Linux 直角形状包含同一个角点。
        val square = barWindowShape(windowWidth, windowHeight, halo, barHeight, 0f)
        assertTrue(square.contains(cornerX, cornerY))
    }
}
