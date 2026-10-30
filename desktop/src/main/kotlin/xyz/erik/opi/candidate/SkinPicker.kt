// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 皮肤选择器（2026-10-30 从 SettingsPages.kt 拆出：加「动态背景」段后那边 509 行
//! 撞 500 行门禁；同包 internal 引用不变）。

package xyz.erik.opi.candidate

import androidx.compose.foundation.ExperimentalFoundationApi
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
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

// ---------- 皮肤选择器（2026-10-09：可视卡，替掉纯文字清单）----------

/**
 * 一款皮肤的预览色（bg/fg/accent 三项）。
 * ⚠️ 十款的色值**不再各写一份**（2026-10-12）：统一取自 `SkinPalette.skinColors`
 * —— 那份的逐值对应由 SettingsTest 的 `skin palettes match the theme generator`
 * 现读现比（同一个「图与实」门禁，只是比对对象挪去了 SkinPalette.kt）。
 */
private data class SkinPreview(val bg: Color, val fg: Color, val accent: Color)

private fun previewOf(skin: SkinOption): SkinPreview = when (skin) {
    // 「不改动」不指定主题：预览画一浅一深的双色底（表意「跟随系统」）—— 这是预览
    // 专属的中性表示，**不是** SkinPalette.KeepColors（那个是状态栏的固定深色）。
    SkinOption.KEEP -> SkinPreview(Color(0xFFF2F2F2), Color(0xFF888888), Teal)
    else -> skinColors(skin).let { SkinPreview(it.bg, it.fg, it.accent) }
}

/**
 * 十张卡按 FlowRow 折行排开（内容宽 800-180-22×2 ≈ 570dp：一行 7×70 + 6×8 = 538 ≤ 570，
 * 十款固定两行）。窄了仍会折行（首版 76/10=420 > 416 时朱砂掉到第二行，截图复核时
 * 抓到过半宽问题本身）。
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun SkinPicker(selected: SkinOption, onSelect: (SkinOption) -> Unit) {
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        for (skin in SkinOption.entries) {
            SkinCard(skin, skin == selected) { onSelect(skin) }
        }
    }
}

/** 一张皮肤卡：候选面板样张（底色 + 样字 + 朱砂/青的强调条）+ 名字。选中 = 青边 + 青字。 */
@Composable
private fun SkinCard(skin: SkinOption, selected: Boolean, onClick: () -> Unit) {
    val pv = previewOf(skin)
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = Modifier.width(70.dp).clickable(onClick = onClick).padding(2.dp),
    ) {
        Box(
            modifier = Modifier.fillMaxWidth().height(40.dp)
                .background(pv.bg)
                .border(
                    width = if (selected) 2.dp else 1.dp,
                    color = if (selected) Teal else Hairline,
                ),
            contentAlignment = Alignment.Center,
        ) {
            if (skin == SkinOption.KEEP) {
                // 「跟随系统」：一浅一深的双色底（不写字，免得误读成某个具体主题）
                Row(Modifier.fillMaxSize()) {
                    Box(Modifier.weight(1f).fillMaxHeight().background(Color(0xFFF2F2F2)))
                    Box(Modifier.weight(1f).fillMaxHeight().background(Color(0xFF3A3A3A)))
                }
            } else {
                Text("你", fontSize = 17.sp, color = pv.fg)
            }
            // 强调条贴底：每款的 accent，命中面板的选中下划线
            Box(
                Modifier.align(Alignment.BottomCenter).fillMaxWidth(0.7f).height(3.dp)
                    .background(pv.accent),
            )
        }
        Spacer(Modifier.height(5.dp))
        Text(
            skin.value,
            fontSize = 12.sp,
            color = if (selected) Teal else Ink,
            fontWeight = if (selected) FontWeight.Bold else FontWeight.Normal,
        )
    }
}
