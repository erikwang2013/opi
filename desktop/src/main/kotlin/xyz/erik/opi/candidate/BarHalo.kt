// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏「错落溢出」的窗口几何（2026-10-30 用户裁决：桌面端允许特色背景从
//! 状态栏错落探出来）。
//!
//! 做法：条窗外加一圈透明边距（[BAR_HALO_DP]），角色画在边距里；窗口用 AWT
//! `Window.shape` 裁成「条 + 若干椭圆岛」的并集 ——：
//!   1. 透明边距里只有岛屿可见，其余是真正的桌面；
//!   2. 点击也只落在条和岛屿上，透明区不挡桌面（矩形透明窗会挡住下面所有点击）。
//!
//! 岛屿的水平锚点与 `shared/scene` 的插画锚点**同源**（[haloAnchorXs] 由
//! `HALO_TOP_ANCHORS`/`HALO_BOTTOM_ANCHORS` 算，插画也吃同一批 x）；窄条上
//! 还要收进窗口内，否则第一个岛会被窗口左缘裁掉。
//!
//! 平台兜底：透明度或窗口裁剪任一不支持（无合成器的 X11、极老 JDK）⇒
//! [supportsOverflowWindow] 返回 false，调用方把边距设 0、窗口恢复成条大小 ——
//! 不能出现「画不出来还挡点击」的透明矩形。
//!
//! shortcut: Windows 上没有单独的输入形状 API，`Window.shape` 同时决定视觉与命中；
//! 条外的椭圆岛会吃掉落在装饰上的点击（点到装饰无动作，但会挡住下面窗口）。岛很小、
//! 只在条沿，先接受；要彻底透传需挂 WM_NCHITTEST 返回 HTTRANSPARENT，等有 Windows
//! 真机验收再上。Linux 已用 XShape 的 ShapeInput 把装饰全部透传（见 SkipTaskbar）。

package xyz.erik.opi.candidate

import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import xyz.erik.opi.scene.HALO_BOTTOM_ANCHORS
import xyz.erik.opi.scene.HALO_TOP_ANCHORS
import java.awt.GraphicsDevice
import java.awt.GraphicsEnvironment
import java.awt.geom.Area
import java.awt.geom.Ellipse2D
import java.awt.geom.Rectangle2D
import java.awt.geom.RoundRectangle2D

/** 条四周的透明边距（100% 时）。随 [barScaleFactor] 等比，Windows 同乘紧凑系数。 */
internal const val BAR_HALO_DP = 22f

/** 窗口裁剪用的小圆角（Windows 条体同款 4dp）；Linux 直角。 */
internal const val BAR_CORNER_DP = 4f

internal fun barHalo(scale: Int, windows: Boolean): Dp =
    (BAR_HALO_DP * barScaleFactor(scale, windows)).dp

/** 窗口宽 = 条宽 + 两侧透明边距。 */
internal fun barWindowWidthDp(buttonCount: Int, scale: Int, windows: Boolean): Float =
    statusBarWidthDp(buttonCount, scale, windows) + 2f * barHalo(scale, windows).value

/** 窗口高 = 条高 + 上下透明边距。 */
internal fun barWindowHeightDp(scale: Int, windows: Boolean): Float =
    barMetrics(scale, windows).height.value + 2f * barHalo(scale, windows).value

/**
 * 一个条外岛屿（椭圆）。坐标是**窗口像素**（AWT 的 Shape 空间）。
 * 岛的上下沿都咬进条体 0.35×halo，保证与条连成一个整体、不出现悬空碎片。
 */
internal data class HaloIsland(
    val cx: Float,
    val cy: Float,
    val rx: Float,
    val ry: Float,
)

/**
 * 把比例锚点收进窗口：左右至少留 `1.35×halo`（最大岛半径）+ 1dp 安全边。
 * 极窄条（全部按钮关掉、只剩模式键）时所有锚点收在中心，岛会叠在一起 ——
 * 那种宽度本来也放不下四个角色，叠着仍是一团可用的装饰。
 */
internal fun haloAnchorXs(windowWidth: Float, halo: Float, fractions: List<Float>): List<Float> {
    val margin = halo * 1.35f + 1f
    if (windowWidth <= 2f * margin) return fractions.map { windowWidth / 2f }
    return fractions.map { (windowWidth * it).coerceIn(margin, windowWidth - margin) }
}

/** 条窗裁剪形状 = 条矩形 + 上/下各一组错落椭圆岛。纯几何，单测钉住。 */
internal fun haloIslands(
    windowWidth: Float,
    windowHeight: Float,
    halo: Float,
    barHeight: Float,
): List<HaloIsland> {
    val barTop = halo
    val barBottom = halo + barHeight
    val topXs = haloAnchorXs(windowWidth, halo, HALO_TOP_ANCHORS)
    val bottomXs = haloAnchorXs(windowWidth, halo, HALO_BOTTOM_ANCHORS)
    // 岛顶/底各留 1dp 安全边；顶部岛宽窄交替 = 「错落」。
    val topY = halo * 0.05f
    val topBottomY = barTop + halo * 0.35f
    val bottomTopY = barBottom - halo * 0.35f
    val bottomY = windowHeight - halo * 0.05f
    val top = topXs.mapIndexed { i, x ->
        HaloIsland(
            cx = x,
            cy = (topY + topBottomY) / 2f,
            rx = halo * (1.15f + 0.15f * (i % 2)),
            ry = (topBottomY - topY) / 2f,
        )
    }
    val bottom = bottomXs.mapIndexed { i, x ->
        HaloIsland(
            cx = x,
            cy = (bottomTopY + bottomY) / 2f,
            rx = halo * (1.05f + 0.15f * (i % 2)),
            ry = (bottomY - bottomTopY) / 2f,
        )
    }
    return top + bottom
}

/**
 * AWT 窗口形状（像素坐标）。[corner] = 条体圆角（px；Linux 传 0）。
 * 形状越界会被窗口裁掉，所以岛屿的锚点先经 [haloAnchorXs] 收口。
 */
internal fun barWindowShape(
    width: Float,
    height: Float,
    halo: Float,
    barHeight: Float,
    corner: Float,
): Area {
    val barW = width - 2f * halo
    val bar = if (corner > 0f) {
        RoundRectangle2D.Float(halo, halo, barW, barHeight, corner, corner)
    } else {
        Rectangle2D.Float(halo, halo, barW, barHeight)
    }
    val area = Area(bar)
    for (i in haloIslands(width, height, halo, barHeight)) {
        area.add(Area(Ellipse2D.Float(i.cx - i.rx, i.cy - i.ry, i.rx * 2f, i.ry * 2f)))
    }
    return area
}

/**
 * 本机能不能做「透明边距 + 窗口裁剪」。
 * 先问 AWT 的 per-pixel 透明支持，再在**未显示**的临时窗口上试一次 `shape`
 * （JDK 没有公开的「支持窗口裁剪」查询；`setShape` 不支持时抛异常）。
 */
internal fun supportsOverflowWindow(): Boolean {
    val translucent = runCatching {
        GraphicsEnvironment.getLocalGraphicsEnvironment().defaultScreenDevice
            .isWindowTranslucencySupported(GraphicsDevice.WindowTranslucency.PERPIXEL_TRANSPARENT)
    }.getOrDefault(false)
    if (!translucent) return false
    return runCatching {
        val w = java.awt.Window(null)
        try {
            w.shape = Rectangle2D.Float(0f, 0f, 1f, 1f)
            true
        } finally {
            w.dispose()
        }
    }.getOrDefault(false)
}
