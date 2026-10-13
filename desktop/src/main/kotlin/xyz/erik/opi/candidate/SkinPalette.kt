// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 皮肤调色板（桌面自绘窗共用）——**值的唯一源 = `scripts/fcitx5_skins.py` 的 SKINS**，
//! 本文件只是它的 Kotlin 镜像（同 `SettingsWindow` 的预览卡先例，2026-10-12 把预览的
//! 三色镜像扩成整块角色表：状态栏也要跟着皮肤走 —— 用户报障「设置皮肤后状态栏皮肤
//! 没同步变化」的根因是条的颜色全写死、从没读过 `Skin=`）。
//!
//! 角色映射（一个不新造颜色，全取 SKINS 已有的键）：
//!   bg ← `bg`、fg ← `text`、hover ← `hl_bg`（带 alpha，即 fcitx5 高亮同款）、
//!   line ← `sep`、accent ← `accent`（条里的提示文字色）
//!
//! 同步机制：状态栏进程本来就**每秒重读配置文件**（`StatusBarApp` 的 1s 轮询）——
//! 设置窗保存 `Skin=` 后 ≤1s 条自己换色，没有第二套通知通道。
//!
//! 「不改动」（[SkinOption.KEEP]）= 条自绘以来的固定深色（[KeepColors]）—— OPI
//! 不介入主题，条也就不换装（行为与本次改动之前一字不差）。
//!
//! ⚠️ 锁同步门禁：`SettingsTest.skin palettes match the theme generator` 按文本读
//! 本文件与 `fcitx5_skins.py` 逐值比对（五个角色全比）—— 改了这里或那里都会红；
//! 改格式（如换 Color 写法）先看那条测试。

package xyz.erik.opi.candidate

import androidx.compose.runtime.compositionLocalOf
import androidx.compose.ui.graphics.Color

/** 一款皮肤的取色角色（见文件头的映射表）。 */
data class SkinColors(
    val bg: Color,
    val fg: Color,
    val hover: Color,
    val line: Color,
    val accent: Color,
)

/**
 * 「不改动」/缺省 = 状态栏自绘以来的固定深色。
 * ⚠️ 这五个值就是 2026-10-12 之前 `StatusBar.kt` 里的 `BarBg/BarFg/BarHover/BarLine/BarDim`
 * 常量，原样搬来 —— KEEP 的行为必须一字不变，别顺手「调好看点」。
 */
internal val KeepColors = SkinColors(
    bg = Color(0xFF2D2D2D),
    fg = Color(0xFFE8E8E8),
    hover = Color(0xFF3F3F3F),
    line = Color(0xFF4A4A4A),
    accent = Color(0xFFFF8A80),
)

/**
 * 自绘窗的调色板（`StatusBarApp` 根部 provide 一次，子 composable 读它）——
 * 与 Android 侧 `LocalKeyboardPalette` 同一套做法：色板由「当前皮肤」决定，
 * 逐层传参会在每个新 composable 上再开一个口子。
 */
internal val LocalBarPalette = compositionLocalOf { KeepColors }

/**
 * 皮肤 → 角色表。九款的顺序 = `SkinOption` 枚举序 = `fcitx5_skins.py` 的键序
 * （三处同序由 `SettingsTest` 的 `skin names stay in lockstep` 钉住）。
 * hover 的 `Color(0xAARRGGBB)` 头两位 = SKINS 里 `hl_bg` 尾两位 alpha
 * （如 `#C0392B1F` → `0x1FC0392B`）。
 */
internal fun skinColors(skin: SkinOption): SkinColors = when (skin) {
    SkinOption.KEEP -> KeepColors
    SkinOption.LIGHT -> SkinColors(
        bg = Color(0xFFFFFFFF),
        fg = Color(0xFF1F1F1F),
        hover = Color(0x1FC0392B),
        line = Color(0xFFD8D8D8),
        accent = Color(0xFFC0392B),
    )
    SkinOption.DARK -> SkinColors(
        bg = Color(0xFF1F1F1F),
        fg = Color(0xFFE6E6E6),
        hover = Color(0x26FF8A80),
        line = Color(0xFF4A4A4A),
        accent = Color(0xFFFF8A80),
    )
    SkinOption.PAPER -> SkinColors(
        bg = Color(0xFFF5EFE1),
        fg = Color(0xFF3D3222),
        hover = Color(0x1F8A5A2B),
        line = Color(0xFFD8CDB4),
        accent = Color(0xFF8A5A2B),
    )
    SkinOption.CINNABAR -> SkinColors(
        bg = Color(0xFFB3272D),
        fg = Color(0xFFFFF6EC),
        hover = Color(0x26FFFFFF),
        line = Color(0xFFCC5A5F),
        accent = Color(0xFFFFD9A0),
    )
    SkinOption.FRESH -> SkinColors(
        bg = Color(0xFFE6F5EE),
        fg = Color(0xFF16332B),
        hover = Color(0x262E7D6B),
        line = Color(0xFFB9D8CB),
        accent = Color(0xFF2E7D6B),
    )
    SkinOption.AZURE -> SkinColors(
        bg = Color(0xFF3A679E),
        fg = Color(0xFFF4F8FD),
        hover = Color(0x26FFFFFF),
        line = Color(0xFF6E96C4),
        accent = Color(0xFFFFE3A8),
    )
    SkinOption.LEMON -> SkinColors(
        bg = Color(0xFFFBEE7E),
        fg = Color(0xFF3A3000),
        hover = Color(0x267E6200),
        line = Color(0xFFD1C462),
        accent = Color(0xFF7E6200),
    )
    SkinOption.SUNSET -> SkinColors(
        bg = Color(0xFFF7C79C),
        fg = Color(0xFF4A2317),
        hover = Color(0x26A63412),
        line = Color(0xFFD1A37F),
        accent = Color(0xFFA63412),
    )
    SkinOption.STARLIGHT -> SkinColors(
        bg = Color(0xFF1B2138),
        fg = Color(0xFFE8EBF7),
        hover = Color(0x33FFD966),
        line = Color(0xFF3A4468),
        accent = Color(0xFFFFD966),
    )
    SkinOption.WISTERIA -> SkinColors(
        bg = Color(0xFFEDE7FB),
        fg = Color(0xFF3A2A63),
        hover = Color(0x266B4BC4),
        line = Color(0xFFCFC4EC),
        accent = Color(0xFF6B4BC4),
    )
    SkinOption.PINE -> SkinColors(
        bg = Color(0xFF1E3A2F),
        fg = Color(0xFFDCEEE2),
        hover = Color(0x337FD3A2),
        line = Color(0xFF3C5C4C),
        accent = Color(0xFF7FD3A2),
    )
}
