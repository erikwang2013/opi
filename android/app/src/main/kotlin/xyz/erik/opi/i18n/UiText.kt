// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.i18n

import android.content.Context
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.ListItem
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.Text as MaterialText
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.TextUnit

/**
 * 设置/关于页的语言层（2026-11-02）。**只翻译这两类页面**，键面与候选保持输入法语言。
 *
 * 翻译表来自 `shared/i18n/zh-en.tsv`，按「精确匹配 → 最长前缀匹配」查表：前缀匹配
 * 让「已下载 · 24 MB」这类带动态数字的文案也能得到英文前缀，数字原样保留。
 * 查不到就保留原文 —— 宁可半句中文，也不显示空串或猜错。
 */
internal object UiLang {
    private const val PREFS = "opi_ui"
    private const val KEY = "ui_language"

    var english by mutableStateOf(false)
        private set

    private var loaded = false
    private var exact: Map<String, String> = emptyMap()
    private var prefixes: List<Pair<String, String>> = emptyList()

    fun ensure(context: Context) {
        if (loaded) return
        loaded = true
        val app = context.applicationContext
        english = app.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .getString(KEY, "zh") == "en"
        exact = runCatching {
            app.assets.open("zh-en.tsv").bufferedReader(Charsets.UTF_8).useLines { lines ->
                lines.mapNotNull { line ->
                    if (line.isBlank() || line.startsWith("#")) return@mapNotNull null
                    val tab = line.indexOf('\t')
                    if (tab <= 0) return@mapNotNull null
                    line.substring(0, tab) to line.substring(tab + 1)
                }.toMap()
            }
        }.getOrDefault(emptyMap())
        prefixes = exact.entries
            .asSequence()
            .filter { it.key.length >= 2 }
            .sortedByDescending { it.key.length }
            .map { it.key to it.value }
            .toList()
    }

    fun setEnglish(context: Context, on: Boolean) {
        english = on
        context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit().putString(KEY, if (on) "en" else "zh").apply()
    }

    fun t(text: String): String {
        if (!english || text.isEmpty()) return text
        exact[text]?.let { return it }
        for ((zh, en) in prefixes) {
            if (text.startsWith(zh)) return en + text.substring(zh.length)
        }
        return text
    }
}

/** 设置/关于页顶部的中英文切换（2026-11-02）。 */
@Composable
internal fun LanguageSwitcher() {
    val context = LocalContext.current
    UiLang.ensure(context)
    ListItem(
        headlineContent = { Text("界面语言") },
        trailingContent = {
            Row {
                TextButton(onClick = { UiLang.setEnglish(context, false) }) {
                    Text("中文", fontWeight = if (!UiLang.english) FontWeight.Bold else null)
                }
                TextButton(onClick = { UiLang.setEnglish(context, true) }) {
                    Text("English", fontWeight = if (UiLang.english) FontWeight.Bold else null)
                }
            }
        },
    )
}

/** 与 Material3 Text 同签名的本地化包装；设置/关于页替换 import 即可。 */
@Composable
internal fun Text(
    text: String,
    modifier: Modifier = Modifier,
    color: Color = Color.Unspecified,
    fontSize: TextUnit = TextUnit.Unspecified,
    fontStyle: FontStyle? = null,
    fontWeight: FontWeight? = null,
    fontFamily: FontFamily? = null,
    letterSpacing: TextUnit = TextUnit.Unspecified,
    textDecoration: TextDecoration? = null,
    textAlign: TextAlign? = null,
    lineHeight: TextUnit = TextUnit.Unspecified,
    overflow: TextOverflow = TextOverflow.Clip,
    softWrap: Boolean = true,
    maxLines: Int = Int.MAX_VALUE,
    minLines: Int = 1,
    onTextLayout: ((TextLayoutResult) -> Unit)? = null,
    style: TextStyle = LocalTextStyle.current,
) {
    UiLang.ensure(LocalContext.current)
    MaterialText(
        text = UiLang.t(text),
        modifier = modifier,
        color = color,
        fontSize = fontSize,
        fontStyle = fontStyle,
        fontWeight = fontWeight,
        fontFamily = fontFamily,
        letterSpacing = letterSpacing,
        textDecoration = textDecoration,
        textAlign = textAlign,
        lineHeight = lineHeight,
        overflow = overflow,
        softWrap = softWrap,
        maxLines = maxLines,
        minLines = minLines,
        onTextLayout = onTextLayout,
        style = style,
    )
}
