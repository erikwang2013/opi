// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

/**
 * 颜文字分类：`assets/kaomoji.tsv`（`颜文字<TAB>分类`，数据侧生成入库）的解析。
 *
 * 与 [SymbolCategories] 同一条先例：解析是**纯函数**，读资产由调用方做（面板第一次
 * 打开时读，别在服务启动时读）；坏行（缺列 / 空颜文字 / 空分类）**跳过而不抛**
 * —— 用户设备上的文件可能被替换或截断，页面该少几组而不是崩 IME。
 *
 * 分类按**首次出现顺序**归并（同一分类的条目并进第一处），`start` 是拼平后列表里
 * 的累计下标（点分类条 = 取 `[start, start+items.size)` 切片）。数据按分类连续排列时，
 * 拼平顺序 = 文件序。
 */
object KaomojiCategories {

    /** 一个分类：名称、在拼平列表里的起始下标、条目（表序 = 文件序）。 */
    data class Category(val label: String, val start: Int, val items: List<String>)

    fun parse(text: String): List<Category> {
        val order = LinkedHashMap<String, MutableList<String>>()
        for (line in text.lineSequence()) {
            if (line.isBlank()) continue
            val parts = line.split('\t', limit = 2)
            if (parts.size < 2) continue
            val item = parts[0]
            val label = parts[1]
            if (item.isBlank() || label.isBlank()) continue
            order.getOrPut(label) { mutableListOf() }.add(item)
        }
        var start = 0
        return order.map { (label, items) ->
            Category(label, start, items).also { start += items.size }
        }
    }

    /** 拼平后的全部颜文字（浏览/搜索共用的同一序）。 */
    fun flatten(categories: List<Category>): List<String> = categories.flatMap { it.items }
}
