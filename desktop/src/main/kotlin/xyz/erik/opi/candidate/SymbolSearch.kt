// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 符号页/表情页的**搜索**（2026-10-13，对齐 Android 的表情搜索）：输入拼音或英语
//! 关键词 → 命中的条目进网格。数据来自 `opi-ctl search <关键词>`（D-Bus `Search(string)`
//! → 引擎 `Engine::search_symbols`；桌面进程够不着引擎，只能走 CLI —— 与 `insert` 同一先例）。
//!
//! 本文件三样东西：**搜索框**（自绘，页面同一套色）、**结局 → 显示态**的纯函数
//! （[searchView]，单测钉住）、**结果区**（同款翻页网格；没匹配/失败各一句话）。
//!
//! 失败原因**如实显示**，但落点与前缀提示行不同：这个窗平时用固定高的提示行
//! （SymbolWindow 的 notice）报一次性失败，而搜索失败是**跟着关键词持续存在**的状态
//! （用户还在打字，打一个字就问一次）—— 提示行 3.5s 会自己消失、也不绑定当前关键词
//! ⇒ 搜索失败画在结果区里，与「没有匹配」同一位置，看得见且随关键词更新。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * 搜索的显示态（[searchView] 的产物）。**界面只画它、不做判断** —— 四种结局在纯函数里
 * 分完类，才有测试钉得住「坏载荷不许画成『没匹配』」这类纪律。
 */
internal sealed interface SearchView {
    /** 还在等回音（防抖 + 一次进程往返）。**不是失败**，别报成失败（也别画成空结果）。 */
    object Pending : SearchView

    /** 通道/协议失败：`why` 原样展示（如实，不吞、不改写）。 */
    class Failed(val why: String) : SearchView

    /** 引擎回话了但没有命中。 */
    object None : SearchView

    /** 命中（保序）。 */
    class Hits(val texts: List<String>) : SearchView
}

/**
 * 搜索结局 → 显示态（**纯函数**）。
 *
 * 表情页要**再筛一道**：引擎的符号搜索不过滤表情（命中的是全表），而表情页只该显示
 * 表情里的命中 —— 判据用 [emojiSet]（`emoji.tsv` 成员集合，与引擎 `builtin_emoji_texts`
 * 同一份表、与 Android `SymbolCatalog.isEmoji` 同义；引擎的 Search 出口没有回「这条是不是
 * 表情」，所以在桌面本地筛）。
 *
 * 两页都还要过 [canRender]（字形覆盖）—— 与本页浏览态同一条纪律：搜出来的东西也不能是
 * 空心框（这正是这一轮要修的东西，搜索路径不豁免）。
 */
internal fun searchView(
    result: SymbolSearch?,
    emojiTab: Boolean,
    emojiSet: Set<String>,
    canRender: (String) -> Boolean = GlyphCoverage::canRender,
): SearchView = when (result) {
    null -> SearchView.Pending
    is SymbolSearch.Failed -> SearchView.Failed(result.why)
    is SymbolSearch.Hits -> {
        val hit = result.texts.filter { !emojiTab || it in emojiSet }.filter(canRender)
        if (hit.isEmpty()) SearchView.None else SearchView.Hits(hit)
    }
}

/** 搜索框（自绘：无边框输入 + 占位提示；样式与页面同一套色）。 */
@Composable
internal fun SymbolSearchField(
    query: String,
    onQuery: (String) -> Unit,
    onFocusChanged: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier.height(24.dp).background(SymStrip).padding(horizontal = 8.dp),
        contentAlignment = Alignment.CenterStart,
    ) {
        // 空时才画的占位符（不是后置文本 —— 它不该被当成能删的字符）。
        if (query.isEmpty()) {
            Text("搜索符号/表情（拼音或英文）", fontSize = 12.sp, color = SymDim, maxLines = 1)
        }
        BasicTextField(
            value = query,
            onValueChange = onQuery,
            modifier = Modifier.fillMaxWidth()
                .semantics { contentDescription = "搜索符号或表情" }
                .onFocusChanged { onFocusChanged(it.isFocused) },
            singleLine = true,
            textStyle = TextStyle(color = SymFg, fontSize = 12.sp),
            cursorBrush = SolidColor(SymAccent),
        )
    }
}

/** 结果区：命中画网格（同款翻页），其余三种态画一行字（没匹配/失败各是各的话）。 */
@Composable
internal fun SearchResultPane(
    view: SearchView,
    emoji: Boolean,
    emptyText: String,
    modifier: Modifier = Modifier,
    onPick: (String) -> Unit,
) {
    when (view) {
        SearchView.Pending -> StatusLine("搜索中…", SymDim, modifier)
        is SearchView.Failed -> StatusLine(view.why, SymErr, modifier)
        SearchView.None -> StatusLine(emptyText, SymDim, modifier)
        is SearchView.Hits -> HitGrid(view.texts, emoji, modifier, onPick)
    }
}

@Composable
private fun StatusLine(text: String, color: Color, modifier: Modifier) {
    Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Text(text, fontSize = 13.sp, color = color, modifier = Modifier.padding(horizontal = 12.dp))
    }
}

@Composable
private fun HitGrid(texts: List<String>, emoji: Boolean, modifier: Modifier, onPick: (String) -> Unit) {
    val pages = (texts.size + SymbolCategories.PAGE_SIZE - 1) / SymbolCategories.PAGE_SIZE
    // ⚠️ `rememberPagerState { pages }` 的 lambda 是**首次组合时捕获**的：换关键词后条数
    // 变了也不会重建（旧页数留着，甚至停在越界页）。key(texts) 强制换结果就重建 ——
    // 与浏览态那边 `LaunchedEffect(selected){pager.scrollToPage(0)}` 是同一类问题的两种解法
    // （那边页码可控，这边整个列表会换掉，重建更直接）。
    key(texts) {
        val pager = rememberPagerState { pages }
        HorizontalPager(state = pager, modifier = modifier) { page ->
            SymbolGridPage(texts, page, emoji = emoji, onPick = onPick)
        }
    }
}
