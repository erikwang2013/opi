// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier

/**
 * 14 键布局表。
 *
 * ⚠️ **协议级约束（与 9 键同款）**：发送的键值（[FourteenKey.key]）必须等于
 * engine-core `scheme.rs` 里 `Scheme::Fourteen.key_class` 的**规范符号**（两字母键
 * 取字母对的**首个字母**：`qw→q`），而 label 的两个字母必须**恰好**是那一对
 * （或单字母键的单个字母）。`FourteenKeyLayoutTest` 与 `scheme.rs` 的类表逐格交叉
 * 核对；改这张表 = 改协议，四端一起改。
 *
 * 布局（对齐搜狗「拼音14键」的常见摆法）：`qw er ty ui op / as df gh jk l / zx cv bn m`，
 * `l` 与 `m` 为单字母键。
 */
internal data class FourteenKey(val label: String, val key: String)

internal val fourteenKeyRows: List<List<FourteenKey>> = listOf(
    listOf(
        FourteenKey("qw", "q"),
        FourteenKey("er", "e"),
        FourteenKey("ty", "t"),
        FourteenKey("ui", "u"),
        FourteenKey("op", "o"),
    ),
    listOf(
        FourteenKey("as", "a"),
        FourteenKey("df", "d"),
        FourteenKey("gh", "g"),
        FourteenKey("jk", "j"),
        FourteenKey("l", "l"),
    ),
    listOf(
        FourteenKey("zx", "z"),
        FourteenKey("cv", "c"),
        FourteenKey("bn", "b"),
        FourteenKey("m", "m"),
    ),
)

/** 拼音 14 键：两行 5 键 + 一行 4 键 + 功能行（与 9 键同一套功能键语义）。 */
@Composable
fun FourteenKeyKeyboard(
    modifier: Modifier = Modifier,
    onKey: (String) -> Unit,
    onSpace: () -> Unit,
    onBackspace: () -> Unit,
    onEnter: () -> Unit,
    onModeSwitch: () -> Unit,
    onSymbol: () -> Unit,
    modeLabel: String = "中",
) {
    Column(modifier = modifier.fillMaxSize()) {
        for (row in fourteenKeyRows) {
            Row(modifier = Modifier.weight(1f).fillMaxSize()) {
                for (k in row) {
                    KeyBox(
                        modifier = Modifier.weight(1f),
                        description = "字母 ${k.label.toCharArray().joinToString(" ")}",
                        onTap = { onKey(k.key) },
                    ) {
                        Text(k.label)
                    }
                }
            }
        }
        // 底行：[符] 在左、[中/英] 其次（与 9 键同序，2026-10-09）。
        Row(modifier = Modifier.weight(1f).fillMaxSize()) {
            KeyButton("符", modifier = Modifier.weight(1f), onTap = onSymbol)
            KeyButton(
                modeLabel,
                modifier = Modifier.weight(1f),
                onTap = onModeSwitch,
                stateDescription = modeStateDescription(modeLabel),
            )
            KeyButton("空格", modifier = Modifier.weight(2f), onTap = onSpace)
            KeyButton("⌫", modifier = Modifier.weight(1f), onTap = onBackspace)
            KeyButton("↵", modifier = Modifier.weight(1f), onTap = onEnter)
        }
    }
}
