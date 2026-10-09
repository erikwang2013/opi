// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xyz.erik.opi.keyboard.LocalKeyboardPalette
import xyz.erik.opi.keyboard.OpiIcons
import xyz.erik.opi.pet.OpiPet
import xyz.erik.opi.pet.PetMood

/** 工具条高度（dp）：计入 ImeScreen 的列布局，不占键盘权重行。
 * 40（原 36）：2026-10-09 起小欧并入本行（用户裁决「入口要和 logo 图标在一行，
 * 放在 logo 后面」），给 28dp 的它留出上下呼吸。 */
const val IME_TOOLBAR_HEIGHT_DP = 40

/**
 * 键盘顶部工具条（2026-10-09，对齐搜狗移动端）：**入口全部用图标** ——
 * 表情直达、输入模式菜单、键盘模式菜单。数据都是**外部状态 + 回调**（无内部业务
 * 状态，菜单展开这类纯 UI 状态留在本组件）。
 *
 * [traditional] 单独传：繁体是**引擎模式**（EngineController.mode），不在
 * [KeyboardModes] 里 —— 菜单里那一项直接读写引擎。
 */
@Composable
fun ImeToolbar(
    currentMode: InputMode,
    keyboardModes: KeyboardModes,
    traditional: Boolean,
    /** 小欧的表情（由 ImeScreen 按引擎状态算好；本组件不碰引擎）。 */
    petMood: PetMood,
    onOpenEmoji: () -> Unit,
    onOpenSkin: () -> Unit,
    onSelectInputMode: (InputMode) -> Unit,
    onToggleTraditional: () -> Unit,
    onUpdateKeyboardModes: (KeyboardModes) -> Unit,
    modifier: Modifier = Modifier,
) {
    val p = LocalKeyboardPalette.current
    var inputMenu by remember { mutableStateOf(false) }
    var kbMenu by remember { mutableStateOf(false) }
    Row(
        modifier = modifier.fillMaxWidth().height(IME_TOOLBAR_HEIGHT_DP.dp)
            .background(p.toolbarBg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // 小欧在最左（2026-10-09 用户裁决：入口与 logo 同行、在其后）。它此前站在
        // 候选栏槽位的待命位 —— 移到本行后那个槽位空闲时就是空的（不多画东西）。
        OpiPet(mood = petMood, size = 28.dp, modifier = Modifier.padding(start = 6.dp))
        Spacer(Modifier.width(2.dp))
        ToolbarIcon(OpiIcons.Emoji, "表情") { onOpenEmoji() }
        ToolbarIcon(OpiIcons.Palette, "皮肤") { onOpenSkin() }
        Box {
            ToolbarIcon(
                OpiIcons.InputMode,
                "输入模式，当前 ${currentMode.label}",
            ) { inputMenu = true }
            DropdownMenu(expanded = inputMenu, onDismissRequest = { inputMenu = false }) {
                for (m in InputMode.entries) {
                    DropdownMenuItem(
                        text = { Text(m.label) },
                        onClick = {
                            inputMenu = false
                            onSelectInputMode(m)
                        },
                        trailingIcon = {
                            if (m == currentMode) Text("✓", fontSize = 14.sp)
                        },
                    )
                }
            }
        }
        Box {
            ToolbarIcon(
                OpiIcons.KeyboardMode,
                "键盘模式${if (keyboardModes.anyOn || traditional) "，有开启项" else ""}",
            ) { kbMenu = true }
            DropdownMenu(expanded = kbMenu, onDismissRequest = { kbMenu = false }) {
                // —— 繁体：引擎模式开关（与「中/繁/英」键同一条路） ——
                ToggleRow("繁体模式", traditional) {
                    kbMenu = false
                    onToggleTraditional()
                }
                // —— 单手：三选一（关 / 左手 / 右手） ——
                for (hand in KeyboardModes.OneHand.entries) {
                    val label = when (hand) {
                        KeyboardModes.OneHand.OFF -> "单手模式：关"
                        KeyboardModes.OneHand.LEFT -> "单手模式：左手"
                        KeyboardModes.OneHand.RIGHT -> "单手模式：右手"
                    }
                    ToggleRow(label, keyboardModes.oneHand == hand) {
                        onUpdateKeyboardModes(keyboardModes.copy(oneHand = hand))
                    }
                }
                ToggleRow("悬浮模式", keyboardModes.floating) {
                    onUpdateKeyboardModes(keyboardModes.copy(floating = !keyboardModes.floating))
                }
                ToggleRow("游戏模式", keyboardModes.game) {
                    onUpdateKeyboardModes(keyboardModes.copy(game = !keyboardModes.game))
                }
                ToggleRow("暗光模式", keyboardModes.dark) {
                    onUpdateKeyboardModes(keyboardModes.copy(dark = !keyboardModes.dark))
                }
                ToggleRow("长辈模式", keyboardModes.elder) {
                    onUpdateKeyboardModes(keyboardModes.copy(elder = !keyboardModes.elder))
                }
            }
        }
    }
}

@Composable
private fun ToolbarIcon(icon: ImageVector, description: String, onClick: () -> Unit) {
    IconButton(onClick = onClick) {
        Icon(icon, contentDescription = description, tint = LocalKeyboardPalette.current.toolbarFg)
    }
}

/**
 * 菜单里的一行开关。点击后**不自动关菜单** —— 键盘模式常常要连开两三个
 * （比如「暗光 + 单手」），每点一次都收起再展开是把常用路径变长。
 * 表达「开了」用行尾的 ✓（与读屏描述同一个判据，见 accessibility 注释）。
 */
@Composable
private fun ToggleRow(label: String, checked: Boolean, onClick: () -> Unit) {
    DropdownMenuItem(
        text = { Text(label) },
        onClick = onClick,
        trailingIcon = { if (checked) Text("✓", fontSize = 14.sp) },
    )
}
