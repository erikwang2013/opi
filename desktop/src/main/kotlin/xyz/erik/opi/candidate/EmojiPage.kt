// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 符号页的「表情」页（2026-10-12 用户裁决：设置与大页面都要能选符号、表情）。
//!
//! 数据 = **构建期拷进资源的 `emoji.tsv`**（真源 `data/raw/emoji.tsv`，见
//! `build.gradle.kts` 的 copyEmojiData）—— 与引擎 `builtin_emoji_texts()`/
//! `builtin_emoji_groups()` 同一份表、同一顺序（UTS#51 官方组序：笑脸在前、旗帜在后）。
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
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch

/** 表情表：全量顺序 + 官方分组（`start`/`count` 与条目下标对齐，组序 = 文件序）。 */
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

    /** 某组第一页的页码（组条跳转用）。 */
    fun pageOfGroup(g: Group): Int = g.start / SymbolCategories.PAGE_SIZE

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

/**
 * 「表情」页：顶上一行**组条**（9 组 —— 官方 UTS#51 是 10 组，Component 全是
 * `component` 状态、生成器「只收完全限定」把它整组滤掉，故实际数据里没有它；组名与
 * 顺序由引擎侧 `emoji_coverage.rs` 的同一串名字钉着）。当前页落在哪组就高亮哪组 ——
 * 左右滑动时自动跟随，不另存「当前组」状态。下面是与符号页同款的网格翻页。
 */
@Composable
internal fun EmojiPane(data: EmojiData.Data, modifier: Modifier = Modifier, onPick: (String) -> Unit) {
    if (data.entries.isEmpty()) {
        Box(modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text("没有表情数据", fontSize = 14.sp, color = SymDim)
        }
        return
    }
    val pages = (data.entries.size + SymbolCategories.PAGE_SIZE - 1) / SymbolCategories.PAGE_SIZE
    val pager = rememberPagerState { pages }
    // 当前页首条的**下标**落在哪一组 ⇒ 哪组高亮（组按 start 升序，取最后一个 start ≤ 它）。
    val active = data.groups.indexOfLast { it.start <= pager.currentPage * SymbolCategories.PAGE_SIZE }
    val scope = rememberCoroutineScope()
    // ⚠️ weight 由**调用方**给（本函数不在 RowScope 里）：见 SymbolWindow 的调用点。
    Column(modifier.fillMaxHeight()) {
        Row(
            modifier = Modifier.fillMaxWidth().height(30.dp)
                .horizontalScroll(rememberScrollState()),
        ) {
            for ((i, g) in data.groups.withIndex()) {
                val on = i == active
                Text(
                    g.label,
                    modifier = Modifier
                        .align(Alignment.CenterVertically)
                        .clickable { scope.launch { pager.animateScrollToPage(EmojiData.pageOfGroup(g)) } }
                        .semantics {
                            contentDescription =
                                "表情分类 ${g.label}（${g.count} 个）${if (on) "，当前" else ""}"
                        }
                        .padding(horizontal = 8.dp, vertical = 4.dp),
                    fontSize = 12.sp,
                    fontWeight = if (on) FontWeight.Bold else FontWeight.Normal,
                    color = if (on) SymAccent else SymFg,
                    maxLines = 1,
                )
            }
        }
        HorizontalPager(state = pager, modifier = Modifier.weight(1f).fillMaxWidth()) { page ->
            SymbolGridPage(data.entries, page, emoji = true, onPick = onPick)
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
