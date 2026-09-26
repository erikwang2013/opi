// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package io.opi.input.keyboard

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * 通用键：圆角灰底，支持长按与高亮（⇧ 激活态用）。
 *
 * [stateDescription] 用于只靠背景色区分、读屏读不到的状态（⇧ 的 SINGLE/LOCK、当前输入模式）。
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun KeyButton(
    label: String,
    modifier: Modifier = Modifier,
    onTap: (() -> Unit)? = null,
    onLongPress: (() -> Unit)? = null,
    highlighted: Boolean = false,
    stateDescription: String? = null,
) {
    val bg = if (highlighted) Color(0xFF546E7A) else Color(0xFFE0E0E0) // blueGrey600 / grey300
    Box(
        modifier = modifier
            .padding(1.dp)
            .clip(RoundedCornerShape(6.dp))
            .background(bg)
            .combinedClickable(onClick = { onTap?.invoke() }, onLongClick = onLongPress)
            .semantics {
                role = Role.Button
                contentDescription = spokenKeyName(label)
                // 参数与接收者同名，必须 this. 限定
                if (stateDescription != null) this.stateDescription = stateDescription
                // pinyin 模式 ⇧ 是死键（onTap/onLongPress 皆 null）：别让读屏用户对着它双击
                if (onTap == null && onLongPress == null) disabled()
            }
            .fillMaxSize(),
        contentAlignment = Alignment.Center,
    ) {
        Text(label, fontSize = 18.sp, maxLines = 1)
    }
}

/**
 * 键面字形 → 读屏播报名。
 * 读屏会照着 Unicode 名念出 ⇧「向上白箭头」、⌫「向左擦除」、↵「向左下箭头」，
 * 故在 KeyButton 一处映射，qwerty / 数字面板 / 符号面板头部全部受益。
 */
internal fun spokenKeyName(label: String): String = when (label) {
    "⇧" -> "大写"
    "⌫" -> "退格"
    "↵" -> "回车"
    "空格" -> "空格"
    "中", "繁", "英" -> "中英切换"
    "123", "?123" -> "数字与符号"
    "ABC" -> "字母"
    "," -> "逗号"
    "." -> "句号"
    else -> if (label.length == 1 && label[0].isLetter()) "$label 键" else label
}
