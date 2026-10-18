// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

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
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
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
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xyz.erik.opi.pet.OpiPet
import xyz.erik.opi.pet.PetMood

/**
 * 符号入口的起始页。[Common]/[All] 是符号页的两个 Tab；[Emoji] 只是**入口标记**
 * （工具条表情图标），由 [SymbolPanel] 直接导向 [EmojiPanel]，不出现在符号页 Tab 里。
 */
enum class SymbolTab { Common, Emoji, All }

/** 符号页自身的 Tab（表情入口不经过本页）。 */
private enum class SymbolPageTab { Common, All }

/**
 * 符号面板：常用/全部 Tab + 关键字搜索（对齐 flutter symbol_panel.dart）。
 * 搜索输入焦点联动 qwerty 叠盘（IME 窗口内 TextField 不会唤起系统键盘）；
 * 防抖在 ImeState（250ms），本组件只按 searchQuery 出结果。
 *
 * 表情入口（`initialTab == SymbolTab.Emoji`）走独立一页 [EmojiPanel]：顶部
 * `[← 返回][自有 | Emoji | 颜文字][🔍]` + 中间网格 + **底部**可横滑分类条
 * （2026-10-18 用户裁决：自有独立成顶级子页、点击插入图片）。符号页只有
 * 常用/全部 两个 Tab，不再保留旧表情网格。
 */
@Composable
fun SymbolPanel(
    catalog: SymbolCatalog,
    searchText: String,
    searchActive: Boolean,
    searchQuery: String,
    onSearchText: (String) -> Unit,
    onSearchFocus: (Boolean) -> Unit,
    onCommit: (String) -> Unit,
    /** 点「自有」宠物形态：走图片插入通道，不经过 [onCommit]（文本）与最近使用。 */
    onPickPet: (PetMood) -> Unit,
    onClose: () -> Unit,
    onBackToNumber: () -> Unit,
    /** 起始 Tab：普通入口=常用，工具条的表情图标直达 [SymbolTab.Emoji]。 */
    initialTab: SymbolTab = SymbolTab.Common,
    /** 颜文字分类（懒读 `assets/kaomoji.tsv`）：只在表情页挂载时取。 */
    kaomojiCategories: () -> List<KaomojiCategories.Category> = { emptyList() },
) {
    fun commit(text: String) {
        if (catalog.isEmoji(text)) catalog.recordRecent(text)
        onCommit(text)
    }

    if (initialTab == SymbolTab.Emoji) {
        EmojiPanel(
            catalog = catalog,
            kaomojiCategories = kaomojiCategories,
            searchText = searchText,
            searchActive = searchActive,
            searchQuery = searchQuery,
            onSearchText = onSearchText,
            onSearchFocus = onSearchFocus,
            onCommit = { commit(it) },
            onPickPet = onPickPet,
            onClose = onClose,
        )
    } else {
        SymbolTabsPage(
            catalog = catalog,
            searchText = searchText,
            searchActive = searchActive,
            searchQuery = searchQuery,
            onSearchText = onSearchText,
            onSearchFocus = onSearchFocus,
            onCommit = { commit(it) },
            onClose = onClose,
            onBackToNumber = onBackToNumber,
            initialTab = if (initialTab == SymbolTab.All) SymbolPageTab.All else SymbolPageTab.Common,
        )
    }
}

/** 符号页（ABC/123 + 常用/全部）：起始 Tab 由调用方定，本页内可切。 */
@Composable
private fun SymbolTabsPage(
    catalog: SymbolCatalog,
    searchText: String,
    searchActive: Boolean,
    searchQuery: String,
    onSearchText: (String) -> Unit,
    onSearchFocus: (Boolean) -> Unit,
    onCommit: (String) -> Unit,
    onClose: () -> Unit,
    onBackToNumber: () -> Unit,
    initialTab: SymbolPageTab,
) {
    val p = LocalKeyboardPalette.current
    // 起始 Tab 由调用方定；换入口时重放（remember 键）
    var tab by remember(initialTab) { mutableStateOf(initialTab) }

    Column(modifier = Modifier.fillMaxSize()) {
        // 头部：ABC / 123 + 搜索框
        Row(modifier = Modifier.fillMaxWidth().height(48.dp)) {
            KeyButton("ABC", modifier = Modifier.weight(2f), onTap = onClose)
            KeyButton("123", modifier = Modifier.weight(2f), onTap = onBackToNumber)
            Box(modifier = Modifier.weight(5f).padding(horizontal = 4.dp)) {
                TextField(
                    value = searchText,
                    onValueChange = onSearchText,
                    modifier = Modifier.fillMaxSize()
                        .onFocusChanged { onSearchFocus(it.isFocused) },
                    singleLine = true,
                    textStyle = TextStyle(fontSize = 14.sp),
                    placeholder = {
                        Text("搜索（拼音/英文）", fontSize = 14.sp, color = p.barDimFg)
                    },
                    colors = TextFieldDefaults.colors(
                        focusedContainerColor = p.panelBg,
                        unfocusedContainerColor = p.panelBg,
                        focusedIndicatorColor = Color.Transparent,
                        unfocusedIndicatorColor = Color.Transparent,
                    ),
                )
            }
        }
        // Tab 栏。选中态原本只靠背景色 —— 读屏读不出当前 Tab，色觉障碍也分不清；
        // selectable + Role.Tab 让读屏播报「已选择/未选择」（子 Text 会被合并成节点名）。
        Row(modifier = Modifier.fillMaxWidth().selectableGroup()) {
            for (t in SymbolPageTab.entries) {
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .height(36.dp)
                        .background(if (t == tab) p.keyBg else Color.Transparent)
                        .selectable(selected = t == tab, role = Role.Tab, onClick = { tab = t }),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(if (t == SymbolPageTab.Common) "常用" else "全部", fontSize = 14.sp)
                }
            }
        }
        // 内容区：搜索激活且有关键字 → 结果网格；否则按 Tab 出内容
        // contentAlignment 只对下面的空结果态起作用（结果网格是 fillMaxSize）
        Box(modifier = Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
            if (searchActive && searchQuery.isNotEmpty()) {
                // FFI（JNI）按查询缓存：只在 query 变化时重新搜索，避免每次重组都打引擎
                val results = remember(searchQuery) { catalog.search(searchQuery) }
                // 渲染前**保序去重**（用户裁决 2026-10-12「表情存在重复的，去重」）。
                // 数据/引擎/搜索三层各自都已有去重（`emoji_coverage` 的零重复门禁、
                // `search_filtered` 按 text 去重），这里是最后一道 —— 无论上游哪层
                // 将来漏了，屏幕上一格一样的东西不会再出现。
                val shown = remember(results) { results.distinct() }
                if (shown.isEmpty()) NoMatchRow() else SymbolGrid(shown, onCommit)
            } else {
                when (tab) {
                    SymbolPageTab.Common -> SymbolGrid(catalog.common, onCommit)
                    SymbolPageTab.All -> SymbolGrid(catalog.all, onCommit)
                }
            }
        }
    }
}

/**
 * 搜索无结果：小欧摊手 + 一句白话（符号搜索 / 表情 / 颜文字共用）。
 *
 * ⚠️ 这里**有意**写死 `PUZZLED`，**不走 `petMood`** —— 别去「修」成与
 * CandidateBar.kt 一致：那个「无匹配」是**词典**的直接产物（降级时几乎每次
 * 输入都落到那里），所以它走真实 `petMood`；而符号搜索查的是**符号表**，
 * 与 luna.opid **无因果关系**，词库降级根本不是这次「无匹配」的成因 ——
 * 接上去就是让宠物**假报**「引擎坏了」。两处看着不一致，是**判据不同**。
 */
@Composable
internal fun NoMatchRow() {
    val p = LocalKeyboardPalette.current
    Row(verticalAlignment = Alignment.CenterVertically) {
        OpiPet(mood = PetMood.PUZZLED, size = 30.dp)
        Text(
            "无匹配",
            modifier = Modifier.padding(start = 8.dp),
            fontSize = 13.sp,
            color = p.barDimFg,
        )
    }
}

/** 最近使用表情行。 */
@Composable
internal fun RecentsRow(recents: List<String>, onCommit: (String) -> Unit) {
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

/** 一格文字符号（emoji / 符号 / 颜文字共用）。 */
@Composable
internal fun SymbolCell(text: String, onTap: (String) -> Unit, fontSize: TextUnit = 20.sp) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .padding(2.dp)
            .clip(RoundedCornerShape(6.dp))
            .clickable { onTap(text) },
        contentAlignment = Alignment.Center,
    ) {
        Text(text, fontSize = fontSize, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

/**
 * 惰性符号网格（全量条目数千，count 构建不展开）。
 *
 * @param columns 列数（emoji/符号 8 列；颜文字更宽，传 3）。
 * @param fontSize 条目字号（多字符 token 传小一号，少截断）。
 */
@Composable
internal fun SymbolGrid(
    entries: List<String>,
    onTap: (String) -> Unit,
    state: LazyGridState = rememberLazyGridState(),
    columns: Int = 8,
    fontSize: TextUnit = 20.sp,
) {
    LazyVerticalGrid(
        columns = GridCells.Fixed(columns),
        state = state,
        modifier = Modifier.fillMaxSize(),
    ) {
        items(entries.size) { i -> SymbolCell(entries[i], onTap, fontSize) }
    }
}
