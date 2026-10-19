// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.pet

/**
 * 「自有」表情映射：宠物形态 → 面板标签 + 生成器用的 Unicode emoji。
 *
 * **两端共用这一份**（Android 的 `keyboard/PetEmoji.kt` 与桌面的表情页）。标签带
 * 「小欧·」前缀（2026-10-19 用户裁决）：自有表情的读屏、搜索与面板文字不能和内置
 * emoji 重名；图片通道失败时上屏的也是这个标签（`[小欧·笑]`），**不再回退 emoji**。
 * [Form.emoji] 只为 `scripts/gen_pet_emoji_font.py` 的三元组正则保留，插入路径不用它。
 *
 * 枚举顺序 = [PetMood.entries] 的枚举序（跨端契约，新形态只能加在末尾）；
 * [PetMood.SLEEPY] / [PetMood.DEGRADED] **有意不映射** —— 它们是输入状态
 * （不记事 / 引擎降级），不是用户可挑的表情形态。
 */
object PetEmojiMap {

    /** 一个可点的宠物形态：情绪、面板标签（带「小欧·」前缀）、生成器用的 emoji。 */
    data class Form(val mood: PetMood, val label: String, val emoji: String)

    private val byMood: Map<PetMood, Pair<String, String>> = mapOf(
        PetMood.IDLE to ("小欧·笑" to "😊"),
        PetMood.WAITING to ("小欧·欢乐" to "😄"),
        PetMood.PUZZLED to ("小欧·沮丧" to "😞"),
        PetMood.CRYING to ("小欧·哭" to "😭"),
        PetMood.TERRIFIED to ("小欧·惊恐" to "😱"),
        PetMood.ANGRY to ("小欧·愤怒" to "😠"),
        PetMood.COQUETTISH to ("小欧·撒娇" to "🥺"),
        PetMood.CUTE to ("小欧·卖萌" to "😜"),
        PetMood.PLEASING to ("小欧·讨好" to "🙏"),
        PetMood.KNOCKING to ("小欧·敲打" to "🔨"),
        PetMood.FACE_PALM to ("小欧·捂脸" to "🤦"),
        PetMood.TONGUE_OUT to ("小欧·吐舌头" to "😛"),
    )

    /** 网格里的形态列表（枚举序）；没有映射的状态不出现在这里。 */
    val forms: List<Form>
        get() = PetMood.entries.mapNotNull { m ->
            byMood[m]?.let { (label, emoji) -> Form(m, label, emoji) }
        }

    /** 按 [PetMood.name] 索引的同一份形态（只有名字、拿不到枚举的调用方用）。 */
    val formsByName: Map<String, Form>
        get() = forms.associateBy { it.mood.name }

    fun emojiFor(mood: PetMood): String? = byMood[mood]?.second

    fun emojiFor(name: String): String? = formsByName[name]?.emoji
}
