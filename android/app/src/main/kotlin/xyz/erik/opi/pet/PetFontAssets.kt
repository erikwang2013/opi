// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.pet

import android.content.res.AssetManager
import android.graphics.Typeface
import android.util.Log

/**
 * APK `assets/pet-font/` 的读取口（构建期由 `copyPetStickers` 从
 * `shared/pet/emoji-font/` 拷入）。
 *
 * 解析本身在共享的 [PetEmojiFont]（纯 JVM）；这里只负责 AssetManager 的 IO 与
 * `Typeface` 加载。字体只有 PUA 码位 U+E000–U+E00B，且色表是 SVG + CBDT；
 * Android 对 CBDT 有支持，但面板贴纸**不依赖**它（走 PNG/GIF），字体留给需要
 * 渲染 PUA 短码/字形的调用方。
 */
object PetFontAssets {

    private const val TAG = "OpiPetFont"

    /** assets 内的字体/映射目录。 */
    private fun path(name: String): String = PetEmojiFont.DIR + "/" + name

    /** 已成功预载的 map 行；首次打开自有贴纸页时把三份资产都读一遍。 */
    private var preloaded: List<PetEmojiFont.Glyph>? = null

    /**
     * 预载 `pet-font/` 三件套（map、shortcodes、TTF）并返回 map 行。面板用它拿
     * 标签，同时让内置字体真的被 `Typeface.createFromAsset` 读入；任何一份缺失
     * 都只记日志（面板退回 `PetEmojiMap` / 矢量占位）。
     */
    fun preload(assets: AssetManager): List<PetEmojiFont.Glyph>? {
        preloaded?.let { return it }
        val glyphs = map(assets)
        shortcodes(assets)
        typeface(assets)
        preloaded = glyphs
        Log.i(TAG, "pet-font 预载：${glyphs?.size ?: 0} 条映射")
        return glyphs
    }

    /** `map.tsv` → 12 条；缺资产/坏数据返回 null（不崩输入法）。 */
    fun map(assets: AssetManager): List<PetEmojiFont.Glyph>? = runCatching {
        PetEmojiFont.parseMap(assets.open(path(PetEmojiFont.MAP_FILE)).bufferedReader().use { it.readText() })
    }.onFailure { Log.w(TAG, "pet-font/map.tsv 读取或解析失败", it) }.getOrNull()

    /** `shortcodes.tsv` → 短码表；缺资产/坏数据返回 null。 */
    fun shortcodes(assets: AssetManager): List<PetEmojiFont.Shortcode>? = runCatching {
        PetEmojiFont.parseShortcodes(
            assets.open(path(PetEmojiFont.SHORTCODES_FILE)).bufferedReader().use { it.readText() },
        )
    }.onFailure { Log.w(TAG, "pet-font/shortcodes.tsv 读取或解析失败", it) }.getOrNull()

    /** 加载 `opi-emoji.ttf`（family "Opi Emoji"）；缺资产/坏字体返回 null。 */
    fun typeface(assets: AssetManager): Typeface? = runCatching {
        Typeface.createFromAsset(assets, path(PetEmojiFont.FONT_FILE))
    }.onFailure { Log.w(TAG, "pet-font/opi-emoji.ttf 加载失败", it) }.getOrNull()
}
