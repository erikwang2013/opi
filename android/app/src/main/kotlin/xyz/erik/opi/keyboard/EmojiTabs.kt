// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

/**
 * 表情页子页。顺序 = 顶栏顺序（2026-10-18 用户裁决：「自有」是**独立顶级分类**，
 * 排第一；默认仍进 Emoji）。
 */
internal enum class EmojiSubTab(val label: String) {
    Pet("自有"),
    Emoji("Emoji"),
    Kaomoji("颜文字"),
}

/**
 * 顶栏实际显示的子页：`showOwn` 缺省 false ⇒ 「自有」整个隐藏（2026-10-20 用户裁决）。
 * 设置页的「表情中显示自有」打开后才加入。
 */
internal fun emojiSubTabs(showOwn: Boolean): List<EmojiSubTab> =
    EmojiSubTab.entries.filter { it != EmojiSubTab.Pet || showOwn }

/**
 * 底部分类条的内容（纯函数，JVM 单测钉住）。
 *
 * - 自有：单段，无分类条 —— **不把「自有」塞进 Emoji 分类条**（用户裁决的独立性）；
 * - Emoji：引擎 9 组原样（`start` 直接对齐 `SymbolCatalog.emoji`，不再做偏移）；
 * - 颜文字：`kaomoji.tsv` 的类目。
 */
internal fun emojiCategoryGroups(
    sub: EmojiSubTab,
    engine: List<SymbolCatalog.EmojiGroup>,
    kaomoji: List<KaomojiCategories.Category>,
): List<SymbolCatalog.EmojiGroup> = when (sub) {
    EmojiSubTab.Pet -> emptyList()
    EmojiSubTab.Emoji -> engine
    EmojiSubTab.Kaomoji -> kaomoji.map {
        SymbolCatalog.EmojiGroup(it.label, it.start, it.items.size)
    }
}

/**
 * 分类条选中项 → 网格要显示的条目（纯函数，JVM 单测钉住）。
 * 组下标越界 / 无分组 → 整表兜底（引擎未就绪时页面不至于空）。
 *
 * 2026-10-30 用户报障「点了后一个分类，显示的是前一个」：组的 `start` 不保证落在
 * 网格行首（8 列下 9 组里 8 组不在行首），旧的 `scrollToItem(start)` 会把**上一组的
 * 尾巴**滚到首行，高亮又从首项反推 ⇒ 看起来选中了上一组。改为按组切片：点哪组就只画
 * 哪组（同鸿蒙 `EmojiPad` 的既有做法）。
 */
internal fun groupSlice(
    all: List<String>,
    groups: List<SymbolCatalog.EmojiGroup>,
    index: Int,
): List<String> {
    val g = groups.getOrNull(index) ?: return all
    val from = g.start.coerceIn(0, all.size)
    val to = (g.start + g.count).coerceIn(from, all.size)
    return all.subList(from, to).toList()
}
