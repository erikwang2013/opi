// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 窗口/任务栏图标 = 小欧（2026-10-10 用户裁决：「设置页面打开，在菜单栏的图标
//! 也要是 logo 图标」）。
//!
//! **与设置页里那只同一套形状代码**：直接调 shared/pet/OpiPet.kt 的 [drawOpiPet]
//! （[OpiPet] 的底座）——改宠物形状只改那一处，这里没有第二套几何。
//!
//! 用法：`Window(icon = OpiPetWindowIcon, ...)`，**所有造窗处都挂**（设置/候选窗/
//! 状态栏条/虚拟键盘/符号页）；不挂的默认是 Java/CMP 自带的图标，与品牌无关。
//! Linux 的菜单/桌面入口图标不在这里，走 hicolor 的 docs/opi-pet.svg（见
//! build.gradle.kts 的 injectSettingsDesktopEntry）。

package xyz.erik.opi.pet

import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.painter.Painter

/**
 * 小欧的 [Painter]。不透明窗框、位图缓存都由 CMP 管，这里只回答「怎么画」。
 *
 * [intrinsicSize] **必须显式给**：CMP 桌面（Painter.toAwtImage）按它把 Painter
 * 栅格化成窗口图标，留着默认的 Size.Unspecified 会渲出一张空白图（不报错、不崩，
 * 图标悄没声地空掉 —— PetIconTest 有一条钉）。256 是任务栏/窗口列表的常见档位；
 * 方形框下 u = 256/240 的换算让内容（竖向 ~236/240 设计单位）连留白带内容全落框内，
 * 不裁切。
 *
 * 不刻 OPI（showLegend = false）：图标尺寸下刻字只剩几像素高，会糊成脏点
 * （与 OpiPet 对 64dp 以下的同一条理由）。
 */
val OpiPetWindowIcon: Painter = object : Painter() {
    override val intrinsicSize: Size = Size(256f, 256f)

    override fun DrawScope.onDraw() {
        drawOpiPet()
    }
}
