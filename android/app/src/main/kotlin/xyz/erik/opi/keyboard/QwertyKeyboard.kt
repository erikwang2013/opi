// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.sp
import xyz.erik.opi.engine.ShiftState

/** QWERTY 键盘：3 行字母（第 3 行含 ⇧；英文另有 ↵）+ 底部功能行。 */
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
    onSymbol: (() -> Unit)? = null,
    shiftState: ShiftState? = null,
    modeLabel: String = "中",
    /** 字母行上方加一行 0-9（英文 26 键；搜索叠盘等其它场景保持关闭）。 */
    showNumberRow: Boolean = false,
    /** 英文 26 键布局：底行 `!?#` + `pro`，↵ 放到第 3 行最右，中英键在空格后。 */
    showProKey: Boolean = false,
    /** `pro` 是否处于专业模式（高亮）；只在 [showProKey] 时有效。 */
    proActive: Boolean = false,
    onProToggle: (() -> Unit)? = null,
) {
    val shiftActive = shiftState != null && shiftState != ShiftState.OFF
    BoxWithConstraints(modifier = modifier.fillMaxSize()) {
        // 横屏矮键区 / 单手窄键区降一档字号，见 [compactKeys]。
        // 英文字号更大（2026-10-19 用户反馈数字/字母键偏小）；数字行压矮、字母行加高，
        // 把多出的一行高度还给字母，而不是让 5 行均分。
        val keyFont = when {
            compactKeys(maxWidth.value, maxHeight.value) -> 16.sp
            showProKey -> 23.sp
            else -> 21.sp
        }
        val rowsToDraw = qwertyRows(showNumberRow, showProKey)
        Column(modifier = Modifier.fillMaxSize()) {
            rowsToDraw.forEachIndexed { index, row ->
                val rowWeight = when {
                    showProKey && showNumberRow && index == 0 -> 0.7f
                    showProKey -> 1.05f
                    else -> 1f
                }
                Row(modifier = Modifier.weight(rowWeight).fillMaxSize()) {
                    for (ch in row) {
                        when (ch) {
                            SHIFT_LABEL -> KeyButton(
                                SHIFT_LABEL,
                                modifier = Modifier.weight(1f),
                                onTap = onShift,
                                onLongPress = onShiftLongPress,
                                highlighted = shiftActive,
                                fontSize = keyFont,
                                // SINGLE/LOCK 原本只靠蓝灰底区分：读屏读不到，色觉障碍也分不清
                                stateDescription = when (shiftState) {
                                    ShiftState.SINGLE -> "仅下一个字母大写"
                                    ShiftState.LOCK -> "大写锁定"
                                    else -> null
                                },
                            )
                            ENTER_LABEL -> KeyButton(
                                ENTER_LABEL,
                                modifier = Modifier.weight(1f),
                                onTap = onEnter,
                                fontSize = keyFont,
                            )
                            else -> {
                                // ⇧ 激活（SINGLE/LOCK）时字母键面也大写（2026-10-15 用户报障
                                // 「切换到英文，让英文字母大写的按钮，点击了，键盘还是显示小写，
                                // 实际输出是大写」）。**只改显示**：onKey 送出去的仍是原字符，
                                // 大小写由 shift 语义/引擎决定（KeyRouter.handleKey 的英文直传）。
                                KeyButton(
                                    if (shiftActive) ch.uppercase() else ch,
                                    modifier = Modifier.weight(1f),
                                    onTap = { onKey(ch) },
                                    fontSize = keyFont,
                                )
                            }
                        }
                    }
                }
            }
            Row(modifier = Modifier.weight(if (showProKey) 0.95f else 1f).fillMaxSize()) {
                if (showProKey) {
                    // 英文 26 键（2026-10-19）：符号 → 普通/专业 → 空格 → 中英 → ⌫。
                    // 数字已在顶部 0-9；↵ 在第 3 行最右。
                    KeyButton(
                        "!?#",
                        modifier = Modifier.weight(1f),
                        onTap = onSymbol,
                        fontSize = keyFont,
                    )
                    KeyButton(
                        "pro",
                        modifier = Modifier.weight(1f),
                        onTap = onProToggle,
                        highlighted = proActive,
                        stateDescription = if (proActive) {
                            "专业模式，点击切回普通"
                        } else {
                            "普通模式，点击启用专业"
                        },
                        fontSize = keyFont,
                    )
                    KeyButton(
                        "空格",
                        modifier = Modifier.weight(3f),
                        onTap = onSpace,
                        fontSize = keyFont,
                    )
                    ModeKey(modeLabel, onModeSwitch, keyFont)
                    KeyButton(
                        "⌫",
                        modifier = Modifier.weight(1f),
                        onTap = onBackspace,
                        // 按住连续删除；短按仍只删一个（2026-10-23 用户裁决）。
                        repeatOnLongPress = true,
                        fontSize = keyFont,
                    )
                } else {
                    // 中文/其它键盘保持既有顺序：123 / 中英 / 空格 / ⌫ / ↵。
                    // 123 不挂长按：长按 500ms 吞 tap，导致面板打不开。
                    KeyButton(
                        "123",
                        modifier = Modifier.weight(1f),
                        onTap = onNumber,
                        fontSize = keyFont,
                    )
                    ModeKey(modeLabel, onModeSwitch, keyFont)
                    KeyButton(
                        "空格",
                        modifier = Modifier.weight(3f),
                        onTap = onSpace,
                        fontSize = keyFont,
                    )
                    KeyButton(
                        "⌫",
                        modifier = Modifier.weight(1f),
                        onTap = onBackspace,
                        // 按住连续删除；短按仍只删一个（2026-10-23 用户裁决）。
                        repeatOnLongPress = true,
                        fontSize = keyFont,
                    )
                    KeyButton(
                        "↵",
                        modifier = Modifier.weight(1f),
                        onTap = onEnter,
                        fontSize = keyFont,
                    )
                }
            }
        }
    }
}

@Composable
private fun RowScope.ModeKey(
    modeLabel: String,
    onModeSwitch: () -> Unit,
    keyFont: androidx.compose.ui.unit.TextUnit = 21.sp,
) {
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
        fontSize = keyFont,
    )
}

private val rows = listOf(
    listOf("q", "w", "e", "r", "t", "y", "u", "i", "o", "p"),
    listOf("a", "s", "d", "f", "g", "h", "j", "k", "l"),
    listOf("⇧", "z", "x", "c", "v", "b", "n", "m"),
)
private val englishRows = listOf(
    listOf("q", "w", "e", "r", "t", "y", "u", "i", "o", "p"),
    listOf("a", "s", "d", "f", "g", "h", "j", "k", "l"),
    // 回车与大写锁定同一行，回车最右（2026-10-19 用户裁决）。
    listOf("⇧", "z", "x", "c", "v", "b", "n", "m", "↵"),
)
private val numberRow = listOf("1", "2", "3", "4", "5", "6", "7", "8", "9", "0")

/** 实际渲染的键位行（数字行在字母行上方；英文第三行带 ↵）。JVM 单测直接核这张表。 */
internal fun qwertyRows(showNumberRow: Boolean, englishLayout: Boolean = false): List<List<String>> {
    val letters = if (englishLayout) englishRows else rows
    return if (showNumberRow) listOf(numberRow) + letters else letters
}

private const val SHIFT_LABEL = "⇧"
private const val ENTER_LABEL = "↵"
