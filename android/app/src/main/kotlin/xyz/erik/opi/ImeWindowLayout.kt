// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi

import android.view.Gravity
import android.view.ViewGroup
import android.view.Window

/**
 * 「键盘模式」里的**窗口层**两种（2026-10-09；2026-11-01 从 `OpiImeService.kt` 拆出，
 * 500 行门禁）：
 *   · 悬浮模式：窗口收窄到屏宽 [FLOATING_WIDTH_RATIO]、水平居中、贴底；
 *   · 游戏模式：窗口更矮（[GAME_HEIGHT_RATIO] × 常规高度）+ 同样收窄居中，
 *     尽量少挡画面。
 * 默认（都关）：全宽 + 常规高度，与 2026-10-09 之前逐像素一致。
 *
 * ⚠️ **本机无真机**：窗口尺寸/重力只能真机看效果（分屏、手势条 inset 都可能影响）。
 * 实现按 Android IME 窗口惯例（setAttributes 改 gravity/width + setLayout 设尺寸），
 * 如实标在 docs/install/android.md；悬浮**不做拖动**（那要处理触摸重设 layout，
 * 没有真机的反馈链不写这个）。
 */
internal fun OpiImeService.applyKeyboardWindow(win: Window) {
    val modes = imeState.keyboardModes
    val height = if (modes.game) {
        (keyboardHeight() * GAME_HEIGHT_RATIO).toInt().coerceAtLeast(1)
    } else {
        keyboardHeight()
    }
    val lp = win.attributes
    if (modes.floating || modes.game) {
        val (w, _) = windowSizePx()
        lp.width = (w * FLOATING_WIDTH_RATIO).toInt()
        win.attributes = lp
        win.setGravity(Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL)
    } else {
        lp.width = ViewGroup.LayoutParams.MATCH_PARENT
        win.attributes = lp
        win.setGravity(Gravity.BOTTOM)
    }
    win.setLayout(lp.width, height)
}

/** 悬浮/游戏模式下窗口宽度占屏宽的比例（收窄居中）。 */
private const val FLOATING_WIDTH_RATIO = 0.86f

/** 游戏模式的窗口高度相对常规键盘高度的比例（更矮，少挡画面）。 */
private const val GAME_HEIGHT_RATIO = 0.72f
