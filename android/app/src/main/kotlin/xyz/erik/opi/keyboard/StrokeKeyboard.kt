// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.sp

/**
 * 笔画键盘（五笔画）。发送的键值是 `h/s/p/n/z`（横竖撇捺折）——**普通字母**，
 * 与 engine-core `scheme.rs` 的 `Scheme::Stroke` 约定一致（那一路换的是**词典**：
 * `stroke.opid` 的键就是笔画串，词是单字；匹配走普通前缀）。键面显示笔画字形，
 * 提示行给出「横竖撇捺折」全字。
 *
 * ⚠️ 笔画串的顺序 = 书写顺序（首笔在前）；`hs` = 横起首笔、竖第二笔。没有词典
 * （stroke.opid 没装/坏了）时候选为空（引擎侧可选增强语义），键盘照常可打。
 */
internal data class StrokeKey(val label: String, val key: String, val name: String)

internal val strokeKeys: List<StrokeKey> = listOf(
    StrokeKey("一", "h", "横"),
    StrokeKey("丨", "s", "竖"),
    StrokeKey("丿", "p", "撇"),
    StrokeKey("丶", "n", "捺"),
    StrokeKey("乙", "z", "折"),
)

/** 笔画键盘：一行五个笔画键 + 功能行（含「生僻字」入口 —— 找不到字时下一步就是它）。 */
@Composable
fun StrokeKeyboard(
    modifier: Modifier = Modifier,
    onKey: (String) -> Unit,
    onSpace: () -> Unit,
    onBackspace: () -> Unit,
    onEnter: () -> Unit,
    onModeSwitch: () -> Unit,
    onSymbol: () -> Unit,
    onRare: () -> Unit,
    onEnglish: () -> Unit,
    modeLabel: String = "中",
) {
    Column(modifier = modifier.fillMaxSize()) {
        Row(modifier = Modifier.weight(1f).fillMaxSize()) {
            for (k in strokeKeys) {
                KeyBox(
                    modifier = Modifier.weight(1f),
                    description = "${k.name}（${
                        k.key.toCharArray().joinToString(" ")
                    } 键）",
                    onTap = { onKey(k.key) },
                ) {
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.Center,
                    ) {
                        Text(k.label, fontSize = 26.sp)
                        Text(
                            k.name,
                            fontSize = 12.sp,
                            color = LocalKeyboardPalette.current.barDimFg,
                        )
                    }
                }
            }
        }
        Row(modifier = Modifier.weight(1f).fillMaxSize()) {
            KeyButton("符", modifier = Modifier.weight(1f), onTap = onSymbol)
            // 模式键只切简/繁（2026-10-15 用户裁决「统一成只切简繁」，与 9/14 键同）——
            // 进英文靠右边那枚独立的 [英文] 键（缺了它英文在本盘就不可达）。
            KeyButton(
                modeLabel,
                modifier = Modifier.weight(1f),
                onTap = onModeSwitch,
                description = "简繁切换",
                stateDescription = modeStateDescription(modeLabel),
            )
            // 底行 7 键在 320dp 上每单位只有 38dp：「英文」（2 字 × 21sp ≈ 42dp）与
            // 「生僻字」（3 字 × 21sp ≈ 63dp，格 53dp）都会截字，故这两个键单独降字号。
            KeyButton("英文", modifier = Modifier.weight(1f), onTap = onEnglish, fontSize = 15.sp)
            KeyButton("生僻字", modifier = Modifier.weight(1.4f), onTap = onRare, fontSize = 14.sp)
            KeyButton("空格", modifier = Modifier.weight(2f), onTap = onSpace, mic = true)
            KeyButton(
                "⌫",
                modifier = Modifier.weight(1f),
                onTap = onBackspace,
                // 按住连续删除；短按仍只删一个（2026-10-23 用户裁决）。
                repeatOnLongPress = true,
            )
            KeyButton("↵", modifier = Modifier.weight(1f), onTap = onEnter)
        }
    }
}
