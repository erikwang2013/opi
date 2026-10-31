// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xyz.erik.opi.ime.opiPanelBackground

/**
 * 9 键左列「自定义」标点的选择器（2026-10-24）：常用符号网格，点一格 = 选中并关闭。
 *
 * 画在键区**之上**（与键盘同层、不是系统 Dialog）：IME 窗口里的 Dialog 要另起窗口，
 * 真机焦点/尺寸都不可控；这里是纯 Compose 覆盖层。根节点吞掉空白处的点击，
 * 免得漏到下面的键。
 */
@Composable
internal fun CustomSymbolPicker(
    symbols: List<String>,
    onPick: (String) -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val palette = LocalKeyboardPalette.current
    Column(
        modifier = modifier
            .fillMaxSize()
            .opiPanelBackground(palette.panelBg)
            // 吞掉空白处点击（网格/按钮的子节点先拿到事件）。
            .pointerInput(Unit) { detectTapGestures { } },
    ) {
        Row(
            modifier = Modifier.fillMaxWidth().height(44.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            KeyButton("返回", modifier = Modifier.weight(1f), onTap = onClose, fontSize = 17.sp)
            Text(
                "选择自定义符号",
                modifier = Modifier.weight(4f).padding(start = 8.dp),
                fontSize = 15.sp,
            )
        }
        Box(modifier = Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
            val shown = remember(symbols) { symbols.distinct() }
            if (shown.isEmpty()) {
                // 引擎没就绪/常用表为空：给一句话，别让用户对着空白面板。
                Text("暂无可用符号", fontSize = 15.sp, color = palette.barDimFg)
            } else {
                SymbolGrid(shown, onTap = onPick)
            }
        }
    }
}
