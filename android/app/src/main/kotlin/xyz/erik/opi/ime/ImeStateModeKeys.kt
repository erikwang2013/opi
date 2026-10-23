// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import xyz.erik.opi.engine.EngineMode

/**
 * 底行模式键的动作（2026-10-24 从 [ImeState] 拆出：500 行门禁；语义未动）。
 *
 * `toggleTraditional` = 9 键/笔画：**只切简/繁**（2026-10-15 用户裁决），走
 * [EngineController.switchLanguage] 的「中→繁」腿：方案不动、缓冲保留
 * （2026-10-09「拼音保留可继续使用」）；键面字 = 点它会切到哪
 * （[jianFanModeKeyLabelOf]，ImeScreen.kt）。
 *
 * `toggleChineseEnglish` = 14 键底行「中英」：中文 → 英文 26 键
 * （[ImeState.switchToEnglishKeyboard]，先按拼音上屏 pending）；英文 → 回上次中文方案，
 * 复用 [ImeState.cycleMode] 的英→中腿（**先清英文缓冲、不提交**，2026-10-19）。
 * 长按这枚键开输入模式面板（[ImeState.openInputMode]），由布局层接。
 */
fun ImeState.toggleTraditional() {
    val next = if (controller.mode == EngineMode.TRADITIONAL) EngineMode.PINYIN else EngineMode.TRADITIONAL
    controller.switchLanguage(next, controller.inputScheme)
}

fun ImeState.toggleChineseEnglish() {
    if (controller.mode == EngineMode.ENGLISH) cycleMode() else switchToEnglishKeyboard()
}

/** 9 键左列「自定义」标点（2026-10-24）：更新键面 + 落盘（空串 = 清回「自定义」）。 */
fun ImeState.selectCustomPunct(symbol: String) {
    customPunct = symbol
    persist()
}
