// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.sp

/**
 * 数字面板：Gboard 风格 5 行（对齐 flutter number_pad.dart）。
 *
 * 键**走引擎**（`onKey` = KeyRouter.handleKey）：数字进引擎缓冲（引擎 Number 模式），
 * `,` `.` 经引擎层标点表出文本（默认半角原样交回，全角开关打开后出 `，` `．`）；
 * 面板打开期间引擎被切到 Number 模式，离开时恢复字母模式（见 ImeState.openNumber）。
 * 这里只出键码 —— 提交/上屏在路由与引擎，四端同一条路。
 */
@Composable
fun NumberPad(
    modifier: Modifier = Modifier,
    onKey: (String) -> Unit,
    onSymbol: () -> Unit,
    onLetters: () -> Unit,
    onSpace: () -> Unit,
    onBackspace: () -> Unit,
    onEnter: () -> Unit,
) {
    BoxWithConstraints(modifier = modifier.fillMaxSize()) {
        // 横屏矮键区降一档，见 [compactKeys]。
        val compact = compactKeys(maxWidth.value, maxHeight.value)
        val keyFont = if (compact) 15.sp else 21.sp
        // 多字键单独一档：底行 5 个单位，320dp 屏每格只有 35.6dp（减 padding 33.6），
        // 「?123」在 21sp 约 44dp、16sp 约 35dp —— 16sp 在 320dp 上仍差 2dp 被省略，
        // 15sp 才稳定放得下；这两个键是次要标签，小一号不伤主键。
        val labelFont = if (compact) 14.sp else 15.sp
        Column(modifier = Modifier.fillMaxSize()) {
            for (row in numberRows) {
                Row(modifier = Modifier.weight(1f).fillMaxSize()) {
                    for (ch in row) {
                        KeyButton(
                            ch,
                            modifier = Modifier.weight(1f),
                            onTap = { onKey(ch) },
                            fontSize = keyFont,
                        )
                    }
                }
            }
            Row(modifier = Modifier.weight(1f).fillMaxSize()) {
                KeyButton("ABC", modifier = Modifier.weight(1f), onTap = onLetters, fontSize = labelFont)
                KeyButton("?123", modifier = Modifier.weight(1f), onTap = onSymbol, fontSize = labelFont)
                KeyButton("空格", modifier = Modifier.weight(5f), onTap = onSpace, fontSize = keyFont)
                KeyButton(
                    "⌫",
                    modifier = Modifier.weight(1f),
                    onTap = onBackspace,
                    // 按住连续删除；短按仍只删一个（2026-10-23 用户裁决）。
                    repeatOnLongPress = true,
                    fontSize = keyFont,
                )
                KeyButton("↵", modifier = Modifier.weight(1f), onTap = onEnter, fontSize = keyFont)
            }
        }
    }
}

/** Gboard 风格：1-9 三行 + 标点行（', 0 .'）。 */
private val numberRows = listOf(
    listOf("1", "2", "3"),
    listOf("4", "5", "6"),
    listOf("7", "8", "9"),
    listOf(",", "0", "."),
)
