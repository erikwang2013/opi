// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.AwaitPointerEventScope
import androidx.compose.ui.input.pointer.PointerId
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.withTimeoutOrNull

/** ⌫ 按住多久开始连续删除（2026-10-23 用户裁决：0.5s）。 */
internal const val BACKSPACE_LONG_PRESS_MS = 500L

/** 连续删除的重复间隔（约 16 次/秒）。 */
internal const val BACKSPACE_REPEAT_INTERVAL_MS = 60L

/** 本轮按下是否已经由连续删除接管：挡掉 clickable 在抬手时多发的那一次 onClick。 */
private class RepeatPressState(var fired: Boolean = false)

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
    /** 读屏名覆盖（默认 null = 按 [spokenKeyName] 从 label 推）。用于**同一键面字在不同
     * 键盘上动作不同**的键：9 键的模式键只切简/繁（2026-10-15 用户裁决），而 label
     * 「中/繁」在 [spokenKeyName] 里的默认读法是「中英切换」—— 会念错动作。 */
    description: String? = null,
    /** 键面字号（默认 21sp；英文 26 键用更大字号）。 */
    fontSize: androidx.compose.ui.unit.TextUnit = 21.sp,
    /**
     * ⌫ 专用（2026-10-23 用户裁决「按住连续删除」）：短按仍只触发一次 [onTap]；
     * 按住 [BACKSPACE_LONG_PRESS_MS] 后立即触发一次，之后每
     * [BACKSPACE_REPEAT_INTERVAL_MS] 一次，抬手/滑出键面/手势取消立即停。
     * 与 [onLongPress] 互斥（同时给会各触发一次）。
     */
    repeatOnLongPress: Boolean = false,
) {
    val palette = LocalKeyboardPalette.current
    val bg = if (highlighted) palette.keyHighlightBg else palette.keyBg
    val fg = if (highlighted) palette.keyHighlightFg else palette.keyFg
    val press = remember { RepeatPressState() }
    val latestOnTap = rememberUpdatedState(onTap)
    // 观察器**不消费**事件：同一键上的 combinedClickable 还要靠它们出点击与涟漪。
    val repeatModifier = if (!repeatOnLongPress) {
        Modifier
    } else {
        Modifier.pointerInput(Unit) {
            awaitEachGesture {
                val down = awaitFirstDown(requireUnconsumed = false)
                press.fired = false // 新的一轮按下：清掉上一次长按留下的抑制标记
                val upBeforeTimeout = withTimeoutOrNull(BACKSPACE_LONG_PRESS_MS) {
                    awaitReleaseOrCancel(down.id)
                }
                if (upBeforeTimeout != null) return@awaitEachGesture // 短按/取消：交给 onClick
                press.fired = true
                latestOnTap.value?.invoke()
                while (true) {
                    val released = withTimeoutOrNull(BACKSPACE_REPEAT_INTERVAL_MS) {
                        awaitReleaseOrCancel(down.id)
                    }
                    if (released != null) break
                    latestOnTap.value?.invoke()
                }
            }
        }
    }
    Box(
        modifier = modifier
            .padding(1.dp)
            .clip(RoundedCornerShape(6.dp))
            .background(bg)
            .combinedClickable(
                onClick = {
                    // 连续删除已经接管时，抬手还会再发一次 onClick —— 挡掉。
                    if (!press.fired) latestOnTap.value?.invoke()
                    press.fired = false
                },
                onLongClick = onLongPress,
            )
            .then(repeatModifier)
            .semantics {
                role = Role.Button
                contentDescription = description ?: spokenKeyName(label)
                // 参数与接收者同名，必须 this. 限定
                if (stateDescription != null) this.stateDescription = stateDescription
                // pinyin 模式 ⇧ 是死键（onTap/onLongPress 皆 null）：别让读屏用户对着它双击
                if (onTap == null && onLongPress == null) disabled()
            }
            .fillMaxSize(),
        contentAlignment = Alignment.Center,
    ) {
        // Ellipsis：窄屏单手/320dp 下「生僻字」「英文」这类多字键不再被硬裁成半个字
        // （读者仍能看出是哪个键；完整读屏名走 contentDescription）。
        Text(label, fontSize = fontSize, maxLines = 1, overflow = TextOverflow.Ellipsis, color = fg)
    }
}

/**
 * 等到这根手指抬起或手势取消；滑出键面按取消算（与 Compose 点击手势同一方向）。
 * 只观察 pressed/位置，不消费事件 —— 同键的 combinedClickable 仍要正常完成点击。
 */
private suspend fun AwaitPointerEventScope.awaitReleaseOrCancel(id: PointerId) {
    while (true) {
        val change = awaitPointerEvent().changes.firstOrNull { it.id == id } ?: return
        val outOfBounds = change.position.x < 0f || change.position.y < 0f ||
            change.position.x > size.width || change.position.y > size.height
        if (!change.pressed || outOfBounds) return
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
