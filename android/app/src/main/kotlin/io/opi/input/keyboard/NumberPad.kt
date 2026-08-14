package io.opi.input.keyboard

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier

/**
 * 数字面板：Gboard 风格 5 行（对齐 flutter number_pad.dart）。
 * 数字/标点直接提交，不经过引擎（引擎 Number 模式无可用提交路径）。
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
    Column(modifier = modifier.fillMaxSize()) {
        for (row in numberRows) {
            Row(modifier = Modifier.weight(1f).fillMaxSize()) {
                for (ch in row) {
                    KeyButton(ch, modifier = Modifier.weight(1f), onTap = { onKey(ch) })
                }
            }
        }
        Row(modifier = Modifier.weight(1f).fillMaxSize()) {
            KeyButton("ABC", modifier = Modifier.weight(1f), onTap = onLetters)
            KeyButton("?123", modifier = Modifier.weight(1f), onTap = onSymbol)
            KeyButton("空格", modifier = Modifier.weight(5f), onTap = onSpace)
            KeyButton("⌫", modifier = Modifier.weight(1f), onTap = onBackspace)
            KeyButton("↵", modifier = Modifier.weight(1f), onTap = onEnter)
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
