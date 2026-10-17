// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import xyz.erik.opi.pet.PetEmojiMap
import xyz.erik.opi.pet.PetMood

/**
 * 「自有」表情 = 小欧形象：网格里画的是 [xyz.erik.opi.pet.OpiPet]（12 个宠物形态），
 * 点击提交的却是**对应的 Unicode emoji**（映射在跨端共享的 [PetEmojiMap]）。
 *
 * [PetMood.SLEEPY] / [PetMood.DEGRADED] 是输入状态（不记事 / 引擎降级），不是表情，
 * 共享映射有意不含它们。分类条依赖 [SymbolCatalog]，所以留在 Android 侧。
 */
object PetEmoji {

    val forms: List<PetEmojiMap.Form> get() = PetEmojiMap.forms

    fun emojiFor(mood: PetMood): String? = PetEmojiMap.emojiFor(mood)

    /**
     * 分类条 = 「自有」+ 引擎的 emoji 分组。
     *
     * 引擎的 `start` 是相对**表情列表**的下标，而网格里表情前面还排着 [forms]
     * ⇒ 各引擎组整体后移；高亮反推直接用 [SymbolCatalog.groupIndexOf]，与 Emoji 页同一套。
     */
    fun categories(engine: List<SymbolCatalog.EmojiGroup>): List<SymbolCatalog.EmojiGroup> {
        val own = forms.size
        return listOf(SymbolCatalog.EmojiGroup("自有", 0, own)) +
            engine.map { it.copy(start = it.start + own) }
    }
}
