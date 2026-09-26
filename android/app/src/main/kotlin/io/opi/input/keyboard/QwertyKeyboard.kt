package io.opi.input.keyboard

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import io.opi.input.engine.ShiftState

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
) {
    val shiftActive = shiftState != null && shiftState != ShiftState.OFF
    Column(modifier = modifier.fillMaxSize()) {
        for (row in rows) {
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
                        KeyButton(ch, modifier = Modifier.weight(1f), onTap = { onKey(ch) })
                    }
                }
            }
        }
        Row(modifier = Modifier.weight(1f).fillMaxSize()) {
            KeyButton(
                modeLabel,
                modifier = Modifier.weight(1f),
                onTap = onModeSwitch,
                // 键面只有「中/繁/英」一个字，读屏分不清这是当前模式还是可切换项
                stateDescription = when (modeLabel) {
                    "中" -> "中文"
                    "繁" -> "繁体"
                    "英" -> "英文"
                    else -> null
                },
            )
            // 123 不挂长按：长按 500ms 吞 tap，导致面板打不开（符号经数字面板进入）
            KeyButton("123", modifier = Modifier.weight(1f), onTap = onNumber)
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
private const val SHIFT_LABEL = "⇧"
