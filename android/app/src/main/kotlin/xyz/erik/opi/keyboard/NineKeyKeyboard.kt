// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * 9 键布局表（拼音 9 键 / 大 9 键共用；大 9 只差字号）。
 *
 * ⚠️ **发送的键值（[NineKey.key]）= 协议**：必须落在 engine-core `scheme.rs` 的
 * Nine 类表里（`'2'..'9'` 各映射三四个字母）。hint 是键面下的字母提示，且**必须
 * 正是该数字在 `scheme.rs` 里带的类** —— `NineKeyLayoutTest` 按类表交叉核对，
 * 两边任何一处改了都会红。
 *
 * `，` 键（发送 `,`）沿用引擎的标点表：中文模式下由标点层出「，」，与 26 键盘的
 * `,` 是**同一条路**（不是 UI 直提）。
 */
internal data class NineKey(val label: String, val key: String, val hint: String = "")

internal val nineKeyRows: List<List<NineKey>> = listOf(
    listOf(NineKey("，", ","), NineKey("2", "2", "abc"), NineKey("3", "3", "def")),
    listOf(NineKey("4", "4", "ghi"), NineKey("5", "5", "jkl"), NineKey("6", "6", "mno")),
    listOf(NineKey("7", "7", "pqrs"), NineKey("8", "8", "tuv"), NineKey("9", "9", "wxyz")),
)

/**
 * 拼音 9 键（T9）。[big] = 大 9 键（同布局放大键面，对齐搜狗「拼音大9键」）。
 * 数字键在此方案下**是输入键**（引擎 Nine 方案收进缓冲），不需要任何数字选词 UI。
 */
@Composable
fun NineKeyKeyboard(
    modifier: Modifier = Modifier,
    onKey: (String) -> Unit,
    onSpace: () -> Unit,
    onBackspace: () -> Unit,
    onEnter: () -> Unit,
    onModeSwitch: () -> Unit,
    onSymbol: () -> Unit,
    modeLabel: String = "中",
    big: Boolean = false,
) {
    val numSize = if (big) 30.sp else 22.sp
    val hintSize = if (big) 14.sp else 11.sp
    Column(modifier = modifier.fillMaxSize()) {
        for (row in nineKeyRows) {
            Row(modifier = Modifier.weight(1f).fillMaxSize()) {
                for (k in row) {
                    KeyBox(
                        modifier = Modifier.weight(1f),
                        description = nineKeySpoken(k),
                        onTap = { onKey(k.key) },
                    ) {
                        Column(
                            horizontalAlignment = Alignment.CenterHorizontally,
                            verticalArrangement = Arrangement.Center,
                        ) {
                            Text(k.label, fontSize = numSize)
                            if (k.hint.isNotEmpty()) {
                                Text(
                                    k.hint,
                                    fontSize = hintSize,
                                    color = LocalKeyboardPalette.current.barDimFg,
                                )
                            }
                        }
                    }
                }
            }
        }
        Row(modifier = Modifier.weight(1f).fillMaxSize()) {
            KeyButton(
                modeLabel,
                modifier = Modifier.weight(1f),
                onTap = onModeSwitch,
                stateDescription = modeStateDescription(modeLabel),
            )
            KeyButton("符", modifier = Modifier.weight(1f), onTap = onSymbol)
            KeyButton("空格", modifier = Modifier.weight(2f), onTap = onSpace)
            KeyButton("⌫", modifier = Modifier.weight(1f), onTap = onBackspace)
            KeyButton("↵", modifier = Modifier.weight(1f), onTap = onEnter)
        }
    }
}

/** 9 键键位的读屏名：「数字 2，字母 a b c」—— 只念数字会丢掉「这键打什么」。 */
internal fun nineKeySpoken(k: NineKey): String = when {
    k.hint.isNotEmpty() -> "数字 ${k.label}，字母 ${k.hint.toCharArray().joinToString(" ")}"
    k.label == "，" -> "逗号"
    else -> spokenKeyName(k.label)
}

/**
 * 键位容器（9 键 / 14 键 / 笔画共用外观）：圆角底 + 点击 + 读屏名。
 * 与 [KeyButton] 同观感，区别只在内容不是单行文本（数字 + 字母提示两行）。
 */
@Composable
internal fun KeyBox(
    modifier: Modifier = Modifier,
    description: String,
    onTap: () -> Unit,
    content: @Composable () -> Unit,
) {
    Box(
        modifier = modifier
            .padding(1.dp)
            .clip(RoundedCornerShape(6.dp))
            .background(LocalKeyboardPalette.current.keyBg)
            .clickable(onClick = onTap)
            .semantics {
                role = Role.Button
                contentDescription = description
            }
            .fillMaxSize(),
        contentAlignment = Alignment.Center,
    ) { content() }
}
