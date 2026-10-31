// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 皮肤选择器（2026-10-31 用户裁决「动态背景就是皮肤」）：19 张动态主题卡直接画
//! 该主题的缩小插画 + 自带配色；同日又按用户要求并回 11 款纯色皮肤，默认清爽。
//! 纯色卡只换配色、关掉插画；动态卡插画与配色一起换。选任一卡都同时写
//! `Background=` 与 `Skin=`（纯色的 `Background=` 为空）。
//!
//! 2026-10-30 从 SettingsPages.kt 拆出（加背景段后 509 行撞 500 行门禁）。

package xyz.erik.opi.candidate

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
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xyz.erik.opi.scene.ScenePalette
import xyz.erik.opi.scene.SceneTheme
import xyz.erik.opi.scene.drawOpiSceneBackdrop

private val CardShape = RoundedCornerShape(4.dp)

/** 纯色在上、动态在下，各自按 FlowRow 折行；选中卡 = 青边 + 青字。 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun SceneSkinPicker(
    selectedScene: SceneTheme?,
    selectedSkin: SkinOption,
    onSelectScene: (SceneTheme) -> Unit,
    onSelectSkin: (SkinOption) -> Unit,
) {
    PickerLabel("纯色")
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        for (skin in SOLID_SKINS) {
            SolidSkinCard(skin, selectedScene == null && skin == selectedSkin) {
                onSelectSkin(skin)
            }
        }
    }
    Spacer(Modifier.height(10.dp))
    PickerLabel("动态")
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        for (theme in SceneTheme.entries) {
            SceneSkinCard(theme, theme == selectedScene) { onSelectScene(theme) }
        }
    }
}

@Composable
private fun PickerLabel(text: String) {
    Text(
        text,
        modifier = Modifier.padding(bottom = 4.dp),
        fontSize = 12.sp,
        fontWeight = FontWeight.Bold,
        color = Muted,
    )
}

/** 纯色卡：皮肤底色 + 三枚键帽示意。 */
@Composable
private fun SolidSkinCard(skin: SkinOption, selected: Boolean, onClick: () -> Unit) {
    val colors = skinColors(skin)
    SkinCardFrame(skin.value, selected, onClick) {
        Box(
            Modifier.fillMaxWidth().height(40.dp).clip(CardShape).background(colors.bg).border(
                width = if (selected) 2.dp else 1.dp,
                color = if (selected) Teal else Hairline,
            ),
        ) {
            Box(
                Modifier.align(Alignment.TopStart).padding(4.dp).size(6.dp)
                    .clip(CircleShape).background(colors.accent),
            )
            Row(
                Modifier.align(Alignment.BottomCenter).padding(4.dp),
                horizontalArrangement = Arrangement.spacedBy(3.dp),
            ) {
                repeat(3) {
                    Box(
                        Modifier.weight(1f).height(12.dp)
                            .clip(RoundedCornerShape(2.dp)).background(colors.hover),
                    )
                }
            }
        }
    }
}

/** 动态卡：该主题的缩小插画（用主题自带配色画）+ 主题名。 */
@Composable
private fun SceneSkinCard(theme: SceneTheme, selected: Boolean, onClick: () -> Unit) {
    val colors = skinColors(theme.defaultSkin())
    val scene = ScenePalette(base = colors.bg, ink = colors.fg, accent = colors.accent)
    SkinCardFrame(theme.label, selected, onClick) {
        Canvas(
            modifier = Modifier.fillMaxWidth().height(40.dp)
                .background(colors.bg)
                .border(
                    width = if (selected) 2.dp else 1.dp,
                    color = if (selected) Teal else Hairline,
                ),
        ) {
            drawOpiSceneBackdrop(scene, theme, phase = 0f, muted = 0.20f)
        }
    }
}

@Composable
private fun SkinCardFrame(
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    preview: @Composable () -> Unit,
) {
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = Modifier.width(70.dp).clickable(onClick = onClick).padding(2.dp),
    ) {
        preview()
        Spacer(Modifier.height(5.dp))
        Text(
            label,
            fontSize = 12.sp,
            color = if (selected) Teal else Ink,
            fontWeight = if (selected) FontWeight.Bold else FontWeight.Normal,
        )
    }
}
