// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 符号页的「表情」页（2026-10-12 用户裁决：设置与大页面都要能选符号、表情）。
//!
//! 2026-10-18 起布局与 Android 的 `keyboard/EmojiPanel.kt` 对齐：
//! 顶部 `[← 返回] [自有 | Emoji | 颜文字] [搜索框]` + 中间网格 + **底部**可横滑分类条。
//!
//! - **自有** 分类 = 小欧的 [PetEmojiMap.forms]（与 Android 同一份映射），独立子页、
//!   4 列网格显示 12 张 PNG；点击 = 复制图片到剪贴板（见 `PetStickerPage.kt`）。
//!   用户裁决（2026-10-18）：自有是**独立顶级分类**，不再混进 Emoji 的分类条/网格。
//! - **Emoji** 分类 = `emoji.tsv` 的连续同名段（组名/组序来自数据，不写死）。
//! - **颜文字** 分类 = `kaomoji.tsv` 第二列（解析见 [KaomojiData]），6×6 网格。
//! - 分类条点一下把分页器滚到该组首项所在页；高亮 = 当前页首项所属的组
//!   （[EmojiData.groupIndexOf]），滑动翻页时自动跟随。
//!
//! 数据 = **构建期拷进资源的 `emoji.tsv` / `kaomoji.tsv`**（真源 `data/raw/`，见
//! `build.gradle.kts` 的 `copyEmojiData` / `copyKaomojiData`）—— 表情表与引擎
//! `builtin_emoji_texts()`/`builtin_emoji_groups()` 同一份表、同一顺序（组序 = 文件序）。
//! 列：`表情 \t 英文名 \t 中文组名`（第 2 列桌面不消费，与引擎同）。
//!
//! 分组口径**逐字对齐引擎**：第 3 列的**连续同名段**切组（`builtin_emoji_groups`）——
//! 别按名字去重排序（官方组序是文件序，组名重复出现要靠连续段区分）。
//!
//! 解析姿态与 [SymbolCategories.parse] 同：坏行跳过、整表读空就显示「没有表情数据」
//! （降级而不是打不开）。⚠️ 与引擎的「缺列即 panic」不同是**有意**的 —— 引擎那份是
//! 编译期嵌入（坏了=仓库坏），这份可能被换/被截断。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.PagerState
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch
import xyz.erik.opi.pet.PetEmojiMap

/** 表情表：OpenMoji 常用子集顺序 + 面板分组（`start`/`count` 与条目下标对齐，组序 = 文件序）。 */
object EmojiData {
    data class Group(val label: String, val start: Int, val count: Int)

    data class Data(val entries: List<String>, val groups: List<Group>)

    /** 空表：字形过滤没落地前（见 `deferredFilter`）与「全表被摘空」共用这一份。 */
    val Empty = Data(emptyList(), emptyList())

    /** 解析 `emoji.tsv`。连续同名段 = 一组（口径同引擎 `builtin_emoji_groups`）。 */
    fun parse(text: String): Data {
        val entries = ArrayList<String>()
        val groups = ArrayList<Group>()
        for (line in text.lineSequence()) {
            val cols = line.split('\t')
            val e = cols.getOrNull(0) ?: continue
            if (e.isEmpty()) continue
            val group = cols.getOrNull(2)?.takeIf { it.isNotEmpty() } ?: continue
            if (groups.lastOrNull()?.label == group) {
                val g = groups[groups.size - 1]
                groups[groups.size - 1] = g.copy(count = g.count + 1)
            } else {
                groups.add(Group(group, entries.size, 1))
            }
            entries.add(e)
        }
        return Data(entries, groups)
    }

    /**
     * 某组第一页的页码（分类条跳转用）。[pageSize] = 该页一页装多少项
     * （表情页 = `SymbolCategories.PAGE_SIZE`，颜文字页 = 6×6）。
     */
    fun pageOfGroup(g: Group, pageSize: Int = SymbolCategories.PAGE_SIZE): Int = g.start / pageSize

    /**
     * 当前页首项下标 → 组下标（高亮反推）。组按 start 升序，取最后一个
     * `start <= itemIndex`；空表 / 下标在首组之前 → `-1`（不亮）。
     *
     * 与 Android `SymbolCatalog.groupIndexOf` 同义：两端都靠这一支把「滚到哪了」
     * 折回「哪个分类亮」。
     */
    fun groupIndexOf(groups: List<Group>, itemIndex: Int): Int =
        groups.indexOfLast { it.start <= itemIndex }

    /**
     * 按「本机画不画得出」过滤条目，并**重算分组条的 `start`/`count`**（2026-10-13，
     * 口径对齐 Android `SymbolCatalog.emojiGroups`）。保序、幂等。
     *
     * 为什么必须重算：`start` 是**过滤后**列表里的下标（组条点击 = `pageOfGroup(g)`
     * 跳页），摘掉若干条后原下标整体偏前，不重算就跳到错的页；`count` 是组条文案里
     * 报的个数和读屏文案（「笑脸（N 个）」），不跟着减就会把摘掉的也算进去。
     *
     * 算法用**前缀存活数**（`aliveUpTo`）而不是逐组累加：组的边界是原下标，
     * 逐组累加会把「整组被摘空」这类情况算错。`start` 与 `count` 一起重算 ⇒
     * 被摘空的组退化成 `start = 上一组的 start`、`count = 0`。
     *
     * 摘空的组**保留在组条里**（与 Android 同）：组条是按官方组序的固定格位，
     * 格位随字体变化就没法靠位置记「第三个是手势」了。
     *
     * 全表被摘空 ⇒ 返回空 Data（走 EmojiPane 现有的「没有表情数据」降级），
     * 此时组条没有意义（没有条目可跳）。
     */
    fun filter(data: Data, canRender: (String) -> Boolean): Data {
        val aliveUpTo = IntArray(data.entries.size + 1)
        val entries = ArrayList<String>(data.entries.size)
        var alive = 0
        for (i in data.entries.indices) {
            aliveUpTo[i] = alive
            val e = data.entries[i]
            if (canRender(e)) {
                entries.add(e)
                alive++
            }
        }
        aliveUpTo[data.entries.size] = alive
        if (alive == data.entries.size) return data // 一条没摘 ⇒ 原样（也顺带保证了幂等）
        if (alive == 0) return Empty
        val groups = data.groups.map { g ->
            val from = g.start.coerceIn(0, data.entries.size)
            val to = (g.start + g.count).coerceIn(0, data.entries.size)
            g.copy(start = aliveUpTo[from], count = aliveUpTo[to] - aliveUpTo[from])
        }
        return Data(entries, groups)
    }
}

/** 读 classpath 资源 `/emoji.tsv`（构建期从 `data/raw/emoji.tsv` 拷入，见 build.gradle.kts）。 */
fun loadEmojiData(): EmojiData.Data {
    val text = EmojiData::class.java.getResourceAsStream("/emoji.tsv")
        ?.bufferedReader()?.use { it.readText() } ?: return EmojiData.Data(emptyList(), emptyList())
    return EmojiData.parse(text)
}

/** 表情页的三个子页（2026-10-18 用户裁决：自有是独立顶级分类）。 */
enum class EmojiSubTab(val label: String) {
    Own("自有"),
    Emoji("Emoji"),
    Kaomoji("颜文字"),
}

/**
 * 顶栏实际显示的子页：`showOwn` 缺省 false ⇒「自有」整个隐藏（2026-10-20 用户裁决）；
 * 设置页「表情中显示自有」打开后才加入。
 */
internal fun emojiSubTabs(showOwn: Boolean): List<EmojiSubTab> =
    EmojiSubTab.entries.filter { it != EmojiSubTab.Own || showOwn }

/** 颜文字网格列数 × 行数（6×6=36/页）：颜文字比单个 emoji 宽得多，12 列会截成省略号。 */
internal const val KAOMOJI_COLUMNS = 6
internal const val KAOMOJI_ROWS = 6
internal const val KAOMOJI_PAGE_SIZE = KAOMOJI_COLUMNS * KAOMOJI_ROWS

/**
 * 表情页浏览态（搜索态由 `SymbolWindow` 的搜索结果区接管，不在这里）。
 *
 * @param data 已过字形过滤的表情表（`EmojiData.filter` 的产物）。
 * @param kaomoji `kaomoji.tsv` 的解析结果（缺失 → 空列表，颜文字页显示空态）。
 */
@Composable
internal fun EmojiPane(
    data: EmojiData.Data,
    sub: EmojiSubTab,
    kaomoji: List<KaomojiData.Category>,
    modifier: Modifier = Modifier,
    onPick: (String) -> Unit,
    onCopy: (PetEmojiMap.Form) -> Unit,
) {
    when (sub) {
        EmojiSubTab.Own -> PetStickerPage(PetEmojiMap.forms, modifier, onCopy)
        EmojiSubTab.Emoji -> EmojiBrowse(data, modifier, onPick)
        EmojiSubTab.Kaomoji -> KaomojiBrowse(kaomoji, modifier, onPick)
    }
}

/** Emoji 子页：表情表，12×6 分页 + 底部分类条（不再含「自有」）。 */
@Composable
private fun EmojiBrowse(data: EmojiData.Data, modifier: Modifier, onPick: (String) -> Unit) {
    if (data.entries.isEmpty()) {
        // 表情表被摘空 / 资源缺失：分页器不能拿 0 页数（未定义用法）。
        CenteredHint("没有表情数据", modifier)
        return
    }
    // 字形过滤在后台落地时 `entries` 从 0 变成全量（见 SymbolWindow 的 deferredFilter）：
    // `rememberPagerState { pages }` 的页数 lambda 是首次组合时捕获的 ⇒ 必须按条数重建。
    key(data.entries.size) {
        val pages = (data.entries.size + SymbolCategories.PAGE_SIZE - 1) / SymbolCategories.PAGE_SIZE
        val pager = rememberPagerState { pages }
        Column(modifier.fillMaxHeight()) {
            HorizontalPager(state = pager, modifier = Modifier.weight(1f).fillMaxWidth()) { page ->
                SymbolGridPage(data.entries, page, emoji = true, onPick = onPick)
            }
            CategoryBar(data.groups, pager, SymbolCategories.PAGE_SIZE)
        }
    }
}

/** 颜文字子页：数据缺失时显示空态（不崩窗，理由同符号页的「没有类目数据」）。 */
@Composable
private fun KaomojiBrowse(
    categories: List<KaomojiData.Category>,
    modifier: Modifier,
    onPick: (String) -> Unit,
) {
    val all = remember(categories) { KaomojiData.flatten(categories) }
    if (all.isEmpty()) {
        Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text("没有颜文字数据", fontSize = 14.sp, color = SymDim)
        }
        return
    }
    val groups = remember(categories) {
        categories.map { EmojiData.Group(it.label, it.start, it.items.size) }
    }
    key(all.size) {
        val pages = (all.size + KAOMOJI_PAGE_SIZE - 1) / KAOMOJI_PAGE_SIZE
        val pager = rememberPagerState { pages }
        Column(modifier.fillMaxHeight()) {
            HorizontalPager(state = pager, modifier = Modifier.weight(1f).fillMaxWidth()) { page ->
                SymbolGridPage(
                    all,
                    page,
                    columns = KAOMOJI_COLUMNS,
                    rows = KAOMOJI_ROWS,
                    fontSize = 13.sp,
                    onPick = onPick,
                )
            }
            CategoryBar(groups, pager, KAOMOJI_PAGE_SIZE)
        }
    }
}

/**
 * 底部分类条（可横滑；选中 = 强调色粗体 + 药丸底）。
 *
 * 高亮反推**只在本组件内读** `pager.currentPage` —— 读在页根上会让整页跟着滚动重组。
 * 点一项把分页器滚到该组首项所在页。
 */
@Composable
private fun CategoryBar(groups: List<EmojiData.Group>, pager: PagerState, pageSize: Int) {
    if (groups.isEmpty()) return
    val selected = EmojiData.groupIndexOf(groups, pager.currentPage * pageSize)
    val scope = rememberCoroutineScope()
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(34.dp)
            .background(SymStrip)
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        for ((i, g) in groups.withIndex()) {
            val on = i == selected
            Text(
                g.label,
                modifier = Modifier
                    .padding(horizontal = 2.dp)
                    .clip(RoundedCornerShape(8.dp))
                    .background(if (on) SymHover else Color.Transparent)
                    .clickable { scope.launch { pager.animateScrollToPage(EmojiData.pageOfGroup(g, pageSize)) } }
                    .semantics {
                        contentDescription =
                            "分类 ${g.label}（${g.count} 个）${if (on) "，当前" else ""}"
                    }
                    .padding(horizontal = 10.dp, vertical = 6.dp),
                fontSize = 12.sp,
                fontWeight = if (on) FontWeight.Bold else FontWeight.Normal,
                color = if (on) SymAccent else SymFg,
                maxLines = 1,
            )
        }
    }
}

/** 组条上一层的「表情」类目条项（与右竖条同一列，只是这条不是 .tsv 类目）。 */
@Composable
internal fun EmojiStripEntry(selected: Boolean, onClick: () -> Unit) {
    val interaction = remember { MutableInteractionSource() }
    Text(
        "表情",
        modifier = Modifier.fillMaxWidth()
            .background(if (selected) SymHover else SymStrip)
            .clickable(interactionSource = interaction, indication = null, onClick = onClick)
            .semantics { contentDescription = "表情页${if (selected) "，已选中" else ""}" }
            .padding(horizontal = 10.dp, vertical = 9.dp),
        fontSize = 14.sp,
        fontWeight = if (selected) FontWeight.Bold else FontWeight.Normal,
        color = if (selected) SymAccent else SymFg,
        maxLines = 1,
    )
}

/** ✕ 关闭（自绘窗没有系统标题栏，关闭入口得自己给；符号页标题行也用）。 */
@Composable
internal fun CloseButton(onClose: () -> Unit) {
    val interaction = remember { MutableInteractionSource() }
    val hovered by interaction.collectIsHoveredAsState()
    Box(
        modifier = Modifier
            .size(26.dp)
            .background(if (hovered) SymHover else Color.Transparent)
            .clickable(interactionSource = interaction, indication = null, onClick = onClose)
            .semantics { contentDescription = "关闭符号页" },
        contentAlignment = Alignment.Center,
    ) {
        Text("✕", fontSize = 13.sp, color = SymFg)
    }
}

/**
 * 表情页顶栏（与 Android `EmojiPanel` 的同一套布局）：`[← 返回] [子页…] [搜索框]`。
 * 「自有」只在设置开关打开时出现在 [tabs] 里（缺省只有 Emoji | 颜文字）。
 *
 * 桌面窗无系统标题栏 ⇒ 最右保留 ✕（本窗自己的页级关闭入口）。搜索沿用桌面现有语义：
 * 输入框常驻（点进去即 `typing`，窗按需获焦）、防抖后走 `opi-ctl search`；颜文字子页
 * 的搜索改为本地子串匹配（见 `SymbolWindow` 的 `kaomojiSearch`）。
 */
@Composable
internal fun EmojiTopBar(
    sub: EmojiSubTab,
    tabs: List<EmojiSubTab>,
    onSub: (EmojiSubTab) -> Unit,
    onBack: () -> Unit,
    query: String,
    onQuery: (String) -> Unit,
    onFocusChanged: (Boolean) -> Unit,
    onClose: () -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth().height(40.dp).padding(start = 8.dp, end = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            "← 返回",
            modifier = Modifier
                .clip(RoundedCornerShape(6.dp))
                .clickable(onClick = onBack)
                .semantics { contentDescription = "返回符号页" }
                .padding(horizontal = 8.dp, vertical = 6.dp),
            fontSize = 13.sp,
            color = SymFg,
            maxLines = 1,
        )
        // 子页切换居中；选中 = 药丸底 + 强调色粗体（与底部分类条同一套视觉）。
        Row(
            modifier = Modifier.weight(1f),
            horizontalArrangement = Arrangement.Center,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            for (t in tabs) {
                val on = t == sub
                Text(
                    t.label,
                    modifier = Modifier
                        .padding(horizontal = 2.dp)
                        .clip(RoundedCornerShape(6.dp))
                        .background(if (on) SymHover else Color.Transparent)
                        .clickable { onSub(t) }
                        .semantics {
                            contentDescription = t.label + if (on) "，已选中" else ""
                        }
                        .padding(horizontal = 14.dp, vertical = 5.dp),
                    fontSize = 14.sp,
                    fontWeight = if (on) FontWeight.Bold else FontWeight.Normal,
                    color = if (on) SymAccent else SymDim,
                    maxLines = 1,
                )
            }
        }
        SymbolSearchField(
            query = query,
            onQuery = onQuery,
            onFocusChanged = onFocusChanged,
            modifier = Modifier.width(220.dp),
        )
        Spacer(Modifier.width(8.dp))
        CloseButton(onClose)
    }
}

/**
 * 颜文字搜索（本地子串匹配，与 Android `KaomojiCategories.flatten(...).filter` 同义）。
 * 命中沿用 6×6 网格；无命中给一句话（不报成「搜索失败」—— 这里根本没有通道可失败）。
 */
@Composable
internal fun KaomojiSearchPane(
    categories: List<KaomojiData.Category>,
    query: String,
    modifier: Modifier,
    onPick: (String) -> Unit,
) {
    val hits = remember(query, categories) {
        KaomojiData.flatten(categories)
            .filter { it.contains(query.trim(), ignoreCase = true) }
            .distinct()
    }
    if (hits.isEmpty()) {
        CenteredHint("没有匹配的颜文字", modifier)
        return
    }
    val pages = (hits.size + KAOMOJI_PAGE_SIZE - 1) / KAOMOJI_PAGE_SIZE
    // key(hits)：换关键词就重建分页器（理由同 SymbolSearch 的 HitGrid）。
    key(hits) {
        val pager = rememberPagerState { pages }
        HorizontalPager(state = pager, modifier = modifier) { page ->
            SymbolGridPage(
                hits,
                page,
                columns = KAOMOJI_COLUMNS,
                rows = KAOMOJI_ROWS,
                fontSize = 13.sp,
                onPick = onPick,
            )
        }
    }
}
