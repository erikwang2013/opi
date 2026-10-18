// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import xyz.erik.opi.engine.ShiftState

/** QWERTY 键盘：3 行字母（第 3 行含 ⇧）+ 底部功能行（中/英 / 123 / 空格 / ⌫ / ↵）。 */
@Composable
fun QwertyKeyboard(
    modifier: Modifier = Modifier,
    onKey: (String) -> Unit,
    onSpace: () -> Unit,
    onBackspace: () -> Unit,
    onEnter: () -> Unit,
    onModeSwitch: () -> Unit,
    onShift: (() -> Unit)? = null,
    onShiftLongPress: (() -> Unit)? = null,
    onNumber: (() -> Unit)? = null,
    shiftState: ShiftState? = null,
    modeLabel: String = "中",
    /** 字母行上方加一行 0-9（英文 26 键；搜索叠盘等其它场景保持关闭）。 */
    showNumberRow: Boolean = false,
) {
    val shiftActive = shiftState != null && shiftState != ShiftState.OFF
    Column(modifier = modifier.fillMaxSize()) {
        for (row in qwertyRows(showNumberRow)) {
            Row(modifier = Modifier.weight(1f).fillMaxSize()) {
                for (ch in row) {
                    if (ch == SHIFT_LABEL) {
                        KeyButton(
                            SHIFT_LABEL,
                            modifier = Modifier.weight(1f),
                            onTap = onShift,
                            onLongPress = onShiftLongPress,
                            highlighted = shiftActive,
                            // SINGLE/LOCK 原本只靠蓝灰底区分：读屏读不到，色觉障碍也分不清
                            stateDescription = when (shiftState) {
                                ShiftState.SINGLE -> "仅下一个字母大写"
                                ShiftState.LOCK -> "大写锁定"
                                else -> null
                            },
                        )
                    } else {
                        // ⇧ 激活（SINGLE/LOCK）时字母键面也大写（2026-10-15 用户报障
                        // 「切换到英文，让英文字母大写的按钮，点击了，键盘还是显示小写，
                        // 实际输出是大写」）。**只改显示**：onKey 送出去的仍是原字符，
                        // 大小写由 shift 语义/引擎决定（KeyRouter.handleKey 的英文直传）。
                        // 读屏名跟着键面走（"Q 键"）—— 与可见字母一致；⇧ 自身的状态
                        // 另由它那枚键的 stateDescription 播报。
                        KeyButton(
                            if (shiftActive) ch.uppercase() else ch,
                            modifier = Modifier.weight(1f),
                            onTap = { onKey(ch) },
                        )
                    }
                }
            }
        }
        // 底行与 9/14/笔画同一套顺序：面板键在左（这里符号经数字面板进入）、
        // [中/英] 其次（2026-10-09 按主流输入法排）。
        Row(modifier = Modifier.weight(1f).fillMaxSize()) {
            // 123 不挂长按：长按 500ms 吞 tap，导致面板打不开（符号经数字面板进入）
            KeyButton("123", modifier = Modifier.weight(1f), onTap = onNumber)
            KeyButton(
                modeLabel,
                modifier = Modifier.weight(1f),
                onTap = onModeSwitch,
                // 键面字是**点它会切到哪**（2026-10-12 晚裁决，见 ime/ImeScreen.kt 的
                // modeKeyLabelOf）—— 读屏按同一语义念成「切到X」，别念成当前模式。
                stateDescription = when (modeLabel) {
                    "中" -> "切到中文"
                    "繁" -> "切到繁体"
                    "英" -> "切到英文"
                    else -> null
                },
            )
            KeyButton("空格", modifier = Modifier.weight(3f), onTap = onSpace)
            KeyButton("⌫", modifier = Modifier.weight(1f), onTap = onBackspace)
            KeyButton("↵", modifier = Modifier.weight(1f), onTap = onEnter)
        }
    }
}

private val rows = listOf(
    listOf("q", "w", "e", "r", "t", "y", "u", "i", "o", "p"),
    listOf("a", "s", "d", "f", "g", "h", "j", "k", "l"),
    listOf("⇧", "z", "x", "c", "v", "b", "n", "m"),
)
private val numberRow = listOf("1", "2", "3", "4", "5", "6", "7", "8", "9", "0")

/** 实际渲染的键位行（数字行在字母行上方）。JVM 单测直接核这张表。 */
internal fun qwertyRows(showNumberRow: Boolean): List<List<String>> =
    if (showNumberRow) listOf(numberRow) + rows else rows

private const val SHIFT_LABEL = "⇧"
