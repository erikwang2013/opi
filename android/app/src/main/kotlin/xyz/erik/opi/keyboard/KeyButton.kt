// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

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
    val palette = LocalKeyboardPalette.current
    val bg = if (highlighted) palette.keyHighlightBg else palette.keyBg
    val fg = if (highlighted) palette.keyHighlightFg else palette.keyFg
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
        Text(label, fontSize = 18.sp, maxLines = 1, color = fg)
    }
}

/**
 * 模式键（中/繁/英）的读屏状态描述。键面只有一个字，读屏分不清它是「当前模式」
 * 还是「可切换项」—— QwertyKeyboard 与 9/14/笔画三个键盘共用这一处，别各写一份。
 */
internal fun modeStateDescription(modeLabel: String): String? = when (modeLabel) {
    "中" -> "中文"
    "繁" -> "繁体"
    "英" -> "英文"
    else -> null
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
    "符" -> "符号" // 9/14 键与笔画键盘的符号入口键
    "," , "，" -> "逗号"
    "." -> "句号"
    "生僻字" -> "生僻字键盘"
    else -> if (label.length == 1 && label[0].isLetter()) "$label 键" else label
}
