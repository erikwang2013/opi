// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 颜文字数据（2026-10-18）：`data/raw/kaomoji.tsv`（`颜文字<TAB>分类`）的解析与读取。
//!
//! 解析口径**逐行镜像 Android 的 `keyboard/KaomojiCategories.kt`**（同一条「解析是纯
//! 函数、读资产归调用方」的先例）—— 两端读同一份产物，解析漂了就是同一批颜文字在两端
//! 分到不同的组。坏行（缺列 / 空颜文字 / 空分类）**跳过而不抛**：文件可能被替换或
//! 截断，页面该少几组而不是打不开。
//!
//! 数据由 `desktop/build.gradle.kts` 的 `copyKaomojiData` 在**构建期原样拷进资源**
//! （源缺失 = 构建响亮失败），运行期按 classpath 资源 `/kaomoji.tsv` 读 —— 与
//! [loadEmojiData] 同一条路。桌面是 JVM 进程，拿不到 Android assets 里那份。

package xyz.erik.opi.candidate

/** 颜文字分类：名称、拼平列表里的起始下标、条目（表序 = 文件序）。 */
object KaomojiData {

    data class Category(val label: String, val start: Int, val items: List<String>)

    /**
     * 解析 TSV。分类按**首次出现顺序**归并（同一分类的条目并进第一处），`start` 是
     * 拼平后列表里的累计下标（点分类条 = 网格滚到 `start` 所在页）。
     */
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

/** 读 classpath 资源 `/kaomoji.tsv`（构建期从 `data/raw/kaomoji.tsv` 拷入）。 */
fun loadKaomojiData(): List<KaomojiData.Category> {
    val text = KaomojiData::class.java.getResourceAsStream("/kaomoji.tsv")
        ?.bufferedReader()?.use { it.readText() } ?: return emptyList()
    return KaomojiData.parse(text)
}
