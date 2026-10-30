// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.AwaitPointerEventScope
import androidx.compose.ui.input.pointer.PointerId
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.withTimeoutOrNull
import kotlin.math.roundToInt

/** ⌫ 按住多久开始连续删除（2026-10-23 用户裁决：0.5s）。 */
internal const val BACKSPACE_LONG_PRESS_MS = 500L

/** 连续删除的重复间隔（约 16 次/秒）。 */
internal const val BACKSPACE_REPEAT_INTERVAL_MS = 60L

/** 本轮按下是否已经由连续删除接管：挡掉 clickable 在抬手时多发的那一次 onClick。 */
private class RepeatPressState(var fired: Boolean = false)

/** 本轮按下是否已经由备选弹层接管：同上，挡掉抬手时 clickable 多发的那次 onClick。 */
private class AlternativesPressState(var fired: Boolean = false)

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
    /**
     * 长按或上滑打开的备选键面（2026-10-24，26 键字母键用）。空表 = 无弹层，
     * 行为与旧版完全一致。有弹层时 [onLongPress] 不再触发（长按归弹层）。
     */
    alternatives: List<KeyAlternative> = emptyList(),
    /** 弹层初始选中下标；越界按 [alternatives] 末项夹住。 */
    defaultAlternative: Int = 0,
    /** 弹层抬手提交选中项的 [KeyAlternative.value]；null 时退回 [onTap]。 */
    onAlternative: ((KeyAlternative) -> Unit)? = null,
    /** 键面上方的小字提示（26 键的符号）。 */
    hint: String? = null,
    /** 键面右上角的小字（拼音 26 键 Q..P 的 1-0）。 */
    cornerHint: String? = null,
) {
    val palette = LocalKeyboardPalette.current
    val fg = if (highlighted) palette.keyHighlightFg else palette.keyFg
    val press = remember { RepeatPressState() }
    val latestOnTap = rememberUpdatedState(onTap)
    val hasAlternatives = alternatives.isNotEmpty()
    val altPress = remember { AlternativesPressState() }
    var popupOpen by remember { mutableStateOf(false) }
    var popupSelected by remember { mutableStateOf(0) }
    // 弹层要夹在键盘窗口里：量的就是承载本键的窗口视图（IME 里 = 键盘窗），
    // 不是 Compose provider 给的整块可见屏。
    val hostView = LocalView.current
    var popupWindowSize by remember { mutableStateOf(IntSize.Zero) }
    val latestAlternatives = rememberUpdatedState(alternatives)
    val latestOnAlternative = rememberUpdatedState(onAlternative)
    val popupItemWidthPx = with(LocalDensity.current) { KEY_POPUP_ITEM_WIDTH.toPx() }
    // 长按超时或上滑过阈值 → 开弹层；之后手指的**横向**位移持续改选中项，抬手提交。
    // 与 repeatModifier 一样**不消费**事件：短按仍走 combinedClickable 的 onClick。
    val alternativesModifier = if (!hasAlternatives) {
        Modifier
    } else {
        Modifier.pointerInput(alternatives, defaultAlternative, popupItemWidthPx) {
            awaitEachGesture {
                val down = awaitFirstDown(requireUnconsumed = false)
                altPress.fired = false
                var dy = 0f
                val releasedEarly: Boolean? =
                    withTimeoutOrNull(viewConfiguration.longPressTimeoutMillis) {
                        var released = false
                        var swiped = false
                        while (!released && !swiped) {
                            val change = awaitPointerEvent().changes.firstOrNull { it.id == down.id }
                            if (change == null || !change.pressed) {
                                released = true
                            } else {
                                dy += change.positionChange().y
                                swiped = -dy > viewConfiguration.touchSlop
                            }
                        }
                        !swiped
                    }
                // 阈值内抬手 = 普通点击，交给 onClick；超时(null)/上滑(false)才开弹层。
                if (releasedEarly == true) return@awaitEachGesture
                altPress.fired = true
                val count = latestAlternatives.value.size
                val start = defaultAlternative.coerceIn(0, count - 1)
                popupSelected = start
                popupWindowSize = IntSize(hostView.width, hostView.height)
                popupOpen = true
                try {
                    var dragDx = 0f
                    var released = false
                    while (true) {
                        val change = awaitPointerEvent().changes.firstOrNull { it.id == down.id } ?: break
                        if (!change.pressed) {
                            released = true
                            break
                        }
                        dragDx += change.positionChange().x
                        // 向右拖 = 选更靠右的格（首项在最左）。
                        popupSelected = popupSelectionForDrag(start, dragDx, popupItemWidthPx, count)
                    }
                    if (released) {
                        val alt = latestAlternatives.value[popupSelected.coerceIn(0, count - 1)]
                        val handler = latestOnAlternative.value
                        if (handler != null) handler(alt) else latestOnTap.value?.invoke()
                    }
                } finally {
                    popupOpen = false
                }
            }
        }
    }
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
            .keyPlate(highlighted)
            .combinedClickable(
                onClick = {
                    // 连续删除 / 备选弹层已经接管时，抬手还会再发一次 onClick —— 挡掉。
                    if (!press.fired && !altPress.fired) latestOnTap.value?.invoke()
                    press.fired = false
                    altPress.fired = false
                },
                // 有弹层时长按归弹层（先开弹层、抬手提交），旧长按回调不再触发。
                onLongClick = if (hasAlternatives) null else onLongPress,
            )
            .then(repeatModifier)
            .then(alternativesModifier)
            .semantics {
                role = Role.Button
                contentDescription = description ?: spokenKeyName(label)
                // 参数与接收者同名，必须 this. 限定
                if (stateDescription != null) this.stateDescription = stateDescription
                // pinyin 模式 ⇧ 是死键（onTap/onLongPress/备选皆 null）：别让读屏用户对着它双击
                if (onTap == null && onLongPress == null && onAlternative == null && !hasAlternatives) {
                    disabled()
                }
            }
            .fillMaxSize(),
        contentAlignment = Alignment.Center,
    ) {
        // 26 键的符号提示 / 拼音 Q 行的数字角标：只作视觉提示，读屏名仍走 contentDescription。
        val hintFont = fontSize * 0.45f
        if (hint != null) {
            Text(
                hint,
                fontSize = hintFont,
                maxLines = 1,
                color = palette.barDimFg,
                modifier = Modifier.align(Alignment.TopCenter).padding(top = 2.dp),
            )
        }
        if (cornerHint != null) {
            Text(
                cornerHint,
                fontSize = hintFont,
                maxLines = 1,
                color = palette.barDimFg,
                modifier = Modifier.align(Alignment.TopEnd).padding(top = 2.dp, end = 4.dp),
            )
        }
        // Ellipsis：窄屏单手/320dp 下「生僻字」「英文」这类多字键不再被硬裁成半个字
        // （读者仍能看出是哪个键；完整读屏名走 contentDescription）。
        Text(label, fontSize = fontSize, maxLines = 1, overflow = TextOverflow.Ellipsis, color = fg)
        if (popupOpen) {
            KeyAlternativesPopup(
                alternatives,
                popupSelected,
                anchorIndex = defaultAlternative.coerceIn(0, alternatives.lastIndex.coerceAtLeast(0)),
                fontSize,
                popupWindowSize,
            )
        }
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
    "↵", "换行" -> "回车"
    "空格" -> "空格"
    "!?#" -> "符号"
    "中" -> "切到中文"
    "英" -> "切到英文"
    "繁" -> "切到繁体"
    "中英" -> "中英切换"
    "123", "?123" -> "数字与符号"
    "ABC" -> "字母"
    "符" -> "符号" // 9/14 键与笔画键盘的符号入口键
    "," , "，" -> "逗号"
    "." -> "句号"
    "生僻字" -> "生僻字键盘"
    else -> if (label.length == 1 && label[0].isLetter()) "$label 键" else label
}
