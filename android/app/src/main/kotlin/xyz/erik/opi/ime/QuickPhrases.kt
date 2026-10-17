// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/**
 * 顶部快捷条的状态与纯逻辑（2026-10-17 用户裁决「那你觉得放什么，默认顶部好空」）。
 *
 * 布局：顶部那条 38dp 槽位（原「拼音行」位）改**共用** —— 输入中画拼音行（现状），
 * 空闲画本快捷条（最近上屏的词/短语：点一下直接上屏、长按删除）。种子 = 内置常用语，
 * 用着用着被自己的词逐步顶掉。数据只在安卓宿主（`ImePreferences` 落盘），
 * 不进引擎、不走 ABI —— 其它三端无此条。
 *
 * 进条门槛（[accepts]）：**短（2..12 字符）且含汉字**。直传的英文/单字母/标点、
 * 超长粘贴不会把条灌成噪音 —— 快捷条是「词/短语」的位置，不是上屏历史。
 */
class QuickPhrases(initial: List<String>) {
    /** 当前条目（最前 = 最新）。Compose 可观察。 */
    var items by mutableStateOf(sanitize(initial))
        private set

    /** 变更回调（宿主用来落盘）。只在**真的变了**时触发；[replaceWith] 不触发。 */
    var onChanged: (List<String>) -> Unit = {}

    /** 记录一次上屏：合格才进条；已在条里则移到最前（不产生重复）。返回是否变化。 */
    fun record(text: String): Boolean {
        val t = text.trim()
        if (!accepts(t)) return false
        if (items.firstOrNull() == t) return false
        val next = (listOf(t) + items.filter { it != t }).take(MAX)
        if (next == items) return false
        items = next
        onChanged(items)
        return true
    }

    /** 长按删除一条。 */
    fun remove(text: String) {
        val next = items.filter { it != text }
        if (next == items) return
        items = next
        onChanged(items)
    }

    /** 宿主恢复（`ImePreferences` 读回来的列表）。空列表 = 用户清空过，**不**回种子。 */
    fun replaceWith(restored: List<String>) {
        items = sanitize(restored)
    }

    companion object {
        /** 条内容量上限（再多横向就翻不完，也记不住）。 */
        const val MAX = 8

        /** 落盘分隔符（条目内禁止出现 —— [sanitize] 会滤掉这样的项）。 */
        const val SEP = "\u001F"

        /** 首启种子：常用语（用户打自己的词之后会被逐步顶掉）。 */
        val DEFAULT = listOf("你好", "谢谢", "好的", "稍等", "收到")

        /** 进条门槛：2..12 个字符，且至少一个汉字（基本区 + 扩展 A）。 */
        fun accepts(text: String): Boolean {
            if (text.length !in 2..12) return false
            if (text.contains(SEP)) return false
            return text.any { c -> c.code in 0x4E00..0x9FFF || c.code in 0x3400..0x4DBF }
        }

        /** 清洗：去空白/重复、剔除不合格项，保序截断。 */
        fun sanitize(list: List<String>): List<String> {
            val seen = mutableSetOf<String>()
            return list.asSequence()
                .map { it.trim() }
                .filter { accepts(it) && seen.add(it) }
                .take(MAX)
                .toList()
        }
    }
}
