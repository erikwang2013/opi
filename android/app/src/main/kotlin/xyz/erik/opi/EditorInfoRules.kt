// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi

import android.text.InputType
import android.view.inputmethod.EditorInfo

// 编辑器信息（EditorInfo）的两条纯判定：密码场自动英文、回车决策。
// 从 OpiImeService.kt 拆出（2026-10-10，500 行门禁）——纯函数归纯函数文件，
// 服务本体只留生命周期与副作用。单测在 PasswordEditorTest / EnterDecisionTest。

/**
 * 密码类编辑器判定（2026-10-10）：**文本类**里的 PASSWORD / VISIBLE_PASSWORD /
 * WEB_PASSWORD 三种变体。纯函数，单测钉边界。
 * ⚠️ 数字类密码（TYPE_CLASS_NUMBER|TYPE_NUMBER_VARIATION_PASSWORD）**不在列**——
 * 数字面板默认直出数字，没有「被拼音吃掉」的问题，别顺手扩进来。
 */
internal fun isPasswordEditor(inputType: Int): Boolean {
    if (inputType and InputType.TYPE_MASK_CLASS != InputType.TYPE_CLASS_TEXT) return false
    return when (inputType and InputType.TYPE_MASK_VARIATION) {
        InputType.TYPE_TEXT_VARIATION_PASSWORD,
        InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD,
        InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD,
        -> true
        else -> false
    }
}

/** 回车键的决策（纯函数，单测钉）。 */
internal sealed interface EnterDecision {
    /** 执行应用声明的编辑器 action（搜索/发送/下一步…）。 */
    data class PerformAction(val action: Int) : EnterDecision

    /** 提交换行（多行编辑器、或应用未声明 action）。 */
    object Newline : EnterDecision
}

/**
 * 回车决策（2026-10-10 用户报障「换行无效」的订正）：
 * **IME_FLAG_NO_ENTER_ACTION 优先于 action 位** —— 多行编辑器惯例上两样都设
 * （NO_ENTER_ACTION + 一个残留的 action），旧逻辑只看 action 会把它们的回车发成
 * 动作而不是换行；搜狗/AOSP 的判法都是先看这个 flag。纯函数，边界由单测钉。
 */
internal fun enterDecision(imeOptions: Int): EnterDecision {
    val action = imeOptions and EditorInfo.IME_MASK_ACTION
    val noEnterAction = (imeOptions and EditorInfo.IME_FLAG_NO_ENTER_ACTION) != 0
    // 两个「没有动作」的编码都要走换行：UNSPECIFIED=0（未声明）与 NONE=1（明确说没有）。
    val hasAction = action != EditorInfo.IME_ACTION_UNSPECIFIED &&
        action != EditorInfo.IME_ACTION_NONE
    return if (!noEnterAction && hasAction) {
        EnterDecision.PerformAction(action)
    } else {
        EnterDecision.Newline
    }
}
