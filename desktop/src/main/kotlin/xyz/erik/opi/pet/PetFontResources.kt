// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.pet

/**
 * 桌面 classpath 上的 `pet-font/` 资源（构建期由 `copyPetStickers` 从
 * `shared/pet/emoji-font/` 拷入）。与 Android 的 [PetFontAssets] 同源。
 *
 * 只做 IO；解析在共享的 [PetEmojiFont]，测试覆盖真实 classpath 接线。
 */
object PetFontResources {

    private fun text(name: String): String? =
        PetFontResources::class.java.getResourceAsStream("/${PetEmojiFont.DIR}/$name")
            ?.bufferedReader()?.use { it.readText() }

    /** `map.tsv` → 12 条；资源缺失/解析失败返回 null。 */
    fun map(): List<PetEmojiFont.Glyph>? = text(PetEmojiFont.MAP_FILE)?.let {
        runCatching { PetEmojiFont.parseMap(it) }.getOrNull()
    }

    /** `shortcodes.tsv` → 短码表；资源缺失/解析失败返回 null。 */
    fun shortcodes(): List<PetEmojiFont.Shortcode>? = text(PetEmojiFont.SHORTCODES_FILE)?.let {
        runCatching { PetEmojiFont.parseShortcodes(it) }.getOrNull()
    }

    /** `opi-emoji.ttf` 原始字节；资源缺失返回 null（桌面不主动注册系统字体）。 */
    fun fontBytes(): ByteArray? =
        PetFontResources::class.java.getResourceAsStream("/${PetEmojiFont.DIR}/${PetEmojiFont.FONT_FILE}")
            ?.use { it.readBytes() }
}
