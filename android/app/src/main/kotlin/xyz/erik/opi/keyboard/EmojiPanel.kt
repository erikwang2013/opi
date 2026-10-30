// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
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
import androidx.compose.foundation.rememberScrollState
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
 *   分类条点一下**切成该组**（只画该组条目，[groupSlice]）；高亮 = 选中的组。
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
    val scope = rememberCoroutineScope()
    // 分类条是「切片选择器」：选中第几组就只画第几组（2026-10-30 用户报障修复）。
    var emojiGroup by remember { mutableStateOf(0) }
    var kaomojiGroup by remember { mutableStateOf(0) }
    val groups = remember(sub, catalog.emojiGroups, kaomoji) {
        emojiCategoryGroups(sub, catalog.emojiGroups, kaomoji)
    }
    // 引擎重载后组数可能变少：内容与高亮共用同一个夹紧后的下标。
    val selectedGroup = (if (sub == EmojiSubTab.Emoji) emojiGroup else kaomojiGroup)
        .coerceIn(0, (groups.size - 1).coerceAtLeast(0))

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
                        minCell = if (sub == EmojiSubTab.Emoji) 40.dp else 112.dp,
                        fontSize = if (sub == EmojiSubTab.Emoji) 20.sp else 14.sp,
                    )
                }
            } else {
                when (sub) {
                    EmojiSubTab.Pet -> PetStickerPanel(onPickPet)
                    EmojiSubTab.Emoji ->
                        EmojiBrowse(catalog, groups, selectedGroup, emojiGrid, onCommit)
                    EmojiSubTab.Kaomoji ->
                        KaomojiBrowse(kaomoji, groups, selectedGroup, kaomojiGrid, onCommit)
                }
            }
        }
        if (!searching && groups.isNotEmpty()) {
            CategoryBar(
                groups = groups,
                selected = selectedGroup,
                onSelect = { i ->
                    val same = i == selectedGroup
                    if (sub == EmojiSubTab.Emoji) emojiGroup = i else kaomojiGroup = i
                    // 同组再点：状态赋同值不触发重组，LaunchedEffect(group) 不会重跑
                    // ⇒ 网格停在原处；直接滚回组顶（换组仍由 LaunchedEffect 处理）。
                    if (same) scope.launch {
                        if (sub == EmojiSubTab.Emoji) emojiGrid.scrollToItem(0)
                        else kaomojiGrid.scrollToItem(0)
                    }
                },
            )
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
    Row(modifier = Modifier.fillMaxWidth().height(52.dp)) {
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
            // 横滑：320dp + 系统字体放大 / 三个 Tab（自有/Emoji/颜文字）时总宽超过
            // 剩余空间，不滑动就会把右侧 🔍 挤出屏幕（或裁掉第三个 Tab）。
            Row(
                modifier = Modifier.weight(1f).horizontalScroll(rememberScrollState())
                    .selectableGroup(),
                horizontalArrangement = Arrangement.Center,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                for (t in tabs) {
                    val on = t == sub
                    Box(
                        modifier = Modifier
                            .height(44.dp)
                            .clip(RoundedCornerShape(6.dp))
                            .selectable(selected = on, role = Role.Tab, onClick = { onSub(t) })
                            .padding(horizontal = 12.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = t.label,
                            fontSize = 15.sp,
                            fontWeight = if (on) FontWeight.Bold else FontWeight.Normal,
                            color = if (on) p.accent else p.barDimFg,
                        )
                    }
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

/** Emoji 子页：最近使用 + 选中分类的表情网格（**不含自有**，自有是独立子页）。 */
@Composable
private fun EmojiBrowse(
    catalog: SymbolCatalog,
    groups: List<SymbolCatalog.EmojiGroup>,
    group: Int,
    grid: LazyGridState,
    onCommit: (String) -> Unit,
) {
    val all = catalog.emoji
    val emojis = remember(all, groups, group) { groupSlice(all, groups, group) }
    Column(modifier = Modifier.fillMaxSize()) {
        if (catalog.recents.isNotEmpty()) RecentsRow(catalog.recents, onCommit)
        Box(modifier = Modifier.weight(1f).fillMaxWidth()) {
            if (all.isNotEmpty() && emojis.isEmpty()) {
                // 整表非空、选中组被字形过滤摘空（如整组都是本机字体画不出的新表情）：
                // 画一个空网格像坏了，给出原因。（整表为空 = 数据未就绪，保持原样不提示。）
                Text(
                    "该分类暂无可用表情",
                    modifier = Modifier.align(Alignment.Center),
                    fontSize = 14.sp,
                    color = LocalKeyboardPalette.current.barDimFg,
                )
            } else {
                // 换组就回到顶部：上一组的滚动偏移留给新切片会停在半截。
                LaunchedEffect(group) { if (emojis.isNotEmpty()) grid.scrollToItem(0) }
                // Adaptive 而非 Fixed(8)：320dp 手机仍是 8 列；平板/横屏自动多排几列，
                // 否则每格 160dp、emoji 悬在大片空白中间。
                LazyVerticalGrid(
                    columns = GridCells.Adaptive(40.dp),
                    state = grid,
                    modifier = Modifier.fillMaxSize(),
                ) {
                    items(emojis.size) { SymbolCell(emojis[it], onCommit) }
                }
            }
        }
    }
}

/** 颜文字子页：数据缺失时显示空态（不崩 IME，理由同符号页的「没有类目数据」）。 */
@Composable
private fun KaomojiBrowse(
    categories: List<KaomojiCategories.Category>,
    groups: List<SymbolCatalog.EmojiGroup>,
    group: Int,
    grid: LazyGridState,
    onCommit: (String) -> Unit,
) {
    val all = remember(categories) { KaomojiCategories.flatten(categories) }
    val shown = remember(all, groups, group) { groupSlice(all, groups, group) }
    if (shown.isEmpty()) {
        Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text(
                if (all.isEmpty()) "没有颜文字数据" else "该分类暂无颜文字",
                fontSize = 14.sp,
                color = LocalKeyboardPalette.current.barDimFg,
            )
        }
    } else {
        // 换分类回到顶部（同 EmojiBrowse）。
        LaunchedEffect(group) { if (shown.isNotEmpty()) grid.scrollToItem(0) }
        // 颜文字比单个 emoji 宽：最小格 112dp（手机仍是 3 列、320dp 落 2 列）+ 小字号，
        // 少截断；平板/横屏才排更多列。
        SymbolGrid(shown, onCommit, grid, minCell = 112.dp, fontSize = 14.sp)
    }
}

/**
 * 底部分类条（可横滑；选中 = 强调色粗体 + 药丸底）。
 *
 * 点一项 = **换切片**，网格只画该组。这里不再用 `scrollToItem(组.start)`：组的 start
 * 不保证落在行首，滚过去首行仍是上一组的尾巴，高亮反推也会把上一组点亮
 * （2026-10-30 用户报障「点后一个分类显示前一个」）。同鸿蒙 EmojiPad 的既有做法。
 */
@Composable
private fun CategoryBar(
    groups: List<SymbolCatalog.EmojiGroup>,
    selected: Int,
    onSelect: (Int) -> Unit,
) {
    val p = LocalKeyboardPalette.current
    LazyRow(
        modifier = Modifier.fillMaxWidth().height(52.dp).background(p.barBg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        itemsIndexed(groups) { i, g ->
            val on = i == selected
            Box(
                modifier = Modifier
                    .padding(horizontal = 3.dp)
                    .height(44.dp)
                    .clip(RoundedCornerShape(8.dp))
                    .background(if (on) p.keyBg else Color.Transparent)
                    .clickable { onSelect(i) }
                    .padding(horizontal = 12.dp)
                    .semantics {
                        role = Role.RadioButton
                        contentDescription = "分类 ${g.name}${if (on) "，已选中" else ""}"
                    },
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = g.name,
                    fontSize = 14.sp,
                    fontWeight = if (on) FontWeight.Bold else FontWeight.Normal,
                    color = if (on) p.accent else p.panelFg,
                    maxLines = 1,
                )
            }
        }
    }
}
