// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

/**
 * 「键盘模式」（2026-10-09，对齐搜狗移动端）。**纯界面状态**（除繁体外）：
 * 繁体是**引擎模式**（EngineMode.TRADITIONAL），不在这里存 —— 菜单里的
 * 「繁体模式」项直接读/写 [xyz.erik.opi.engine.EngineController] 的 mode。
 *
 * 每个模式的落地层（如实记，改之前先看）：
 *   · [floating] / [game]：**IME 窗口尺寸**（OpiImeService.onConfigureWindow）——
 *     悬浮 = 窄一些的居中窗口；游戏 = 更矮、半透明尽量少挡画面。⚠️ 二者
 *     **本机无真机可验**（窗口行为只能真机看），实现按 Android IME 窗口惯例写，
 *     如实标在 docs/install/android.md。
 *   · [oneHand]：**布局层**（ImeScreen 里键盘整体偏左/偏右 + 收窄）——纯 Compose。
 *   · [dark]：**配色层**（Palette.kt 的 DarkPalette）。
 *   · [elder]：**配色 + 字号**（ElderPalette + ImeScreen 的整块缩放）。
 */
data class KeyboardModes(
    val floating: Boolean = false,
    val game: Boolean = false,
    val oneHand: OneHand = OneHand.OFF,
    val dark: Boolean = false,
    val elder: Boolean = false,
) {
    enum class OneHand { OFF, LEFT, RIGHT }

    /** 菜单里显示的「开了哪些」（繁体在 controller.mode 上，单独问）。 */
    val anyOn: Boolean
        get() = floating || game || oneHand != OneHand.OFF || dark || elder
}
