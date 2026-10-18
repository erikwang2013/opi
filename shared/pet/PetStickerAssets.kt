// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.pet

/**
 * 小欧表情**图片贴纸**的文件名契约（Android assets / 桌面 classpath 共用）。
 *
 * 「自有」表情页点形态提交的是图片（不是 Unicode 笑脸）；PNG 真源 =
 * `shared/pet/stickers/`，由 `scripts/gen_pet_stickers.py` 从 [OpiPet] 的同一套形状
 * 代码直出（透明底）。文件名规则与 [PetMood.name] 的对应关系是跨端契约：
 * 小写、下划线换连字符，例如 [PetMood.FACE_PALM] → `face-palm.png`。
 *
 * [PetMood.SLEEPY] / [PetMood.DEGRADED] **没有贴纸** —— 它们是输入状态（不记事 /
 * 引擎降级），不是用户可挑的表情形态（同 [PetEmojiMap] 的排除口径）。本函数对它们
 * 仍是纯字符串映射，但盘上不会有对应文件，调用方不要拿状态枚举当贴纸用。
 */
object PetStickerAssets {

    /** 情绪 → 贴纸文件名（纯函数，Android/桌面共用）。 */
    fun stickerFile(mood: PetMood): String =
        mood.name.lowercase().replace('_', '-') + ".png"

    /**
     * 情绪 → 动画 GIF 文件名。源 = `shared/pet/stickers-gif/`（同一张 PNG 的轻微
     * 弹跳/缩放/摆动，`scripts/gen_pet_stickers.py` 生成）；与 PNG 同基名。
     */
    fun gifFile(mood: PetMood): String =
        mood.name.lowercase().replace('_', '-') + ".gif"
}
