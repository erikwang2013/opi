// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import xyz.erik.opi.ime.PanelHeader
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * 皮肤面板（2026-10-09 用户裁决「移动端没有皮肤设置」）。
 *
 * 形态对齐桌面设置窗的皮肤选择器与主流输入法的皮肤页：**五张预览卡**，点一张换一张 ——
 * 预览就是**小键盘样张**（底色 + 键面 + 一枚强调键），不是纯色块，也不是文字清单。
 * 面板**不自动关**：皮肤要连换着比才顺手；左上角返回箭头走
 * [xyz.erik.opi.ime.ImeState.backToLetters]。
 *
 * 配色真源是 [SkinOption.palette]（与 fcitx5 侧主题同一套设计语言）；本文件
 * 不持有任何颜色字面量。
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun SkinPanel(
    selected: SkinOption,
    onSelect: (SkinOption) -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val p = LocalKeyboardPalette.current
    Column(modifier = modifier.fillMaxSize().background(p.panelBg)) {
        PanelHeader("皮肤", "选中即换；「不改动」= 跟随暗光/长辈这些模式", onClose)
        // FlowRow 不用 Row：窄屏折行；外层可滚（返回箭头独占一行后高度增加）。
        Column(
            modifier = Modifier.fillMaxWidth().weight(1f)
                .verticalScroll(rememberScrollState()).padding(horizontal = 12.dp),
        ) {
            FlowRow(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                for (skin in SkinOption.entries) {
                    SkinCard(skin, skin == selected) { onSelect(skin) }
                }
            }
        }
    }
}

/** 一张皮肤卡：小键盘样张 + 名字。选中 = 强调色描边 + 强调色粗体名。 */
@Composable
private fun SkinCard(skin: SkinOption, selected: Boolean, onClick: () -> Unit) {
    val pal = skin.palette
    val p = LocalKeyboardPalette.current
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = Modifier
            .width(60.dp)
            .padding(top = 6.dp)
            .clip(RoundedCornerShape(8.dp))
            .clickable(onClick = onClick)
            .semantics {
                role = Role.RadioButton
                contentDescription = "皮肤 ${skin.label}${if (selected) "，已选中" else ""}"
            }
            .padding(4.dp),
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(42.dp)
                .background(pal.barBg)
                .border(
                    width = if (selected) 2.dp else 1.dp,
                    color = if (selected) p.accent else p.barDimFg.copy(alpha = 0.35f),
                    shape = RoundedCornerShape(6.dp),
                ),
            contentAlignment = Alignment.Center,
        ) {
            MiniKeyboard(pal)
        }
        Spacer(Modifier.height(4.dp))
        Text(
            skin.label,
            fontSize = 11.sp,
            color = if (selected) p.accent else p.panelFg,
            fontWeight = if (selected) FontWeight.Bold else FontWeight.Normal,
            maxLines = 1,
        )
    }
}

/** 小键盘样张：三行键面，第二行第三键是强调键（对应键盘上的高亮位）。 */
@Composable
private fun MiniKeyboard(pal: KeyboardPalette) {
    Column(
        modifier = Modifier.padding(horizontal = 6.dp, vertical = 5.dp),
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        for (row in 0 until 3) {
            Row(horizontalArrangement = Arrangement.spacedBy(2.dp)) {
                for (col in 0 until 4) {
                    Box(
                        Modifier
                            .weight(1f)
                            .height(5.dp)
                            .clip(RoundedCornerShape(1.5.dp))
                            .background(if (row == 1 && col == 2) pal.keyHighlightBg else pal.keyBg),
                    )
                }
            }
        }
    }
}
