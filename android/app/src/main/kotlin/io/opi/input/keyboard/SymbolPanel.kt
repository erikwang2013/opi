package io.opi.input.keyboard

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

enum class SymbolTab { Common, Emoji, All }

/**
 * 符号面板：常用/表情/全部 Tab + 关键字搜索（对齐 flutter symbol_panel.dart）。
 * 搜索输入焦点联动 qwerty 叠盘（IME 窗口内 TextField 不会唤起系统键盘）；
 * 防抖在 ImeState（250ms），本组件只按 searchQuery 出结果。
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun SymbolPanel(
    catalog: SymbolCatalog,
    searchText: String,
    searchActive: Boolean,
    searchQuery: String,
    onSearchText: (String) -> Unit,
    onSearchFocus: (Boolean) -> Unit,
    onCommit: (String) -> Unit,
    onClose: () -> Unit,
    onBackToNumber: () -> Unit,
) {
    var tab by remember { mutableStateOf(SymbolTab.Common) }

    fun commit(text: String) {
        if (SymbolCatalog.isEmoji(text)) catalog.recordRecent(text)
        onCommit(text)
    }

    Column(modifier = Modifier.fillMaxSize()) {
        // 头部：ABC / 123 + 搜索框
        Row(modifier = Modifier.fillMaxWidth().height(48.dp)) {
            KeyButton("ABC", modifier = Modifier.weight(2f), onTap = onClose)
            KeyButton("123", modifier = Modifier.weight(2f), onTap = onBackToNumber)
            Box(modifier = Modifier.weight(5f).padding(horizontal = 4.dp)) {
                TextField(
                    value = searchText,
                    onValueChange = onSearchText,
                    modifier = Modifier.fillMaxSize().onFocusChanged { onSearchFocus(it.isFocused) },
                    singleLine = true,
                    textStyle = TextStyle(fontSize = 14.sp),
                    placeholder = { Text("搜索（拼音/英文）", fontSize = 14.sp, color = Color(0xFF9E9E9E)) },
                    colors = TextFieldDefaults.colors(
                        focusedContainerColor = Color.White,
                        unfocusedContainerColor = Color.White,
                        focusedIndicatorColor = Color.Transparent,
                        unfocusedIndicatorColor = Color.Transparent,
                    ),
                )
            }
        }
        // Tab 栏
        Row(modifier = Modifier.fillMaxWidth()) {
            for (t in SymbolTab.entries) {
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .height(36.dp)
                        .background(if (t == tab) Color(0xFFE0E0E0) else Color.Transparent)
                        .clickable { tab = t },
                    contentAlignment = Alignment.Center,
                ) {
                    Text(tabLabel(t), fontSize = 14.sp)
                }
            }
        }
        // 内容区：搜索激活且有关键字 → 结果网格（表情 Tab 过滤）；否则按 Tab 出内容
        Box(modifier = Modifier.weight(1f).fillMaxWidth()) {
            if (searchActive && searchQuery.isNotEmpty()) {
                val results = catalog.search(searchQuery)
                val shown = if (tab == SymbolTab.Emoji) results.filter(SymbolCatalog::isEmoji) else results
                SymbolGrid(shown, ::commit)
            } else {
                when (tab) {
                    SymbolTab.Common -> SymbolGrid(catalog.common, ::commit)
                    SymbolTab.Emoji -> Column(modifier = Modifier.fillMaxSize()) {
                        if (catalog.recents.isNotEmpty()) RecentsRow(catalog.recents, onCommit)
                        Box(modifier = Modifier.weight(1f).fillMaxWidth()) {
                            SymbolGrid(catalog.emoji, ::commit)
                        }
                    }
                    SymbolTab.All -> SymbolGrid(catalog.all, ::commit)
                }
            }
        }
    }
}

private fun tabLabel(t: SymbolTab): String = when (t) {
    SymbolTab.Common -> "常用"
    SymbolTab.Emoji -> "表情"
    SymbolTab.All -> "全部"
}

/** 最近使用表情行。 */
@Composable
private fun RecentsRow(recents: List<String>, onCommit: (String) -> Unit) {
    LazyRow(modifier = Modifier.fillMaxWidth().height(44.dp)) {
        itemsIndexed(recents) { _, text ->
            Box(
                modifier = Modifier.fillMaxSize().clickable { onCommit(text) },
                contentAlignment = Alignment.Center,
            ) {
                Text(text, fontSize = 22.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

/** 8 列符号网格（惰性构建：全量条目数千，count 构建不展开）。 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun SymbolGrid(entries: List<String>, onTap: (String) -> Unit) {
    LazyVerticalGrid(columns = GridCells.Fixed(8), modifier = Modifier.fillMaxSize()) {
        items(entries.size) { i ->
            val text = entries[i]
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(2.dp)
                    .clip(RoundedCornerShape(6.dp))
                    .clickable { onTap(text) },
                contentAlignment = Alignment.Center,
            ) {
                Text(text, fontSize = 20.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}
