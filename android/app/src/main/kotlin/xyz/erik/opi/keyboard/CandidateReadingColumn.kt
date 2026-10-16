// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.TextUnit

/**
 * **候选拼音读数列**（2026-10-16 从 `NineKeyKeyboard` 抽出）：输入中 9 键左列的
 * 「读数态」整块 —— 一个读音一行、整列可滑、点行 = 钉读音。
 *
 * 抽出的理由：平铺候选视图（`CandidateTiledView`）要求**同一套语义的第二处**（放大的
 * 行）—— 点击行为、读屏串、滚动复位（`scrollKey`）、封顶格数都必须逐字同构，抄第二份
 * 就是漂移的充分条件。9 键的空闲态（标点列）仍留在 `NineKeyKeyboard`：平铺态只在
 * 输入中出现，没有空闲态。
 *
 * 语义（2026-10-09 用户裁决「选择后候选词跟着拼音变化，不要直接确认」；2026-10-16
 * 裁决收紧为**读音恰等**）：点一行拼音 = 钉住该读音、**不上屏**；同一行再点 = 解除。
 */
@Composable
internal fun CandidateReadingColumn(
    readings: List<String>,
    pinnedReading: String,
    onPick: (Int) -> Unit,
    /** 读数列的滚动复位键（传缓冲：缓冲一变整列回顶部，别停在旧位置）。 */
    scrollKey: String,
    /** 与标点列同一节奏的格数（9 键 = `nineKeyStripPuncts.size`）。 */
    cells: Int,
    fontSize: TextUnit,
    modifier: Modifier = Modifier,
) {
    BoxWithConstraints(
        modifier = modifier.semantics {
            // 念前几条就够（全量可能几十条）；逐条与候选的对应靠候选
            // 自己的语义节点，这里只报「读数列有货」。
            contentDescription = "候选拼音 " + readings.take(4).joinToString("、") +
                if (readings.size > 4) "等" else ""
        },
    ) {
        val cell = maxHeight / cells
        Column(
            // 新一批读数（缓冲变了）整列回顶部 —— 否则继续输入后视口
            // 停在旧位置，与「当前读音」对不上（scrollKey 由宿主给缓冲）。
            modifier = Modifier.fillMaxSize()
                .verticalScroll(remember(scrollKey) { ScrollState(0) }),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            for ((i, r) in readings.withIndex()) {
                val pinned = r == pinnedReading && pinnedReading.isNotEmpty()
                Box(
                    // 点一行拼音 = **钉住该读音**：候选收窄到读音**恰等**的条目、不上屏；
                    // 同一行再点 = 解除（见本文件头注的两条裁决）。
                    modifier = Modifier.fillMaxWidth().height(cell)
                        .clickable { onPick(i) }
                        .semantics {
                            role = Role.Button
                            contentDescription = "第 ${i + 1} 项拼音 $r" +
                                if (pinned) "，已选中，点击取消" else "，点击只看这一读音"
                        },
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        r,
                        fontSize = fontSize,
                        color = if (pinned) {
                            LocalKeyboardPalette.current.accent
                        } else {
                            LocalKeyboardPalette.current.barDimFg
                        },
                        fontWeight = if (pinned) FontWeight.Bold else FontWeight.Normal,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        textAlign = TextAlign.Center,
                    )
                }
            }
        }
    }
}
