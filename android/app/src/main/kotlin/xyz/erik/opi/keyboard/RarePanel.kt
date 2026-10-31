// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xyz.erik.opi.ime.opiPanelBackground

/**
 * 生僻字键盘（对齐主流输入法：**选部首 → 选笔画数 → 挑字**的浏览式查法）。
 *
 * 数据来自 [RareIndex]（Unihan 部首 + 总笔画数索引，覆盖扩展区），**不经过引擎**：
 * 面板直接提交字形（`onCommit`）。索引由调用方在**首次打开时**读资产并缓存
 * （1.1MB，别放服务启动路径）。
 *
 * 两级筛选按「先部首、后笔画」——与纸质字典的检索顺序一致；顶部始终有一条
 * 面包屑（部首 → 笔画）可回退。
 */
@Composable
fun RarePanel(
    index: Map<Int, Map<Int, List<String>>>,
    onCommit: (String) -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val p = LocalKeyboardPalette.current
    var radical by remember { mutableStateOf<Int?>(null) }
    var strokes by remember { mutableStateOf<Int?>(null) }

    Column(modifier = modifier.fillMaxSize().opiPanelBackground(p.panelBg)) {
        // 面包屑/标题行
        Row(
            // heightIn(min)：长标题/大字号时行可长高，不把 14sp 面包屑上下裁掉。
            modifier = Modifier.fillMaxWidth().heightIn(min = 40.dp).padding(horizontal = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            val title = when {
                radical == null -> "生僻字 · 选部首"
                strokes == null -> "生僻字 · ${RareIndex.radicalGlyph(radical!!)}（部首 $radical）· 选笔画数"
                else -> "生僻字 · 部首 $radical · $strokes 画"
            }
            Text(
                title,
                fontSize = 14.sp,
                color = p.panelFg,
                modifier = Modifier.weight(1f),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            // 回退：笔画 → 部首；部首 → 关闭由 onClose 管
            if (radical != null) {
                Text(
                    "‹ 返回",
                    fontSize = 14.sp,
                    color = p.accent,
                    modifier = Modifier
                        .clip(RoundedCornerShape(6.dp))
                        .clickable {
                            if (strokes != null) strokes = null else radical = null
                        }
                        .padding(horizontal = 8.dp, vertical = 4.dp),
                )
            }
            Text(
                "收起",
                fontSize = 14.sp,
                color = p.accent,
                modifier = Modifier
                    .clip(RoundedCornerShape(6.dp))
                    .clickable(onClick = onClose)
                    .padding(horizontal = 8.dp, vertical = 4.dp),
            )
        }
        when {
            // Adaptive(40dp)：手机 320dp 仍是 8 列，平板/横屏自动多排（Fixed(8) 在
            // 800dp 平板上每格 100dp，部首字悬在空框中间）。
            radical == null -> LazyVerticalGrid(
                columns = GridCells.Adaptive(40.dp),
                modifier = Modifier.weight(1f).fillMaxWidth(),
            ) {
                items((1..RareIndex.MAX_RADICAL).toList()) { r ->
                    CellText(
                        text = RareIndex.radicalGlyph(r),
                        description = "部首 $r",
                        onClick = { radical = r; strokes = null },
                    )
                }
            }
            strokes == null -> {
                val counts = RareIndex.strokeCounts(index, radical!!)
                if (counts.isEmpty()) {
                    // 该部首在索引里没有字（理论不可达 —— 索引由 kRSUnicode 生成；
                    // 资产被替换时可能遇到）→ 明说而不是空网格
                    Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                        Text("该部首没有索引数据", color = p.barDimFg)
                    }
                } else {
                    LazyRow(modifier = Modifier.weight(1f).fillMaxWidth()) {
                        items(counts) { s ->
                            Box(
                                modifier = Modifier
                                    .padding(6.dp)
                                    .clip(RoundedCornerShape(8.dp))
                                    .background(p.keyBg)
                                    .clickable { strokes = s }
                                    .padding(horizontal = 16.dp, vertical = 12.dp),
                            ) {
                                Text("$s 画", fontSize = 16.sp, color = p.keyFg)
                            }
                        }
                    }
                }
            }
            else -> {
                val chars = RareIndex.chars(index, radical!!, strokes!!)
                LazyVerticalGrid(
                    columns = GridCells.Adaptive(40.dp),
                    modifier = Modifier.weight(1f).fillMaxWidth(),
                    verticalArrangement = Arrangement.Top,
                ) {
                    items(chars) { ch ->
                        CellText(
                            text = ch,
                            description = "生僻字 $ch",
                            onClick = { onCommit(ch) },
                        )
                    }
                }
            }
        }
    }
}

/** 网格单元：部首/字共用一个观感的可点格子。 */
@Composable
private fun CellText(text: String, description: String, onClick: () -> Unit) {
    Box(
        modifier = Modifier
            .padding(2.dp)
            .clip(RoundedCornerShape(6.dp))
            .background(LocalKeyboardPalette.current.keyBg)
            .clickable(onClick = onClick)
            .semantics {
                role = Role.Button
                contentDescription = description
            }
            .height(44.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, fontSize = 20.sp, color = LocalKeyboardPalette.current.keyFg)
    }
}
