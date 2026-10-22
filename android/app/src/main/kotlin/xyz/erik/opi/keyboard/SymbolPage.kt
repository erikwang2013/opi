// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * 符号页（2026-10-09 用户裁决：「9 键，左下角符按钮点进去应该是个新的页面，右L布局，
 * 主要是常用、中文、英文、网络、数字、角标、序号、音标、平假、片假、箭头、特殊、拼音、
 * 注音、竖标、部首、俄文、希文、拉丁、制表、土音、藏文等功能标点、标签、符号，前面数字
 * 排列，滑动翻页显示」；2026-10-10 订正：「分类功能在最右侧而不是底部」→ 类目条移到**底部**）。
 *
 * 布局落法：**上方大区** = 当前类目的符号网格（横向滑动翻页，页大小 [SymbolCategories.PAGE_SIZE]）；
 * **底部横条** = 类目表，每项 `序号.名称`（可横滑，选中强调色粗体）。点符号即上屏
 * （走 [onCommit]，与生僻字面板同一条通路）。
 *
 * 数据来自宿主懒读的资产（失败 → 空列表 → 显示「没有类目数据」，不崩 IME）。
 */
@Composable
fun SymbolPage(
    categories: List<SymbolCategories.Category>,
    onCommit: (String) -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val p = LocalKeyboardPalette.current
    var selected by remember { mutableStateOf(0) }
    Column(modifier = modifier.fillMaxSize().background(p.panelBg)) {
        // 头：与皮肤/输入模式/键盘模式面板同形态
        Row(
            modifier = Modifier.fillMaxWidth().padding(start = 14.dp, end = 4.dp, top = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text("符号", fontSize = 15.sp, fontWeight = FontWeight.Bold, color = p.panelFg)
            Spacer(Modifier.width(10.dp))
            // 提示占弹性空间（原 Spacer 的位置）：320dp + 系统大字号时先省略提示，
            // 右侧关闭键不被挤出；常规字号下文字仍从左侧起排、观感不变。
            Text(
                "点符号上屏；左右滑动翻页",
                modifier = Modifier.weight(1f),
                fontSize = 11.sp,
                color = p.barDimFg,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            IconButton(onClick = onClose) {
                Icon(OpiIcons.Close, contentDescription = "关闭符号页", tint = p.toolbarFg)
            }
        }
        Row(modifier = Modifier.weight(1f).fillMaxWidth()) {
            val cat = categories.getOrNull(selected)
            if (cat == null) {
                Box(
                    modifier = Modifier.weight(1f).fillMaxHeight(),
                    contentAlignment = Alignment.Center,
                ) {
                    Text("没有类目数据", fontSize = 14.sp, color = p.barDimFg)
                }
            } else {
                val pages =
                    (cat.symbols.size + SymbolCategories.PAGE_SIZE - 1) / SymbolCategories.PAGE_SIZE
                val pager = rememberPagerState { pages }
                // 换类目回第 1 页（否则停在上一个类目的页码上；页数还可能变少而越界）
                LaunchedEffect(selected) { pager.scrollToPage(0) }
                HorizontalPager(
                    state = pager,
                    modifier = Modifier.weight(1f).fillMaxHeight(),
                ) { page ->
                    SymbolGridPage(cat.symbols, page, onCommit)
                }
            }
        }
        // 底部：类目条（2026-10-10 用户订正「分类在最右侧而不是底部」→ 横排到底部；
        // 选中 = 强调色粗体；22 项放不下一页宽 ⇒ 可横滑）
        Row(
            modifier = Modifier.fillMaxWidth().height(42.dp)
                .background(p.barBg)
                .horizontalScroll(rememberScrollState()),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            for ((i, c) in categories.withIndex()) {
                val on = i == selected
                Text(
                    "${c.index}.${c.label}",
                    modifier = Modifier
                        .clickable { selected = i }
                        .semantics {
                            role = Role.RadioButton
                            contentDescription =
                                "符号类目 ${c.index}.${c.label}${if (on) "，已选中" else ""}"
                        }
                        .padding(horizontal = 10.dp, vertical = 8.dp),
                    fontSize = 13.sp,
                    fontWeight = if (on) FontWeight.Bold else FontWeight.Normal,
                    color = if (on) p.accent else p.panelFg,
                    maxLines = 1,
                )
            }
        }
    }
}

/**
 * 一页符号网格：8 列 × 4 行（[SymbolCategories.PAGE_SIZE]）。单字符 22sp、多字符 token
 * （网络表情等）收小到 14sp 并允许省略号。末行/末页补空格子 —— 网格高度稳定，翻页不跳。
 */
@Composable
private fun SymbolGridPage(all: List<String>, page: Int, onPick: (String) -> Unit) {
    val p = LocalKeyboardPalette.current
    val start = page * SymbolCategories.PAGE_SIZE
    val cells = all.subList(start, minOf(start + SymbolCategories.PAGE_SIZE, all.size))
    Column(modifier = Modifier.fillMaxSize().padding(horizontal = 4.dp, vertical = 4.dp)) {
        for (rowStart in cells.indices step 8) {
            Row(modifier = Modifier.weight(1f).fillMaxWidth()) {
                for (i in rowStart until minOf(rowStart + 8, cells.size)) {
                    val s = cells[i]
                    KeyBox(
                        modifier = Modifier.weight(1f),
                        description = "符号 $s",
                        onTap = { onPick(s) },
                    ) {
                        Text(
                            s,
                            fontSize = if (s.length > 1) 14.sp else 22.sp,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            color = p.keyFg,
                        )
                    }
                }
                for (i in minOf(rowStart + 8, cells.size) until rowStart + 8) {
                    Box(Modifier.weight(1f))
                }
            }
        }
        for (r in (cells.size + 7) / 8 until 4) {
            Row(modifier = Modifier.weight(1f).fillMaxWidth()) {}
        }
    }
}
