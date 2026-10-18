// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import xyz.erik.opi.pet.PetEmojiMap
import xyz.erik.opi.pet.PetMood
import xyz.erik.opi.pet.PetStickerAssets

/**
 * 「自有」表情 = 小欧形象（12 个宠物形态）。
 *
 * **2026-10-18 用户裁决改判**：点自有形态**插入 PNG 图片**，不再提交 Unicode emoji
 * —— 上屏内容由 `OpiImeService.commitPetSticker` 走图片通道（文本通道 commitWithRetry
 * 不参与）。编辑器不支持图片时回退**独立文本标签** `[小欧·笑]`（仍不是 emoji）。
 * 跨端 emoji 映射 [PetEmojiMap] 仍留给其他端，本端只取它的情绪/标签。
 *
 * 「自有」是**独立顶级子页**（见 [EmojiSubTab]），不再混进 Emoji 分类条与最近使用。
 *
 * [PetMood.SLEEPY] / [PetMood.DEGRADED] 是输入状态（不记事 / 引擎降级），不是表情，
 * 共享映射有意不含它们。
 */
object PetEmoji {

    val forms: List<PetEmojiMap.Form> get() = PetEmojiMap.forms

    /** 中文标签（图片插不进去时上屏 `[小欧·笑]` 用）。 */
    fun label(mood: PetMood): String = forms.firstOrNull { it.mood == mood }?.label ?: "表情"

    /**
     * APK assets 内的路径。文件名契约只留一份：共享的 [PetStickerAssets.stickerFile]
     * （桌面 classpath 与 Android assets 同名同规则）；这里只补 assets 的 `pet/` 前缀。
     */
    fun assetPath(mood: PetMood): String = "pet/" + PetStickerAssets.stickerFile(mood)
}
