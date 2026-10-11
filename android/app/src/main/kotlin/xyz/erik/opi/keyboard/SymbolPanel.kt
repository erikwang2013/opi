// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

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
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch
import xyz.erik.opi.pet.OpiPet
import xyz.erik.opi.pet.PetMood

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
    /** 起始 Tab：普通入口=常用，工具条的表情图标直达 [SymbolTab.Emoji]。 */
    initialTab: SymbolTab = SymbolTab.Common,
) {
    val p = LocalKeyboardPalette.current
    // 起始 Tab 由调用方定（工具条的表情入口直达 Emoji）；换入口时重放（remember 键）
    var tab by remember(initialTab) { mutableStateOf(initialTab) }

    fun commit(text: String) {
        if (catalog.isEmoji(text)) catalog.recordRecent(text)
        onCommit(text)
    }

    Column(modifier = Modifier.fillMaxSize()) {
        // 表情页（工具条「表情」入口）**有自己的两套工具条**（2026-10-12 用户裁决）：
        //   浏览态 `[← 返回][分类条][🔍]`；搜索态 `[← 返回][搜索框]` + 面板下方叠字母盘。
        // 符号页（符键入口）保持原样：ABC/123/搜索框 + 三个 Tab。
        val emojiMode = initialTab == SymbolTab.Emoji
        // 表情页网格的滚动状态：分类条点一下就 scrollToItem(组.start)，高亮按它反推。
        val grid = rememberLazyGridState()
        if (emojiMode) {
            EmojiTopBar(
                catalog = catalog,
                searchText = searchText,
                searchActive = searchActive,
                onSearchText = onSearchText,
                onSearchFocus = onSearchFocus,
                onClose = onClose,
                grid = grid,
            )
        } else {
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
                for (t in SymbolTab.entries) {
                    Box(
                        modifier = Modifier
                            .weight(1f)
                            .height(36.dp)
                            .background(if (t == tab) p.keyBg else Color.Transparent)
                            .selectable(selected = t == tab, role = Role.Tab, onClick = { tab = t }),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(tabLabel(t), fontSize = 14.sp)
                    }
                }
            }
        }
        // 内容区：搜索激活且有关键字 → 结果网格（表情 Tab 过滤）；否则按 Tab 出内容
        // contentAlignment 只对下面的空结果态起作用（网格/最近行都是 fillMaxSize）
        Box(modifier = Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
            if (searchActive && searchQuery.isNotEmpty()) {
                // FFI（JNI）按查询缓存：只在 query 变化时重新搜索，避免每次重组都打引擎
                val results = remember(searchQuery) { catalog.search(searchQuery) }
                val filtered = if (tab == SymbolTab.Emoji) results.filter(catalog::isEmoji) else results
                // 渲染前**保序去重**（用户裁决 2026-10-12「表情存在重复的，去重」）。
                // 数据/引擎/搜索三层各自都已有去重（`emoji_coverage` 的零重复门禁、
                // `search_filtered` 按 text 去重），这里是最后一道 —— 无论上游哪层
                // 将来漏了，屏幕上一格一样的东西不会再出现。
                val shown = remember(filtered) { filtered.distinct() }
                // 空结果此前是一片空白：用户分不清「搜过了没有」和「面板没反应」。
                // 同候选栏的「无匹配」——小欧摊手 + 一句白话。
                //
                // ⚠️ 这里**有意**写死 `PUZZLED`，**不走 `petMood`** —— 别去「修」成与
                // CandidateBar.kt 一致：那个「无匹配」是**词典**的直接产物（降级时几乎每次
                // 输入都落到那里），所以它走真实 `petMood`；而符号搜索查的是**符号表**，
                // 与 luna.opid **无因果关系**，词库降级根本不是这次「无匹配」的成因 ——
                // 接上去就是让宠物**假报**「引擎坏了」。两处看着不一致，是**判据不同**。
                if (shown.isEmpty()) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        OpiPet(mood = PetMood.PUZZLED, size = 30.dp)
                        Text(
                            "无匹配",
                            modifier = Modifier.padding(start = 8.dp),
                            fontSize = 13.sp,
                            color = p.barDimFg,
                        )
                    }
                } else {
                    SymbolGrid(shown, ::commit)
                }
            } else {
                when (tab) {
                    SymbolTab.Common -> SymbolGrid(catalog.common, ::commit)
                    SymbolTab.Emoji -> Column(modifier = Modifier.fillMaxSize()) {
                        if (catalog.recents.isNotEmpty()) RecentsRow(catalog.recents, ::commit)
                        Box(modifier = Modifier.weight(1f).fillMaxWidth()) {
                            // 分类条的 start 是**本列表**的下标 ⇒ 用 grid 这个状态滚
                            SymbolGrid(catalog.emoji, ::commit, grid)
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

/**
 * 表情页的两套工具条（2026-10-12 用户裁决「左返回 / 中表情分类 / 右搜索；点搜索跳转会
 * 带搜索框的键盘」）：
 *
 * - **浏览态** `[← 返回][分类条][🔍 搜索]`：返回 = 关面板回键盘；分类条可横滑，点一项把
 *   网格 `scrollToItem(组.start)`；高亮 = 网格**当前首个可见项**所属的组（滚动时跟着走
 *   —— `firstVisibleItemIndex` 是 Compose 状态，滚一下就重组）。🔍 = 进搜索态。
 * - **搜索态** `[← 返回][搜索框]`：返回 = 退回浏览态并清查询（网格回全量）；输入框一聚焦，
 *   宿主就在面板下方叠字母盘（既有机制 —— 焦点事件走 `onSearchFocus`）。
 *
 * ⚠️ 搜索框**不常驻**：它是「点 🔍 才出现」的那一态（用户裁决的原文就是两套工具条）。
 */
@Composable
private fun EmojiTopBar(
    catalog: SymbolCatalog,
    searchText: String,
    searchActive: Boolean,
    onSearchText: (String) -> Unit,
    onSearchFocus: (Boolean) -> Unit,
    onClose: () -> Unit,
    grid: LazyGridState,
) {
    val p = LocalKeyboardPalette.current
    val scope = rememberCoroutineScope()
    val focus = remember { FocusRequester() }
    val focusManager = LocalFocusManager.current
    // 本地点 🔍 的意图（渲染哪一套工具条）；真焦点仍由 onSearchFocus 上报（叠盘靠它）。
    var searchWanted by remember { mutableStateOf(false) }
    val searching = searchWanted || searchActive
    LaunchedEffect(searchWanted) { if (searchWanted) focus.requestFocus() }

    fun back() {
        if (searching) {
            searchWanted = false
            onSearchText("") // 退出搜索同时退出过滤，否则网格停在过滤结果上
            focusManager.clearFocus()
            onSearchFocus(false)
        } else {
            onClose()
        }
    }

    Row(modifier = Modifier.fillMaxWidth().height(48.dp)) {
        IconButton(onClick = ::back) {
            Icon(
                OpiIcons.ArrowBack,
                contentDescription = if (searching) "返回表情" else "关闭表情页",
                tint = p.toolbarFg,
            )
        }
        if (searching) {
            Box(modifier = Modifier.weight(1f).padding(end = 4.dp)) {
                TextField(
                    value = searchText,
                    onValueChange = onSearchText,
                    modifier = Modifier.fillMaxSize()
                        .focusRequester(focus)
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
        } else {
            val groups = catalog.emojiGroups
            val cur = SymbolCatalog.groupIndexOf(groups, grid.firstVisibleItemIndex)
            LazyRow(
                modifier = Modifier.weight(1f),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                itemsIndexed(groups) { i, g ->
                    val selected = i == cur
                    Text(
                        g.name,
                        fontSize = 14.sp,
                        color = if (selected) p.accent else p.barDimFg,
                        modifier = Modifier
                            .clip(RoundedCornerShape(6.dp))
                            .clickable { scope.launch { grid.scrollToItem(g.start) } }
                            .padding(horizontal = 10.dp, vertical = 6.dp)
                            .semantics { contentDescription = "分类 ${g.name}" },
                    )
                }
            }
            IconButton(onClick = { searchWanted = true }) {
                Icon(OpiIcons.Search, contentDescription = "搜索表情", tint = p.toolbarFg)
            }
        }
    }
}

/** 8 列符号网格（惰性构建：全量条目数千，count 构建不展开）。 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun SymbolGrid(
    entries: List<String>,
    onTap: (String) -> Unit,
    state: LazyGridState = rememberLazyGridState(),
) {
    LazyVerticalGrid(
        columns = GridCells.Fixed(8),
        state = state,
        modifier = Modifier.fillMaxSize(),
    ) {
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
