// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xyz.erik.opi.keyboard.LocalKeyboardPalette
import xyz.erik.opi.keyboard.OpiIcons
import xyz.erik.opi.pet.OpiPet
import xyz.erik.opi.pet.PetMood

/** 工具条高度（dp）：计入 ImeScreen 的列布局，不占键盘权重行。
 * 40（原 36）：2026-10-09 起小欧并入本行（用户裁决「入口要和 logo 图标在一行，
 * 放在 logo 后面」），给 28dp 的它留出上下呼吸。 */
const val IME_TOOLBAR_HEIGHT_DP = 44

/**
 * 键盘顶部工具条（2026-10-09，对齐搜狗移动端）：**入口全部用图标** ——
 * 表情直达、皮肤面板、输入模式面板、键盘模式面板。数据都是**外部状态 + 回调**
 * （无内部业务状态）。
 *
 * **输入模式/键盘模式自 2026-10-09 起不再用 DropdownMenu 弹框**（用户裁决
 * 「做成像选择皮肤的功能」）：点图标切 [ImeState.View.INPUT_MODE] /
 * [ImeState.View.KEYBOARD_MODE]，面板形态与皮肤/表情同一套（面板占键区、
 * 右上角关闭）—— 弹框那两个只盖一小块的浮层退役。
 */
@Composable
fun ImeToolbar(
    currentMode: InputMode,
    keyboardModes: KeyboardModes,
    /** 小欧的表情（由 ImeScreen 按引擎状态算好；本组件不碰引擎）。 */
    petMood: PetMood,
    onOpenEmoji: () -> Unit,
    onOpenSkin: () -> Unit,
    onOpenInputMode: () -> Unit,
    onOpenKeyboardMode: () -> Unit,
    /** 全角开关当前状态（图标两态：满月=全角、半月=半角，与桌面状态栏同款）。 */
    fullwidth: Boolean = false,
    /** 点月相 = 切全角/半角（2026-10-12 用户裁决「Android 补入口」）。 */
    onToggleFullwidth: () -> Unit = {},
    /** 收起键盘（2026-10-10 用户裁决：功能按钮区最右侧加关闭键盘图标）——
     * 服务层注入 requestHideSelf。 */
    onCloseKeyboard: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val p = LocalKeyboardPalette.current
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
        ToolbarIcon(OpiIcons.InputMode, "输入模式，当前 ${currentMode.label}") { onOpenInputMode() }
        ToolbarIcon(
            OpiIcons.KeyboardMode,
            "键盘模式${if (keyboardModes.anyOn) "，有开启项" else ""}",
        ) { onOpenKeyboardMode() }
        // 全角/半角（2026-10-12 用户裁决补入口）：图标两态 + 状态进 contentDescription
        //（与「输入模式，当前 …」同一口径：只说当前状态，不描述点击行为）。
        ToolbarIcon(
            if (fullwidth) OpiIcons.MoonFull else OpiIcons.MoonHalf,
            "全角/半角，当前${if (fullwidth) "全角" else "半角"}",
        ) { onToggleFullwidth() }
        // 收起键盘压在**最右**（用户裁决「功能按钮区域最右侧」）：弹性空白把它推到头。
        // 图标 = 向下折角（2026-10-10 用户裁决「不要用 x」——✕ 是关闭语义）。
        Spacer(Modifier.weight(1f))
        ToolbarIcon(OpiIcons.KeyboardHide, "收起键盘") { onCloseKeyboard() }
    }
}

@Composable
private fun ToolbarIcon(icon: ImageVector, description: String, onClick: () -> Unit) {
    IconButton(onClick = onClick) {
        Icon(icon, contentDescription = description, tint = LocalKeyboardPalette.current.toolbarFg)
    }
}

/** 面板头（皮肤/输入模式/键盘模式共用同一形态：标题 + 提示 + 右上角关闭）。 */
@Composable
private fun PanelHeader(title: String, hint: String, closeDesc: String, onClose: () -> Unit) {
    val p = LocalKeyboardPalette.current
    Row(
        modifier = Modifier.fillMaxWidth().padding(start = 14.dp, end = 4.dp, top = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(title, fontSize = 15.sp, fontWeight = FontWeight.Bold, color = p.panelFg)
        Spacer(Modifier.width(10.dp))
        Text(hint, fontSize = 11.sp, color = p.barDimFg)
        Spacer(Modifier.weight(1f))
        IconButton(onClick = onClose) {
            Icon(OpiIcons.Close, contentDescription = closeDesc, tint = p.toolbarFg)
        }
    }
}

/**
 * 面板里的一枚选项（文字 chip）：选中 = 强调色描边 + 粗体 + 行尾 ✓ ——
 * 与旧弹框菜单的「行尾 ✓」同一套视觉语言，换了容器而已。
 * [role] 用来分「单选」（输入模式、单手）与「复选」（繁体/悬浮/…开关）。
 */
@Composable
private fun PanelChip(
    label: String,
    selected: Boolean,
    description: String,
    role: Role,
    onClick: () -> Unit,
) {
    val p = LocalKeyboardPalette.current
    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(8.dp))
            .background(p.keyBg)
            .border(
                width = if (selected) 2.dp else 1.dp,
                color = if (selected) p.accent else p.barDimFg.copy(alpha = 0.35f),
                shape = RoundedCornerShape(8.dp),
            )
            .clickable(onClick = onClick)
            .semantics {
                this.role = role
                contentDescription = description
            }
            .padding(horizontal = 14.dp, vertical = 9.dp),
    ) {
        Text(
            if (selected) "$label ✓" else label,
            fontSize = 13.sp,
            color = if (selected) p.accent else p.panelFg,
            fontWeight = if (selected) FontWeight.Bold else FontWeight.Normal,
        )
    }
}

/** 面板选项区的排布（FlowRow 折行 + 可滚动：窄屏/长辈模式字号下都不裁项）。 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun PanelChips(content: @Composable () -> Unit) {
    Column(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp)
            .verticalScroll(rememberScrollState()),
    ) {
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) { content() }
    }
}

/**
 * 输入模式面板（2026-10-09 用户裁决「移动端输入模式打开做成像选择皮肤的功能，
 * 不要现在的弹框」）。选中即换：走 [ImeState.selectInputMode]，由它收尾
 * （面板项回键面、生僻字回浏览面板）—— 所以**不自动关**这件事由选择本身完成。
 */
@Composable
fun InputModePanel(
    current: InputMode,
    onSelect: (InputMode) -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(modifier = modifier.fillMaxSize().background(LocalKeyboardPalette.current.panelBg)) {
        PanelHeader("输入模式", "选中即换", "关闭输入模式面板", onClose)
        PanelChips {
            for (m in InputMode.entries) {
                PanelChip(
                    label = m.label,
                    selected = m == current,
                    description = "输入模式 ${m.label}${if (m == current) "，已选中" else ""}",
                    role = Role.RadioButton,
                ) { onSelect(m) }
            }
        }
    }
}

/**
 * 键盘模式面板（同上）。**点选项不关面板** —— 键盘模式常常要连开两三个
 * （比如「暗光 + 单手」），每点一次都收起再展开是把常用路径变长（原弹框的注释
 * 原样保留其理由）；关闭走右上角，与皮肤面板同一条。
 * 繁体是**引擎模式**（不在 [KeyboardModes] 里），由宿主传入单独的回调。
 */
@Composable
fun KeyboardModePanel(
    traditional: Boolean,
    keyboardModes: KeyboardModes,
    onToggleTraditional: () -> Unit,
    onUpdate: (KeyboardModes) -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(modifier = modifier.fillMaxSize().background(LocalKeyboardPalette.current.panelBg)) {
        PanelHeader("键盘模式", "可连开多个；再点一下关", "关闭键盘模式面板", onClose)
        PanelChips {
            // —— 繁体：引擎模式开关（与「中/繁/英」键同一条路） ——
            PanelChip(
                label = "繁体模式",
                selected = traditional,
                description = "繁体模式${if (traditional) "，已开启" else ""}",
                role = Role.Checkbox,
                onClick = onToggleTraditional,
            )
            // —— 单手：三选一（关 / 左手 / 右手） ——
            for (hand in KeyboardModes.OneHand.entries) {
                val label = when (hand) {
                    KeyboardModes.OneHand.OFF -> "单手模式：关"
                    KeyboardModes.OneHand.LEFT -> "单手模式：左手"
                    KeyboardModes.OneHand.RIGHT -> "单手模式：右手"
                }
                PanelChip(
                    label = label,
                    selected = keyboardModes.oneHand == hand,
                    description = "$label${if (keyboardModes.oneHand == hand) "，已选中" else ""}",
                    role = Role.RadioButton,
                ) { onUpdate(keyboardModes.copy(oneHand = hand)) }
            }
            PanelChip(
                "悬浮模式",
                keyboardModes.floating,
                "悬浮模式${if (keyboardModes.floating) "，已开启" else ""}",
                Role.Checkbox,
            ) { onUpdate(keyboardModes.copy(floating = !keyboardModes.floating)) }
            PanelChip(
                "游戏模式",
                keyboardModes.game,
                "游戏模式${if (keyboardModes.game) "，已开启" else ""}",
                Role.Checkbox,
            ) { onUpdate(keyboardModes.copy(game = !keyboardModes.game)) }
            PanelChip(
                "暗光模式",
                keyboardModes.dark,
                "暗光模式${if (keyboardModes.dark) "，已开启" else ""}",
                Role.Checkbox,
            ) { onUpdate(keyboardModes.copy(dark = !keyboardModes.dark)) }
            PanelChip(
                "长辈模式",
                keyboardModes.elder,
                "长辈模式${if (keyboardModes.elder) "，已开启" else ""}",
                Role.Checkbox,
            ) { onUpdate(keyboardModes.copy(elder = !keyboardModes.elder)) }
        }
    }
}
