// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi

import android.content.res.AssetManager
import android.util.Log
import xyz.erik.opi.keyboard.KaomojiCategories
import xyz.erik.opi.keyboard.SymbolCategories

/**
 * IME 的资产类目读取（2026-10-18 从 `OpiImeService` 拆出 —— 表情页加颜文字后那里
 * 越过 500 行门禁；同 `EditorInfoRules` 的拆分先例）。
 *
 * 两条都是懒读的普通列表：资产缺失/损坏 → 空列表（页面显示空态），不崩 IME。
 */
private const val TAG = "ImeAssetData"

internal fun loadSymbolCategories(assets: AssetManager): List<SymbolCategories.Category> =
    try {
        assets.open("symbol_categories.tsv").bufferedReader()
            .use { SymbolCategories.parse(it.readText()) }
    } catch (e: Exception) {
        Log.w(TAG, "symbol_categories.tsv 读取失败，符号页将为空: $e")
        emptyList()
    }

internal fun loadKaomojiCategories(assets: AssetManager): List<KaomojiCategories.Category> =
    try {
        assets.open("kaomoji.tsv").bufferedReader()
            .use { KaomojiCategories.parse(it.readText()) }
    } catch (e: Exception) {
        Log.w(TAG, "kaomoji.tsv 读取失败，颜文字页将为空: $e")
        emptyList()
    }
