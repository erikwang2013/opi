// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.pet

import android.content.res.AssetManager
import android.util.Log

/**
 * APK `assets/pet-font/` 的读取口（构建期由 `copyPetStickers` 从
 * `shared/pet/emoji-font/` 拷入）。
 *
 * 解析本身在共享的 [PetEmojiTable]（纯 JVM）；这里只负责 AssetManager 的 IO。
 * 面板用 `map.tsv` 拿标签与短码；贴纸本身走 `assets/pet/`、`assets/pet-gif/`。
 */
object PetEmojiTables {

    private const val TAG = "OpiPetTables"

    private fun path(name: String): String = PetEmojiTable.DIR + "/" + name

    /** `map.tsv` → 12 条；缺资产/坏数据返回 null（不崩输入法）。 */
    fun map(assets: AssetManager): List<PetEmojiTable.Glyph>? = runCatching {
        PetEmojiTable.parseMap(
            assets.open(path(PetEmojiTable.MAP_FILE)).bufferedReader().use { it.readText() },
        )
    }.onFailure { Log.w(TAG, "pet-font/map.tsv 读取或解析失败", it) }.getOrNull()

    /** `shortcodes.tsv` → 短码表；缺资产/坏数据返回 null。 */
    fun shortcodes(assets: AssetManager): List<PetEmojiTable.Shortcode>? = runCatching {
        PetEmojiTable.parseShortcodes(
            assets.open(path(PetEmojiTable.SHORTCODES_FILE)).bufferedReader().use { it.readText() },
        )
    }.onFailure { Log.w(TAG, "pet-font/shortcodes.tsv 读取或解析失败", it) }.getOrNull()
}
