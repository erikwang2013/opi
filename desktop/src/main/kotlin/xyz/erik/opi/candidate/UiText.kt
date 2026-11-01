// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 设置/关于页的中英语言层（2026-11-02）。只翻译这两类页面，键面与候选保持原样。
//! 翻译表与 Android 共用 `shared/i18n/zh-en.tsv`；精确匹配 → 最长前缀匹配。

package xyz.erik.opi.candidate

import androidx.compose.foundation.layout.Row
import androidx.compose.material.LocalTextStyle
import androidx.compose.material.Text as MaterialText
import androidx.compose.material.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.sp

internal object UiLang {
    var english by mutableStateOf(false)
        private set

    private var language = ""
    private var exact: Map<String, String> = emptyMap()
    private var prefixes: List<Pair<String, String>> = emptyList()

    fun init(language: String) {
        if (this.language == language && (language == "zh" || exact.isNotEmpty())) {
            english = language == "en"
            return
        }
        this.language = language
        english = language == "en"
        if (exact.isEmpty()) {
            exact = runCatching {
                UiLang::class.java.getResourceAsStream("/zh-en.tsv")
                    ?.bufferedReader(Charsets.UTF_8)?.useLines { lines ->
                        lines.mapNotNull { line ->
                            if (line.isBlank() || line.startsWith("#")) return@mapNotNull null
                            val tab = line.indexOf('\t')
                            if (tab <= 0) return@mapNotNull null
                            line.substring(0, tab) to line.substring(tab + 1)
                        }.toMap()
                    } ?: emptyMap()
            }.getOrDefault(emptyMap())
            prefixes = exact.entries
                .asSequence()
                .filter { it.key.length >= 2 }
                .sortedByDescending { it.key.length }
                .map { it.key to it.value }
                .toList()
        }
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

/** 与 Material Text 同签名的本地化包装；设置/关于页替换 import 即可。 */
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
    onTextLayout: ((TextLayoutResult) -> Unit)? = null,
    style: TextStyle = LocalTextStyle.current,
) {
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
        onTextLayout = onTextLayout,
        style = style,
    )
}

/** 设置窗、关于页顶部的中英文切换。 */
@Composable
internal fun LanguageRow(language: String, onChange: (String) -> Unit) {
    Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
        Text("界面语言", fontSize = 12.sp, modifier = Modifier.weight(1f))
        TextButton(onClick = { onChange("zh") }) {
            Text("中文", fontWeight = if (language != "en") FontWeight.Bold else null)
        }
        TextButton(onClick = { onChange("en") }) {
            Text("English", fontWeight = if (language == "en") FontWeight.Bold else null)
        }
    }
}
