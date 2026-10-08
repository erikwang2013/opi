// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color

/**
 * 键盘配色（2026-10-09 集中化）。**唯一色板** —— 此前各控件里散着字面量
 * （KeyButton/CandidateBar/SymbolPanel），「暗光模式」「长辈模式」要换肤就必须
 * 先收敛到这里；散着改 = 每加一个模式都要再翻一遍全部文件，必漏。
 *
 * 由 [xyz.erik.opi.ime.ImeScreen] 按当前键盘模式 provide 进
 * [LocalKeyboardPalette]，各控件读 `LocalKeyboardPalette.current`。
 * 没包 provider 的地方（测试、单件预览）拿到默认 [LightPalette]。
 *
 * ⚠️ 值本身是既有字面量的原样搬运（浅色板 = 改造前的观感），改配色只改本文件。
 */
data class KeyboardPalette(
    /** 普通键底色 / 键面字色。 */
    val keyBg: Color,
    val keyFg: Color,
    /** 高亮键（⇧ 激活态、候选选中项）底色 / 键面字色。 */
    val keyHighlightBg: Color,
    val keyHighlightFg: Color,
    /** 候选栏底色 / 缓冲字色（半透明）/ 副文字色。 */
    val barBg: Color,
    val barBufferFg: Color,
    val barDimFg: Color,
    /** 强调色（面板标题、页码等）。 */
    val accent: Color,
    /** 面板（符号/生僻字）底色与主字色。 */
    val panelBg: Color,
    val panelFg: Color,
    /** 工具条底色与图标色。 */
    val toolbarBg: Color,
    val toolbarFg: Color,
)

/** 浅色（默认，= 2026-10-09 之前的观感，逐值搬运）。 */
val LightPalette = KeyboardPalette(
    keyBg = Color(0xFFE0E0E0),
    keyFg = Color(0xFF1F1F1F),
    keyHighlightBg = Color(0xFF546E7A),
    keyHighlightFg = Color(0xFFFFFFFF),
    barBg = Color(0xFFEEEEEE),
    barBufferFg = Color(0x8A000000),
    barDimFg = Color(0xFF757575),
    accent = Color(0xFF455A64),
    panelBg = Color(0xFFEEEEEE),
    panelFg = Color(0xFF1F1F1F),
    toolbarBg = Color(0xFFE0E0E0),
    toolbarFg = Color(0xFF455A64),
)

/** 暗光模式：深底浅字（对比度按 WCAG AA 对 18pt 正文宽松档取）。 */
val DarkPalette = KeyboardPalette(
    keyBg = Color(0xFF2A2A2A),
    keyFg = Color(0xFFE6E6E6),
    keyHighlightBg = Color(0xFF4A6572),
    keyHighlightFg = Color(0xFFFFFFFF),
    barBg = Color(0xFF1F1F1F),
    barBufferFg = Color(0xCCE6E6E6),
    barDimFg = Color(0xFF9E9E9E),
    accent = Color(0xFF9FB4BE),
    panelBg = Color(0xFF1F1F1F),
    panelFg = Color(0xFFE6E6E6),
    toolbarBg = Color(0xFF2A2A2A),
    toolbarFg = Color(0xFFCFD8DC),
)

/** 长辈模式：高对比（纯白底 + 纯黑字 + 更大字号由 ImeScreen 的 elder 缩放管）。 */
val ElderPalette = KeyboardPalette(
    keyBg = Color(0xFFFAFAFA),
    keyFg = Color(0xFF000000),
    keyHighlightBg = Color(0xFF263238),
    keyHighlightFg = Color(0xFFFFFFFF),
    barBg = Color(0xFFFFFFFF),
    barBufferFg = Color(0xFF000000),
    barDimFg = Color(0xFF424242),
    accent = Color(0xFF263238),
    panelBg = Color(0xFFFFFFFF),
    panelFg = Color(0xFF000000),
    toolbarBg = Color(0xFFF5F5F5),
    toolbarFg = Color(0xFF263238),
)

/**
 * 由 [xyz.erik.opi.ime.ImeScreen] 提供；`staticCompositionLocalOf` 的取舍：
 * 换肤时整棵键盘子树本来就要重组（颜色全换），不需要细粒度失效。
 */
val LocalKeyboardPalette = staticCompositionLocalOf { LightPalette }
