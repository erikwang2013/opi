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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch
import xyz.erik.opi.pet.PetMood
import xyz.erik.opi.settings.ImePreferences

/**
 * 表情页（工具条「表情」入口）。布局按用户参考图：
 *
 * ```
 * [← 返回]   [自有 | Emoji | 颜文字]   [🔍]
 * ┌────────────── 网格 ──────────────┐
 * └──── 可横滑的分类条（底部一条）────┘
 * ```
 *
 * - **自有**（2026-10-18 用户裁决的独立顶级分类）：12 个小欧形态（PNG 表情图），
 *   点击**插入图片**（[onPickPet]，见 `OpiImeService.commitPetSticker`；编辑器不支持时
 *   回退文本标签 `[小欧·笑]`）；不进 Emoji 分类条，也不进最近使用。
 * - **Emoji**：引擎 `emojiGroups()` 的 9 组（组名来自 OpenMoji 数据，不写死），
 *   分类条与网格一一对齐（`start` 直接是 `catalog.emoji` 下标）。
 * - **颜文字**：`assets/kaomoji.tsv` 第二列（解析见 [KaomojiCategories]）。
 * - 默认进 **Emoji**（工具条入口 [xyz.erik.opi.keyboard.SymbolTab.Emoji] 的既有行为）；
 *   分类条点一下把网格滚到该组首项（`scrollToItem(start)`）；高亮 = 网格当前首个
 *   可见项所属的组（滚动时跟着走）。两页共用 [SymbolCatalog.groupIndexOf] 这一套。
 * - 搜索态保持既有两套工具条的裁决：`[← 返回][搜索框]`，框一聚焦宿主就在面板下方
 *   叠字母盘（焦点事件走 `onSearchFocus`）；防抖在 ImeState。返回 = 清查询回浏览态。
 *   自有是纯图片页（没有可搜的文本），不给 🔍。
 */
@Composable
internal fun EmojiPanel(
    catalog: SymbolCatalog,
    kaomojiCategories: () -> List<KaomojiCategories.Category>,
    searchText: String,
    searchActive: Boolean,
    searchQuery: String,
    onSearchText: (String) -> Unit,
    onSearchFocus: (Boolean) -> Unit,
    onCommit: (String) -> Unit,
    onPickPet: (PetMood) -> Unit,
    onClose: () -> Unit,
) {
    // 「自有」是否出现由设置页开关决定（缺省隐藏）；面板打开时读一次即可。
    val context = LocalContext.current
    val showOwn = remember { ImePreferences.from(context).showOwn }
    val tabs = remember(showOwn) { emojiSubTabs(showOwn) }
    var sub by remember { mutableStateOf(EmojiSubTab.Emoji) }
    // 本地点 🔍 的意图（渲染哪一套工具条）；真焦点仍由 onSearchFocus 上报（叠盘靠它）。
    var searchWanted by remember { mutableStateOf(false) }
    val searching = searchWanted || searchActive
    val focus = remember { FocusRequester() }
    val focusManager = LocalFocusManager.current
    // 资产懒读一次（只在表情页真正挂载时取；缺失 → 空列表，颜文字页显示空态）。
    val kaomoji = remember { kaomojiCategories() }
    val emojiGrid = rememberLazyGridState()
    val kaomojiGrid = rememberLazyGridState()

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

    Column(modifier = Modifier.fillMaxSize()) {
        EmojiTopBar(
            sub = sub,
            tabs = tabs,
            onSub = { sub = it },
            searching = searching,
            searchText = searchText,
            onSearchText = onSearchText,
            onSearchFocus = onSearchFocus,
            focus = focus,
            onBack = { back() },
            onStartSearch = { searchWanted = true },
        )
        Box(modifier = Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
            if (searching && searchQuery.isNotEmpty()) {
                val shown = when (sub) {
                    // 自有页没有 🔍 入口；这里只是 when 穷尽（真到不了）。
                    EmojiSubTab.Pet -> emptyList()
                    EmojiSubTab.Emoji -> remember(searchQuery) {
                        catalog.search(searchQuery).filter(catalog::isEmoji).distinct()
                    }
                    EmojiSubTab.Kaomoji -> remember(searchQuery, kaomoji) {
                        KaomojiCategories.flatten(kaomoji)
                            .filter { it.contains(searchQuery, ignoreCase = true) }
                            .distinct()
                    }
                }
                if (shown.isEmpty()) {
                    NoMatchRow()
                } else {
                    // 搜索结果沿用各子页的格宽/字号（颜文字比 emoji 宽，8 列会截成省略号）。
                    SymbolGrid(
                        shown,
                        onCommit,
                        columns = if (sub == EmojiSubTab.Emoji) 8 else 3,
                        fontSize = if (sub == EmojiSubTab.Emoji) 20.sp else 14.sp,
                    )
                }
            } else {
                when (sub) {
                    EmojiSubTab.Pet -> PetStickerPanel(onPickPet)
                    EmojiSubTab.Emoji -> EmojiBrowse(catalog, emojiGrid, onCommit)
                    EmojiSubTab.Kaomoji -> KaomojiBrowse(kaomoji, kaomojiGrid, onCommit)
                }
            }
        }
        if (!searching) {
            val groups = emojiCategoryGroups(sub, catalog.emojiGroups, kaomoji)
            if (groups.isNotEmpty()) {
                CategoryBar(
                    groups = groups,
                    grid = if (sub == EmojiSubTab.Emoji) emojiGrid else kaomojiGrid,
                )
            }
        }
    }
}

/**
 * 浏览态工具条 `[← 返回][自有 | Emoji | 颜文字][🔍]`；搜索态 `[← 返回][搜索框]`。
 *
 * ⚠️ 搜索框**不常驻**：它是「点 🔍 才出现」的那一态（2026-10-12 用户裁决的两套工具条）。
 */
@Composable
private fun EmojiTopBar(
    sub: EmojiSubTab,
    tabs: List<EmojiSubTab>,
    onSub: (EmojiSubTab) -> Unit,
    searching: Boolean,
    searchText: String,
    onSearchText: (String) -> Unit,
    onSearchFocus: (Boolean) -> Unit,
    focus: FocusRequester,
    onBack: () -> Unit,
    onStartSearch: () -> Unit,
) {
    val p = LocalKeyboardPalette.current
    Row(modifier = Modifier.fillMaxWidth().height(48.dp)) {
        IconButton(onClick = onBack) {
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
            // 子页切换：读屏用 Role.Tab 播报「已选择」（选中态不只看颜色）。
            Row(
                modifier = Modifier.weight(1f).selectableGroup(),
                horizontalArrangement = Arrangement.Center,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                for (t in tabs) {
                    val on = t == sub
                    Text(
                        text = t.label,
                        modifier = Modifier
                            .clip(RoundedCornerShape(6.dp))
                            .selectable(selected = on, role = Role.Tab, onClick = { onSub(t) })
                            .padding(horizontal = 14.dp, vertical = 6.dp),
                        fontSize = 15.sp,
                        fontWeight = if (on) FontWeight.Bold else FontWeight.Normal,
                        color = if (on) p.accent else p.barDimFg,
                    )
                }
            }
            // 自有只有图片，没有可搜索的文本 —— 给了 🔍 也只会落到「无匹配」。
            if (sub != EmojiSubTab.Pet) {
                IconButton(onClick = onStartSearch) {
                    Icon(OpiIcons.Search, contentDescription = "搜索", tint = p.toolbarFg)
                }
            }
        }
    }
}

/** Emoji 子页：最近使用 + 引擎表情网格（**不含自有**，自有是独立子页）。 */
@Composable
private fun EmojiBrowse(catalog: SymbolCatalog, grid: LazyGridState, onCommit: (String) -> Unit) {
    val emojis = catalog.emoji
    Column(modifier = Modifier.fillMaxSize()) {
        if (catalog.recents.isNotEmpty()) RecentsRow(catalog.recents, onCommit)
        Box(modifier = Modifier.weight(1f).fillMaxWidth()) {
            LazyVerticalGrid(
                columns = GridCells.Fixed(8),
                state = grid,
                modifier = Modifier.fillMaxSize(),
            ) {
                items(emojis.size) { SymbolCell(emojis[it], onCommit) }
            }
        }
    }
}

/** 颜文字子页：数据缺失时显示空态（不崩 IME，理由同符号页的「没有类目数据」）。 */
@Composable
private fun KaomojiBrowse(
    categories: List<KaomojiCategories.Category>,
    grid: LazyGridState,
    onCommit: (String) -> Unit,
) {
    val all = remember(categories) { KaomojiCategories.flatten(categories) }
    if (all.isEmpty()) {
        Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text("没有颜文字数据", fontSize = 14.sp, color = LocalKeyboardPalette.current.barDimFg)
        }
    } else {
        // 颜文字比单个 emoji 宽：3 列 + 小字号，少截断。
        SymbolGrid(all, onCommit, grid, columns = 3, fontSize = 14.sp)
    }
}

/**
 * 底部分类条（可横滑；选中 = 强调色粗体 + 药丸底）。
 *
 * 高亮反推**只在本组件内读** `grid.firstVisibleItemIndex` —— 读在面板根上会让整个
 * 网格跟着滚动重组。点一项把网格滚到该组首项。
 */
@Composable
private fun CategoryBar(groups: List<SymbolCatalog.EmojiGroup>, grid: LazyGridState) {
    val p = LocalKeyboardPalette.current
    val scope = rememberCoroutineScope()
    val selected = SymbolCatalog.groupIndexOf(groups, grid.firstVisibleItemIndex)
    LazyRow(
        modifier = Modifier.fillMaxWidth().height(44.dp).background(p.barBg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        itemsIndexed(groups) { i, g ->
            val on = i == selected
            Text(
                text = g.name,
                modifier = Modifier
                    .padding(horizontal = 4.dp)
                    .clip(RoundedCornerShape(8.dp))
                    .background(if (on) p.keyBg else Color.Transparent)
                    .clickable { scope.launch { grid.scrollToItem(g.start) } }
                    .padding(horizontal = 10.dp, vertical = 7.dp)
                    .semantics {
                        role = Role.RadioButton
                        contentDescription = "分类 ${g.name}${if (on) "，已选中" else ""}"
                    },
                fontSize = 13.sp,
                fontWeight = if (on) FontWeight.Bold else FontWeight.Normal,
                color = if (on) p.accent else p.panelFg,
                maxLines = 1,
            )
        }
    }
}
