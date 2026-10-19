// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.pet

/**
 * 桌面 classpath 上的 `pet-font/` 资源（构建期由 `copyPetStickers` 从
 * `shared/pet/emoji-font/` 拷入）。与 Android 的 [PetEmojiTables] 同源。
 *
 * 目录名是历史遗留，里面只剩 `map.tsv`/`shortcodes.tsv`（标签、短码、稳定码位 ID），
 * 没有字体文件。只做 IO；解析在共享的 [PetEmojiTable]，测试覆盖真实 classpath 接线。
 */
object PetEmojiResources {

    private fun text(name: String): String? =
        PetEmojiResources::class.java.getResourceAsStream("/${PetEmojiTable.DIR}/$name")
            ?.bufferedReader()?.use { it.readText() }

    /** `map.tsv` → 12 条；资源缺失/解析失败返回 null。 */
    fun map(): List<PetEmojiTable.Glyph>? = text(PetEmojiTable.MAP_FILE)?.let {
        runCatching { PetEmojiTable.parseMap(it) }.getOrNull()
    }

    /** `shortcodes.tsv` → 短码表；资源缺失/解析失败返回 null。 */
    fun shortcodes(): List<PetEmojiTable.Shortcode>? = text(PetEmojiTable.SHORTCODES_FILE)?.let {
        runCatching { PetEmojiTable.parseShortcodes(it) }.getOrNull()
    }
}
