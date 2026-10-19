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
