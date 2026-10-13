// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.unit.dp

/**
 * 工具条与菜单的**自绘矢量图标**（24×24 视口，线稿风格）。
 *
 * 为什么不引 `material-icons-extended`：那是数千个图标的整包依赖，而本模块没有
 * 裁剪配置（IME 是常驻进程，包体积实打实）—— 为四个图标拉整包不值当。
 * [docs/opi-pet.svg] 的既有先例也是自绘矢量。
 *
 * 全部**线稿 + `strokeLineCap.Round`**：图标颜色由调用方 `Icon(tint=…)` 决定
 * （这里填黑仅作占位，`Icon` 会整体染色），故换肤时图标跟着工具条色走。
 */
object OpiIcons {

    /** 表情入口（笑脸）。 */
    val Emoji: ImageVector by lazy {
        ImageVector.Builder(
            name = "OpiEmoji",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            stroke(width = 1.8f) {
                // 头（整圆：两段半圆闭合）
                moveTo(12f, 3f)
                arcToRelative(9f, 9f, 0f, true, true, 0f, 18f)
                arcToRelative(9f, 9f, 0f, true, true, 0f, -18f)
                close()
                // 双眼（短竖线 + 圆头 = 点）
                moveTo(9f, 9.2f)
                lineTo(9f, 10.2f)
                moveTo(15f, 9.2f)
                lineTo(15f, 10.2f)
                // 笑弧（椭圆弧，浅弧向下鼓）
                moveTo(8f, 13.6f)
                arcToRelative(4.2f, 3.2f, 0f, false, false, 8f, 0f)
            }
        }.build()
    }

    /** 输入模式入口（键盘：外框 + 几个键点 + 空格条）。 */
    val InputMode: ImageVector by lazy {
        ImageVector.Builder(
            name = "OpiInputMode",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            stroke(width = 1.8f) {
                moveTo(3f, 6f)
                lineTo(21f, 6f)
                lineTo(21f, 18f)
                lineTo(3f, 18f)
                close()
                // 键点两条 + 空格条
                moveTo(7f, 9.5f)
                lineTo(10f, 9.5f)
                moveTo(7f, 12.5f)
                lineTo(10f, 12.5f)
                moveTo(7f, 15f)
                lineTo(17f, 15f)
            }
        }.build()
    }

    /** 键盘模式入口（滑杆：三档开关的通用写法）。 */
    val KeyboardMode: ImageVector by lazy {
        ImageVector.Builder(
            name = "OpiKeyboardMode",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            stroke(width = 1.8f) {
                // 三根滑轨
                moveTo(4f, 7f)
                lineTo(20f, 7f)
                moveTo(4f, 12f)
                lineTo(20f, 12f)
                moveTo(4f, 17f)
                lineTo(20f, 17f)
                // 三个旋钮（短竖线加粗表意：位置各不同）
                moveTo(9f, 5.6f)
                lineTo(9f, 8.4f)
                moveTo(15f, 10.6f)
                lineTo(15f, 13.4f)
                moveTo(7f, 15.6f)
                lineTo(7f, 18.4f)
            }
        }.build()
    }

    /** 皮肤入口（调色板：圆 + 四枚颜料点 + 一角留缺口）。 */
    val Palette: ImageVector by lazy {
        ImageVector.Builder(
            name = "OpiPalette",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            stroke(width = 1.8f) {
                // 盘身：整圆（两段半圆闭合，同 Emoji 的头部）
                moveTo(12f, 3f)
                arcToRelative(9f, 9f, 0f, true, true, 0f, 18f)
                arcToRelative(9f, 9f, 0f, true, true, 0f, -18f)
                close()
                // 四枚颜料点（短线段 + 圆头 = 点；位置错开，右下留远一点当「拇指缺口」的暗示）
                moveTo(8.6f, 8.2f)
                lineTo(8.6f, 9.2f)
                moveTo(15.4f, 8.6f)
                lineTo(15.4f, 9.6f)
                moveTo(8.0f, 13.4f)
                lineTo(8.0f, 14.4f)
                moveTo(13.2f, 15.2f)
                lineTo(13.2f, 16.2f)
            }
        }.build()
    }

    /** 返回（表情页工具条左格，2026-10-12）：一横一折的箭头。 */
    val ArrowBack: ImageVector by lazy {
        ImageVector.Builder(
            name = "OpiArrowBack",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            stroke(width = 1.8f) {
                moveTo(20f, 12f)
                lineTo(4f, 12f)
                moveTo(11f, 5f)
                lineTo(4f, 12f)
                lineTo(11f, 19f)
            }
        }.build()
    }

    /** 搜索（表情页工具条右格）：镜圈 + 手柄。 */
    val Search: ImageVector by lazy {
        ImageVector.Builder(
            name = "OpiSearch",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            stroke(width = 1.8f) {
                moveTo(11f, 4f)
                arcToRelative(7f, 7f, 0f, true, true, 0f, 14f)
                arcToRelative(7f, 7f, 0f, true, true, 0f, -14f)
                close()
                moveTo(16f, 16f)
                lineTo(20.5f, 20.5f)
            }
        }.build()
    }

    /** 关闭/收起（菜单与面板右上角）。 */
    val Close: ImageVector by lazy {
        ImageVector.Builder(
            name = "OpiClose",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            stroke(width = 1.8f) {
                moveTo(6f, 6f)
                lineTo(18f, 18f)
                moveTo(18f, 6f)
                lineTo(6f, 18f)
            }
        }.build()
    }

    /**
     * 收起键盘（2026-10-10 用户裁决「隐藏键盘按钮图标不要用 x」）：
     * 向下折角（⌄ 的线稿版）—— 输入法「收起」的通用语；✕ 语义是关闭，仍归 [Close]
     * （关面板/关页在用，两者别混）。
     */
    val KeyboardHide: ImageVector by lazy {
        ImageVector.Builder(
            name = "OpiKeyboardHide",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            stroke(width = 2.0f) {
                moveTo(6f, 10f)
                lineTo(12f, 16f)
                lineTo(18f, 10f)
            }
        }.build()
    }
}

/**
 * 建一条线稿路径（本文件四个图标的共同参数：只描边、不填充、圆头圆角）。
 * 抽出来只为四个图标参数一致 —— 不是为了复用花样。
 *
 * ⚠️ `fill = null` 是承重的，别"顺手补上颜色"：首版这里写的是
 * `fill = SolidColor(fill)`，于是**闭环子路径全被填成实心** —— 笑脸变实心圆饼、
 * 键盘变实心方块（2026-10-09 渲染复核抓到的，数值检查看不见这种形态）。
 * 填色交给 `Icon(tint=…)` 统一染色，本文件一律线稿。
 */
private fun ImageVector.Builder.stroke(
    width: Float,
    block: androidx.compose.ui.graphics.vector.PathBuilder.() -> Unit,
) {
    path(
        fill = null,
        stroke = SolidColor(Color.Black),
        strokeLineWidth = width,
        strokeLineCap = StrokeCap.Round,
        strokeLineJoin = androidx.compose.ui.graphics.StrokeJoin.Round,
        pathBuilder = block,
    )
}
