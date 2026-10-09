// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 虚拟键盘窗（2026-10-10 用户裁决「虚拟键盘本期就做」）：无边框 + 置顶 +
//! **不抢焦点**的 Compose 键盘；按键经 [injectKey] 注入系统（实现见 KeyInject.kt）。
//!
//! 键盘面 = qwerty + 数字行 + 空格/回车/退格（用户裁决的键集，本版不加别的键）。
//! 键位表与「键 → 注入序列」的映射都是**纯数据/纯函数**（VirtualKeyboardTest 钉住）：
//! 注入是唯一有副作用的一步，抽出来才能在本机（无 Windows）验其余部分。
//!
//! ⚠️ 注入的键会照常流经输入法：中文态下注入 "n" 出候选、空格选词都是**正确行为**
//! ——它就是一次真实的系统按键，本模块**不试图绕过** fcitx5/TSF。
//!
//! ⚠️ 不抢焦点是**承重**的：窗口一旦拿到键盘焦点，注入的键会落回本窗自己
//! （`focusable = false`，与候选窗同一条理由）。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.WindowPosition
import androidx.compose.ui.window.rememberWindowState
import kotlinx.coroutines.delay

// ---------- 键位表（纯数据） ----------

/**
 * 虚拟键盘的一个键。
 *
 * 两个平台标识并列存放（不是二选一）：`keysym` 是 X11 的 keysym 名
 * （`XStringToKeysym` 的输入，见 KeyInject.kt），`vk` 是 Windows 的虚拟键码
 * （`SendInput` 的 `wVk`）。两列都由 VirtualKeyboardTest 逐键钉住 —— 错一个字节
 * 的表现是「某个键点下去没反应/出别的字符」，两端都不会报错。
 */
data class VkKey(val label: String, val keysym: String, val vk: Int)

/** 字母键：X11 小写 keysym（`a`）；Windows VK 用大写字母码（'A' = 0x41）。 */
private fun letter(c: Char) = VkKey(c.toString(), c.toString(), c.uppercaseChar().code)

/** 数字键：两个平台都与 ASCII 码同值。 */
private fun digit(c: Char) = VkKey(c.toString(), c.toString(), c.code)

/** 空格（X11 keysym 名 `space`；Windows VK_SPACE = 0x20）。 */
val VK_SPACE = VkKey("空格", "space", 0x20)

/** 回车（X11 `Return`；Windows VK_RETURN = 0x0D）。 */
val VK_ENTER = VkKey("回车", "Return", 0x0D)

/** 退格（X11 `BackSpace`；Windows VK_BACK = 0x08）。 */
val VK_BACKSPACE = VkKey("退格", "BackSpace", 0x08)

/** 字母区四行（数字行 + 三行 qwerty）。底行（空格/回车/退格）单独放 [VK_BOTTOM]。 */
val VK_ROWS: List<List<VkKey>> = listOf(
    "1234567890".map(::digit),
    "qwertyuiop".map(::letter),
    "asdfghjkl".map(::letter),
    "zxcvbnm".map(::letter),
)

/** 底行：[VK_SPACE]（6 格宽）/[VK_ENTER]（2 格）/[VK_BACKSPACE]（2 格）。 */
val VK_BOTTOM: List<Pair<VkKey, Int>> = listOf(
    VK_SPACE to 6,
    VK_ENTER to 2,
    VK_BACKSPACE to 2,
)

// ---------- 键 → 注入序列（纯函数） ----------

/** 一次注入事件：某个键的按下（[down] = true）或抬起。 */
data class VkEvent(val key: VkKey, val down: Boolean)

/**
 * 纯函数：一个键 → 交给注入器的**事件序列**。
 *
 * 本版每个键恒为「按下 + 抬起」两条；把它写成序列而不是单个键，是为了组合键
 * （未来的 Shift+字母之类）在这里就能表达 —— 注入器只按序执行，不改语义。
 */
fun vkEventSequence(key: VkKey): List<VkEvent> = listOf(VkEvent(key, true), VkEvent(key, false))

// ---------- 窗口 ----------

private val KbBg = Color(0xFF232323)
private val KeyBg = Color(0xFF3A3A3A)
private val KeyPressed = Color(0xFF4C8DFF)
private val KbFg = Color(0xFFE8E8E8)
private val KbDim = Color(0xFFFF8A80)

private const val KB_WIDTH_DP = 500
private const val KB_HEIGHT_DP = 246

/** 屏幕底部居中（AWT 逻辑像素 = .dp 数值，见 Main.kt 头注释的 position 段）。 */
private fun keyboardPosition(): WindowPosition {
    val screen = java.awt.Toolkit.getDefaultToolkit().screenSize
    return WindowPosition(
        ((screen.width - KB_WIDTH_DP) / 2).dp,
        (screen.height - KB_HEIGHT_DP - 60).dp,
    )
}

/**
 * 虚拟键盘窗。由状态栏的「软键盘」按钮开关（[StatusBarApp]），本窗自身没有关闭
 * 按钮（无边框、不抢焦点，关不了也不会挡事）。
 */
@Composable
fun VirtualKeyboardWindow(onClose: () -> Unit) {
    var notice by remember { mutableStateOf<String?>(null) }
    val windowState = rememberWindowState(
        width = KB_WIDTH_DP.dp,
        height = KB_HEIGHT_DP.dp,
        position = remember { keyboardPosition() },
    )
    LaunchedEffect(notice) {
        if (notice != null) {
            delay(4000)
            notice = null
        }
    }
    Window(
        onCloseRequest = onClose,
        state = windowState,
        undecorated = true,
        alwaysOnTop = true,
        resizable = false,
        focusable = false, // 承重：见文件头 ⚠️
        title = "opi输入法 虚拟键盘",
    ) {
        Column(
            modifier = Modifier.fillMaxSize().background(KbBg).padding(6.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            for (row in VK_ROWS) {
                Row(
                    modifier = Modifier.fillMaxWidth().weight(1f),
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    for (key in row) {
                        VkKeyButton(key, Modifier.weight(1f)) { press(key) { notice = it } }
                    }
                }
            }
            Row(
                modifier = Modifier.fillMaxWidth().weight(1f),
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                for ((key, weight) in VK_BOTTOM) {
                    VkKeyButton(key, Modifier.weight(weight.toFloat())) { press(key) { notice = it } }
                }
            }
            // 注入失败提示（别静默）：固定高度，避免提示出现时整窗跳动。
            Box(Modifier.fillMaxWidth().height(16.dp), contentAlignment = Alignment.Center) {
                notice?.let { Text(it, fontSize = 11.sp, color = KbDim, maxLines = 1) }
            }
        }
    }
}

/** 点击一个键：按纯函数给的序列注入；失败（非 null 原因）交给 [onFailure] 显示。 */
private fun press(key: VkKey, onFailure: (String) -> Unit) {
    for (ev in vkEventSequence(key)) {
        val failure = injectKey(ev.key, down = ev.down)
        if (failure != null) {
            onFailure(failure)
            return
        }
    }
}

@Composable
private fun VkKeyButton(key: VkKey, modifier: Modifier, onClick: () -> Unit) {
    val interaction = remember { MutableInteractionSource() }
    val pressed by interaction.collectIsPressedAsState()
    Box(
        modifier = modifier
            .fillMaxSize()
            .background(if (pressed) KeyPressed else KeyBg)
            .clickable(interactionSource = interaction, indication = null, onClick = onClick)
            .semantics { contentDescription = "虚拟键盘 ${key.label}" },
        contentAlignment = Alignment.Center,
    ) {
        Text(
            key.label,
            fontSize = if (key.label.length > 1) 13.sp else 16.sp,
            color = KbFg,
            maxLines = 1,
        )
    }
}
