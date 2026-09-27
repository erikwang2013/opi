// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 候选窗 UI：Compose 自绘（无 material3 依赖，见 build.gradle.kts 注释）；
//! 读 CandidateModel 渲染，交互回调交给 PipeServer 发消息（线协议全貌见
//! Main.kt 头部注释）。

package io.opi.candidate

import androidx.compose.foundation.background
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
import androidx.compose.foundation.layout.width
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.opi.input.pet.OpiPet
import io.opi.input.pet.PetMood
import io.opi.input.pet.PetPalette

// ---------- 自绘配色（无 material3 依赖，见 build.gradle.kts 注释） ----------

private val BgColor = Color(0xFF2D2D2D)
private val TextColor = Color(0xFFE8E8E8)
private val DimColor = Color(0xFF9E9E9E)
private val AccentColor = Color(0xFF4C8DFF)
private val BtnColor = Color(0xFF3A3A3A)

/** 单页候选上限（与 Rust 侧 logic.rs 的 PAGE_SIZE 一致）。 */
private const val MAX_CANDIDATES = 8

// ---------- Compose UI ----------

@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun CandidatePanel(
    model: CandidateModel,
    onSelect: (Int) -> Unit,
    onNext: () -> Unit,
    onPrev: () -> Unit,
) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .background(BgColor)
            .padding(horizontal = 12.dp, vertical = 8.dp),
    ) {
        Column {
            // 首行：缓冲 + 模式标签
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    model.buffer,
                    fontSize = 16.sp,
                    fontWeight = FontWeight.Bold,
                    color = TextColor,
                )
                Spacer(Modifier.width(8.dp))
                Text(modeLabel(model.mode), fontSize = 12.sp, color = DimColor)
            }
            Spacer(Modifier.height(6.dp))
            if (model.candidates.isEmpty()) {
                // 无候选：小欧出来摊手，替掉一片空白（与 Android 候选栏同一处理）。
                Row(verticalAlignment = Alignment.CenterVertically) {
                    OpiPet(
                        mood = PetMood.PUZZLED,
                        palette = PetPalette.Dark,
                        size = 34.dp,
                    )
                    Spacer(Modifier.width(8.dp))
                    Text("无匹配", fontSize = 13.sp, color = DimColor)
                }
            } else {
                // 候选列表（最多 8 个/页；点击 → TSF 提交）
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(16.dp),
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    model.candidates.take(MAX_CANDIDATES).forEachIndexed { index, text ->
                        CandidateItem(index + 1, text) { onSelect(index) }
                    }
                }
            }
            Spacer(Modifier.height(4.dp))
            // 底行：页码 + 翻页。
            // 两个箭头只发 next_page/prev_page 消息、**不改本地页码**：页码的唯一真源是
            // TSF 回发的 show（TSF 侧已接线，见 Main.kt 头注释「翻页」段）。enabled 只是
            // 禁用态提示，不参与置页 —— 点了箭头要等下一帧 show 回来才算翻页。
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                PageButton("‹", enabled = model.page > 1, onClick = onPrev)
                Text(
                    "${model.page}/${model.pageCount}",
                    fontSize = 12.sp,
                    color = DimColor,
                )
                PageButton("›", enabled = model.page < model.pageCount, onClick = onNext)
            }
        }
    }
}

@Composable
private fun CandidateItem(index: Int, text: String, onClick: () -> Unit) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.clickable(onClick = onClick),
    ) {
        Text("$index.", fontSize = 13.sp, color = DimColor)
        Text(text, fontSize = 17.sp, color = TextColor)
    }
}

/** 翻页按钮：无 material3 的 TextButton 平替。 */
@Composable
private fun PageButton(text: String, enabled: Boolean, onClick: () -> Unit) {
    val color = if (enabled) AccentColor else DimColor
    Box(
        modifier = Modifier
            .background(BtnColor)
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 14.dp, vertical = 2.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, fontSize = 16.sp, color = color)
    }
}

/**
 * 模式 → 中文标签（协议字符串见 `candidate_io.rs` 的 `mode_str`）。
 *
 * `else -> "拼音"` 是**载荷分支**，不是兜底美化：Rust 侧有意把 `Mode::Traditional`
 * 也编码成 `"pinyin"`（`mode_str` 的注释：候选窗无简繁概念，新增 `"traditional"`
 * 只会造成协议漂移），所以繁体态落到这里显示「拼音」是两端约定好的行为。
 * 改这个 else 之前先读 `mode_str` 的注释 —— 改坏了两端都不报错，只会静默显示错标签。
 *
 * `internal` 而非 `private`：让 WireContractTest 钉住上面这条跨轨约定。
 */
internal fun modeLabel(mode: String): String = when (mode) {
    "english" -> "英文"
    "number" -> "数字"
    "symbol" -> "符号"
    else -> "拼音"
}
