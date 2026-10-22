// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import kotlin.math.min

/**
 * IME 窗口高度（px）的纯计算 —— 屏幕尺寸/朝向/平板/分屏都只走这一处
 * （`OpiImeService.keyboardHeight` 只负责取窗口尺寸与 density）。
 *
 * 原公式 `0.48 × 短边 + 76dp + 38dp` 在竖屏手机上成立（约屏高 35%），但**横屏时短边
 * 就是屏高**：1080×2400 横过来后键盘要 831px，占屏高 **77%**，被输入应用只剩 23%。
 * 平板横屏同理（短边 = 高度，键盘按宽度线性膨胀到 62%）。两处都不是「大屏更好用」，
 * 而是把应用挤没。
 *
 * 规则（都不改竖屏手机的既有像素）：
 * · 横屏（宽 > 高）：键区最多占窗口高度的 [LANDSCAPE_MAX_RATIO]；
 * · 任意朝向：绝对上限 [MAX_HEIGHT_DP] —— 平板上键高不再随宽度无限放大；
 * · 最后再钳到窗口高度内（分屏/自由窗口窗口本身就窄，绝不能算出比窗口还高的键盘）。
 *
 * 窗口高度含系统栏 inset：`decorFitsSystemWindows` 默认 true，导航栏/手势条由系统
 * 以 padding 让出（见 OpiImeService 的注释），所以这里拿到的是可用窗口的完整高度。
 */
internal object ImeWindowMetrics {

    /** 键区占短边的比例（竖屏手机既有值，勿动）。 */
    const val KEY_AREA_RATIO = 0.48f

    /** 键区在比例基数之外额外加的高度（dp；竖屏手机既有值，勿动）。 */
    const val KEY_AREA_EXTRA_DP = 76

    /** 横屏：键盘最多占窗口高度。横屏手机实测原值 77%，55–60% 接近主流输入法。 */
    const val LANDSCAPE_MAX_RATIO = 0.60f

    /** 任意朝向的绝对上限（dp）：平板上键高不随宽度线性膨胀。 */
    const val MAX_HEIGHT_DP = 340

    fun keyboardHeightPx(windowWidthPx: Int, windowHeightPx: Int, density: Float): Int {
        val side = min(windowWidthPx, windowHeightPx)
        val raw = side * KEY_AREA_RATIO +
            KEY_AREA_EXTRA_DP * density +
            COMPOSING_BAR_HEIGHT_DP * density
        val absoluteCap = MAX_HEIGHT_DP * density
        val cap = if (windowWidthPx > windowHeightPx) {
            min(windowHeightPx * LANDSCAPE_MAX_RATIO, absoluteCap)
        } else {
            min(windowHeightPx.toFloat(), absoluteCap)
        }
        return raw.toInt().coerceAtMost(cap.toInt()).coerceAtLeast(1)
    }
}
