// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 符号页右侧类目条（2026-11-07 从 SymbolsPage.kt 拆出：加搜索框焦点交还后
//! 那个文件 514 行，撞 500 行门禁）。纯搬移，行为一字未动。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

internal const val STRIP_W = 96

/** 右竖条的「表情」项 = 特殊下标（`.tsv` 类目从 0 起）。 */
internal const val TAB_EMOJI = -1

/** 右：类目条（首项 = 「表情」页，其余 `序号.名称`；可竖滑；选中 = 强调色粗体）。 */
@Composable
internal fun CategoryStrip(
    categories: List<SymbolCategories.Category>,
    selected: Int,
    emojiSelected: Boolean,
    onSelect: (Int) -> Unit,
) {
    Column(
        modifier = Modifier.width(STRIP_W.dp).fillMaxHeight()
            .background(SymStrip)
            .verticalScroll(rememberScrollState()),
    ) {
        EmojiStripEntry(emojiSelected) { onSelect(TAB_EMOJI) }
        for ((i, c) in categories.withIndex()) {
            val on = i == selected
            Text(
                "${c.index}.${c.label}",
                modifier = Modifier.fillMaxWidth()
                    .clickable { onSelect(i) }
                    .semantics { contentDescription = "符号类目 ${c.index}.${c.label}${if (on) "，已选中" else ""}" }
                    .padding(horizontal = 10.dp, vertical = 9.dp),
                fontSize = 14.sp,
                fontWeight = if (on) FontWeight.Bold else FontWeight.Normal,
                color = if (on) SymAccent else SymFg,
                maxLines = 1,
            )
        }
    }
}
