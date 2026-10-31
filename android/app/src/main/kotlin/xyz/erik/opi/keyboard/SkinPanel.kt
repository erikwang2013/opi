// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 皮肤面板（2026-10-09 用户裁决「移动端没有皮肤设置」；2026-10-31 起**动态背景
//! 就是皮肤**：19 张主题卡，每张卡直接画该主题的缩小插画 + 它自带的配色）。
//! 2026-10-31 晚些又按用户要求把原来的 11 款纯色皮肤并回同一张列表，默认「清爽」：
//! 纯色卡只换配色、关掉插画；动态卡插画与配色一起换。
//!
//! 面板**不自动关**：皮肤要连换着比才顺手；左上角返回箭头走
//! [xyz.erik.opi.ime.ImeState.backToLetters]。

package xyz.erik.opi.keyboard

import androidx.compose.foundation.Canvas
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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
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
import xyz.erik.opi.ime.PanelHeader
import xyz.erik.opi.scene.ScenePalette
import xyz.erik.opi.scene.SceneTheme
import xyz.erik.opi.scene.drawOpiSceneBackdrop

private val CardShape = RoundedCornerShape(6.dp)

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun SkinPanel(
    selectedScene: SceneTheme?,
    selectedSkin: SkinOption,
    onSelectScene: (SceneTheme) -> Unit,
    onSelectSkin: (SkinOption) -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val p = LocalKeyboardPalette.current
    Column(modifier = modifier.fillMaxSize().background(p.panelBg)) {
        PanelHeader("皮肤", "11 款纯色 + 19 套动态；默认清爽，选中即换", onClose)
        Column(
            modifier = Modifier.fillMaxWidth().weight(1f)
                .verticalScroll(rememberScrollState()).padding(horizontal = 12.dp),
        ) {
            SectionLabel("纯色")
            FlowRow(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                for (skin in SOLID_SKINS) {
                    SolidSkinCard(skin, selectedScene == null && skin == selectedSkin) {
                        onSelectSkin(skin)
                    }
                }
            }
            SectionLabel("动态")
            FlowRow(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                for (theme in SceneTheme.entries) {
                    SceneCard(theme, theme == selectedScene) { onSelectScene(theme) }
                }
            }
        }
    }
}

@Composable
private fun SectionLabel(text: String) {
    Text(
        text,
        modifier = Modifier.padding(top = 10.dp, bottom = 2.dp),
        fontSize = 12.sp,
        fontWeight = FontWeight.Bold,
        color = LocalKeyboardPalette.current.barDimFg,
    )
}

/** 一张纯色皮肤卡：色板底色 + 三枚键帽示意，选中的卡加青边。 */
@Composable
private fun SolidSkinCard(skin: SkinOption, selected: Boolean, onClick: () -> Unit) {
    val p = LocalKeyboardPalette.current
    val pal = skin.palette
    SkinCardFrame(skin.label, selected, onClick) {
        Box(
            Modifier.fillMaxWidth().height(42.dp).clip(CardShape).background(pal.barBg).border(
                width = if (selected) 2.dp else 1.dp,
                color = if (selected) p.accent else p.barDimFg.copy(alpha = 0.35f),
                shape = CardShape,
            ),
        ) {
            Box(
                Modifier.align(Alignment.TopStart).padding(5.dp).size(7.dp)
                    .clip(CircleShape).background(pal.accent),
            )
            Row(
                Modifier.align(Alignment.BottomCenter).padding(5.dp),
                horizontalArrangement = Arrangement.spacedBy(3.dp),
            ) {
                repeat(3) {
                    Box(
                        Modifier.weight(1f).height(13.dp)
                            .clip(RoundedCornerShape(2.dp)).background(pal.keyBg),
                    )
                }
            }
        }
    }
}

/** 一张动态皮肤卡：直接画该主题的缩小版插画（静态相位），配色取主题自带皮肤。 */
@Composable
private fun SceneCard(theme: SceneTheme, selected: Boolean, onClick: () -> Unit) {
    val p = LocalKeyboardPalette.current
    val pal = remember(theme) { theme.defaultSkin().palette }
    val scene = remember(pal) {
        ScenePalette(base = pal.barBg, ink = pal.keyFg, accent = pal.accent)
    }
    SkinCardFrame(theme.label, selected, onClick) {
        Canvas(
            modifier = Modifier.fillMaxWidth().height(42.dp).clip(CardShape)
                .background(pal.barBg)
                .border(
                    width = if (selected) 2.dp else 1.dp,
                    color = if (selected) p.accent else p.barDimFg.copy(alpha = 0.35f),
                    shape = CardShape,
                ),
        ) {
            drawOpiSceneBackdrop(scene, theme, phase = 0f, muted = 0.20f)
        }
    }
}

/** 卡片外壳：宽度、点击、读屏标签与名称都由这里统一。 */
@Composable
private fun SkinCardFrame(
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    preview: @Composable () -> Unit,
) {
    val p = LocalKeyboardPalette.current
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = Modifier
            .width(84.dp)
            .padding(top = 6.dp)
            .clip(RoundedCornerShape(8.dp))
            .clickable(onClick = onClick)
            .semantics {
                role = Role.RadioButton
                contentDescription = "皮肤 $label${if (selected) "，已选中" else ""}"
            }
            .padding(4.dp),
    ) {
        preview()
        Spacer(Modifier.height(4.dp))
        Text(
            label,
            fontSize = 11.sp,
            color = if (selected) p.accent else p.panelFg,
            fontWeight = if (selected) FontWeight.Bold else FontWeight.Normal,
            maxLines = 1,
        )
    }
}
