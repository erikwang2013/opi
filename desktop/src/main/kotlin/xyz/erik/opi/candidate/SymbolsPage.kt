// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 符号页（**两个平台共用** —— 2026-10-12 用户裁决「符号按钮点击后要是个大的页面，
//! 可以翻页选择」后，Linux 不再走 fcitx5 的候选条，也开本窗）：22 类目右竖条 +
//! 网格左右滑动翻页 + 点符号上屏。交互与布局对齐 Android 的 `SymbolPage.kt`
//! （那边是键区内嵌面板，这里是独立窗）。
//!
//! 上屏通路**两条**，由入口注入的 `onCommit` 决定（见 Main.kt 的 symbolCommit）：
//! · Windows = 命名管道的 `commit_text`（协议条目见 Main.kt 头注释；TSF 侧在插入点
//!   插入，不动候选/composition）；管道单实例（PipeServer 的 MAX_INSTANCES=1）且有
//!   **候选窗进程**持有 ⇒ ⚠️ 状态栏进程里的符号页**上不了屏**，会把失败如实显示。
//!   lead 裁决（2026-10-10）：本轮**不开第二条 UI→TSF 通路**（TSF 插入面还没接线，
//!   通了也到不了文档）；**TSF 接线落地时再定 UI 进程拓扑**（候选窗/状态栏/符号页同
//!   进程是首选）。Windows 侧本轮没有拉起触发器（lead 裁决「代码就绪、无拉起触发器」）。
//! · Linux = `opi-ctl insert <文本>`（D-Bus → addon 的 `ic->commitString`）—— 只要
//!   fcitx5 里有**焦点输入框**就能上屏，与进程拓扑无关。
//!
//! 数据文件（真源 = `android/app/src/main/assets/symbol_categories.tsv`，由
//! `scripts/gen_symbol_categories.py` 生成）。读取**顺位**见 [symbolDataCandidates]：
//! · Windows = ① `%LOCALAPPDATA%\opi\`（与 settings.ini 同目录）→ ② **exe 同目录**
//!   （windows.yml 打包时把 tsv 装在启动器旁）→ 都缺就显示「没有类目数据」
//!   （本侧不硬编兜底数据）。
//! · Linux = `/usr/share/opi/symbol_categories.tsv`（fcitx5-glue 的 CMake install
//!   在装；本机实测在：`ls -l /usr/share/opi/`）。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
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
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.WindowPosition
import androidx.compose.ui.window.rememberWindowState
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import xyz.erik.opi.pet.OpiPetWindowIcon

// ---------- 窗口 ----------

// internal（不是 private）：EmojiPage.kt 的「表情」页共用同一套色（同一个窗的两页）。
internal val SymBg = Color(0xFF2D2D2D)
internal val SymFg = Color(0xFFE8E8E8)
internal val SymDim = Color(0xFF9E9E9E)
internal val SymStrip = Color(0xFF3A3A3A)
internal val SymHover = Color(0xFF454545)
internal val SymAccent = Color(0xFF4C8DFF)
internal val SymErr = Color(0xFFFF8A80)

// 大页面（2026-10-12 用户裁决）：520×430 → 820×600；页面本身早就有翻页（HorizontalPager
// + 左右滑动），放大只是让一页装得下更多（见 PAGE_SIZE）。
private const val WIN_W = 820
private const val WIN_H = 600
private const val STRIP_W = 96

/** 居中的一行灰字占位（加载中/整类摘空/真没数据三态共用，措辞由调用方给全）。 */
@Composable
private fun CenteredHint(text: String, modifier: Modifier) {
    Box(modifier, contentAlignment = Alignment.Center) { Text(text, fontSize = 14.sp, color = SymDim) }
}

/** 屏幕右下角往内缩一截（别压任务栏）。 */
private fun symbolPosition(): WindowPosition {
    val screen = java.awt.Toolkit.getDefaultToolkit().screenSize
    return WindowPosition((screen.width - WIN_W - 40).dp, (screen.height - WIN_H - 120).dp)
}

/**
 * 符号页窗（**两页**：符号 / 表情 —— 页选择在右竖条首项，`-1` = 表情）。
 *
 * [onCommit] 返回 null = 已送出去，非 null = **给用户看的**失败原因（页面显示它）。
 * 点完**不收页也不回第 1 页**：连着点几个符号是常态；收页由用户按 ✕（这是本窗自己的
 * 页级状态决策 —— 引擎侧只负责把 text 插到插入点，不管本页停在哪）。
 * [initialEmoji] = 开窗直接落在表情页（设置里的「表情」入口；状态栏 Ω 仍从符号页开）。
 */
@Composable
fun SymbolWindow(onCommit: (String) -> String?, onClose: () -> Unit, initialEmoji: Boolean = false) {
    // 开窗读一次（同 Android「面板第一次打开时读资产」）；窗口重开才会重读。
    val categories = remember { loadSymbolCategories() }
    val emoji = remember { loadEmojiData() }
    // 表情搜索的「像不像表情」判据（见 searchView）：用**未过滤**的 emoji 表建集；判据同
    // Android 的 `isEmoji`（那边用过滤后的列表，差只在本机画不出的条目，随后 canRender 同摘）。
    val emojiSet = remember(emoji) { emoji.entries.toHashSet() }
    var selected by remember { mutableStateOf(if (initialEmoji) TAB_EMOJI else 0) }
    var notice by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(notice) {
        if (notice != null) {
            delay(3500)
            notice = null
        }
    }
    // 搜索（2026-10-13）：query = 输入框内容，search = 最近一次结局（null = 还没有结局）。
    // 空关键词 = 浏览态（不发命令：空关键词在 CLI 侧是用法错误，见 desktopSymbolSearch）。
    var query by remember { mutableStateOf("") }
    var search by remember { mutableStateOf<SymbolSearch?>(null) }
    // 本窗能不能拿系统键盘焦点（默认不能 —— 见 Window 的 focusable 注释）。只由搜索框的
    // 焦点驱动：点进搜索框才需要打字，点选符号时交还。
    var typing by remember { mutableStateOf(false) }
    LaunchedEffect(query) {
        val q = query.trim()
        if (q.isEmpty()) {
            search = null
            return@LaunchedEffect
        }
        search = null // 立刻回「搜索中…」：旧结果留在屏上会被当成新关键词的结果
        // 防抖 250ms：用户打字约 100ms 一个字，每个字都发一次进程 + D-Bus 太贵。
        delay(250)
        // ⚠️ 不能在组合线程上跑：那是一整次子进程 + D-Bus 往返（超时 3s），会把界面冻住。
        search = withContext(Dispatchers.IO) { desktopSymbolSearch(q) }
    }
    val windowState = rememberWindowState(
        width = WIN_W.dp,
        height = WIN_H.dp,
        position = remember { symbolPosition() },
    )
    val onEmoji = selected == TAB_EMOJI
    Window(
        onCloseRequest = onClose,
        state = windowState,
        undecorated = true,
        alwaysOnTop = true,
        resizable = false,
        // 默认不抢焦点：上屏要求文档窗口仍持有焦点（Windows 侧 TSF 无焦点会跳过插入；
        // Linux 侧 `opi-ctl insert` 提交到「最近获得焦点的输入上下文」，抢了焦那个上下文
        // 就成了本窗）。**搜索要打字**（键入事件来自 AWT 窗口，窗口没拿到系统焦点就一个字
        // 打不进），所以按需开：搜索框拿到焦点 ⇒ [typing]，点选符号时交还（见 pick）。
        focusable = typing,
        title = if (onEmoji) "opi输入法 表情" else "opi输入法 符号大全",
        icon = OpiPetWindowIcon, // 窗口/任务栏图标 = 小欧（PetIcon.kt）
    ) {
        // ⚠️ 这两个 local 必须读在**窗内容里**：`LocalFocusManager` 无默认值、只有 Window
        // 内容才 provide —— 读在窗前时状态栏那条路（application{} 顶层，无 Window 祖先）
        // 点 Ω 即整进程崩（2026-10-13 P0 真机复现；网见 SymbolWindowCallPathTest）。
        val focus = LocalFocusManager.current
        LaunchedEffect(window) { SkipTaskbar.apply(window) } // 同虚拟键盘：输入工具窗不进任务栏（设置窗不贴）
        val scope = rememberCoroutineScope()
        // 上屏串行锁：从前「点一下 = EDT 冻到上屏返回」，连点两下是 EDT 队列里顺序执行；
        // 挪到 IO 后并发来自协程，这把锁把旧次序保住 —— 否则连点两个符号是两条并发
        // `opi-ctl insert`，插进输入框的先后可能反序（用户的字就错位了）。
        val pickLock = remember { Mutex() }
        // 字形过滤（2026-10-13）：**渲染前**摘掉本机字体画不出的字形（判据见 GlyphCoverage，
        // 规则逐条对齐 Android 的 canRenderGlyph）。与 categories/emoji 同寿命，读一次滤一次。
        // ⚠️ 必须走 deferredFilter：过滤留在组合里会把「窗显示」推到它之后（真机实测 24.6s/15.6s）。
        val catsShown = deferredFilter(emptyList(), categories) { SymbolCategories.filter(categories, GlyphCoverage::canRender) }
        val emojiShown = deferredFilter(EmojiData.Empty, emoji) { EmojiData.filter(emoji, GlyphCoverage::canRender) }
        // 允许获焦之后才申请系统焦点（顺序反了申请无效）。交还键盘时只关 focusable 不够：
        // :0 探针实测 —— 关掉后 X 焦点仍留在本窗、用户的键入被静默丢掉；重挂一次窗才会按
        // WM 的焦点历史落回文档窗，且 focusable=false 的窗重新映射不会再抢回来。
        LaunchedEffect(typing) {
            if (typing) window.requestFocus() else if (window.isFocused) {
                window.isVisible = false
                window.isVisible = true
            }
        }
        Column(Modifier.fillMaxSize().background(SymBg)) {
            Row(
                modifier = Modifier.fillMaxWidth().padding(start = 14.dp, end = 6.dp, top = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    if (onEmoji) "表情" else "符号大全",
                    fontSize = 15.sp, fontWeight = FontWeight.Bold, color = SymFg,
                )
                Spacer(Modifier.width(10.dp))
                Text(
                    if (onEmoji) "点表情上屏；左右滑动翻页，点分类条跳组" else "点符号上屏；左右滑动翻页",
                    fontSize = 11.sp, color = SymDim,
                )
                Spacer(Modifier.weight(1f))
                // 搜索框进标题行（本行本来就是「标题 + 提示 + 关闭」的自绘条）：不新开一行，
                // 页面高度与两页的内容区一点不动（布局纪律：现有类别条/网格结构不改）。
                SymbolSearchField(
                    query = query,
                    onQuery = { query = it },
                    onFocusChanged = { typing = it },
                    modifier = Modifier.width(220.dp),
                )
                Spacer(Modifier.width(8.dp))
                CloseButton(onClose)
            }
            // 固定高的提示行：失败提示出现/消失不该让整页跳。
            Box(Modifier.fillMaxWidth().height(18.dp).padding(horizontal = 14.dp), contentAlignment = Alignment.CenterStart) {
                notice?.let { Text(it, fontSize = 11.sp, color = SymErr, maxLines = 1) }
            }
            Row(Modifier.weight(1f).fillMaxWidth()) {
                fun pick(sym: String) {
                    // 先交还键盘再上屏（Linux 的 `opi-ctl insert` 提交给「最近获得焦点的输入
                    // 上下文」，不还就会插进本窗自己）；交还异步入账，故等焦点真离开本窗，等不到
                    // 也照上。没搜过时本窗没拿过焦点，等循环零次（与改动前同一条路）。
                    //
                    // ⚠️ 上屏（onCommit）是一次子进程 + D-Bus 往返（超时 3s）：挪出组合线程，
                    // 别冻住页面 —— 与搜索那条同一条纪律（见上面 LaunchedEffect(query)；
                    // 2026-10-13 EDT 取证轮补上点选这半边）。等焦点那段留在 UI 线程（读
                    // window.isFocused 是窗口状态，跟子进程无关）。
                    focus.clearFocus()
                    scope.launch {
                        var waited = 0
                        while (window.isFocused && waited < 300) { delay(20); waited += 20 }
                        withContext(Dispatchers.IO) { pickLock.withLock { onCommit(sym) } }?.let { notice = it }
                    }
                }
                if (query.isNotBlank()) {
                    SearchResultPane(
                        searchView(search, onEmoji, emojiSet),
                        emoji = onEmoji,
                        emptyText = if (onEmoji) "没有匹配的表情" else "没有匹配的符号",
                        modifier = Modifier.weight(1f).fillMaxHeight(),
                        onPick = ::pick,
                    )
                } else if (onEmoji) {
                    // 同符号页那条诚实占位：表非空而过滤结果空 = 后台还在查（deferredFilter），
                    // 不是「没有表情数据」—— 后者是 EmojiPane 对真空表的原话。
                    val filtering = emoji.entries.isNotEmpty() && emojiShown.entries.isEmpty()
                    if (filtering) CenteredHint("正在检查字形…", Modifier.weight(1f).fillMaxHeight())
                    else EmojiPane(emojiShown, Modifier.weight(1f).fillMaxHeight(), ::pick)
                } else {
                    val cat = catsShown.getOrNull(selected)
                    if (cat == null) {
                        // 过滤在后台（deferredFilter）：类目表非空而这里空 = 还没落地；真没数据仍报原话。
                        CenteredHint(if (categories.isEmpty()) "没有类目数据" else "正在检查字形…", Modifier.weight(1f).fillMaxHeight())
                    } else if (cat.symbols.isEmpty()) {
                        // 整类被字形过滤摘空（这一类的字本机一个都画不出）：既不是「没有类目
                        // 数据」，也不能给分页器一个 0 页数（pageCount=0 是未定义用法）。
                        CenteredHint("这一类在本机没有可显示的字形", Modifier.weight(1f).fillMaxHeight())
                    } else {
                        val pages = (cat.symbols.size + SymbolCategories.PAGE_SIZE - 1) / SymbolCategories.PAGE_SIZE
                        val pager = rememberPagerState { pages }
                        // 换类目回第 1 页：否则停在上一个类目的页码上，页数还可能变少而越界。
                        LaunchedEffect(selected) { pager.scrollToPage(0) }
                        HorizontalPager(
                            state = pager,
                            modifier = Modifier.weight(1f).fillMaxHeight(),
                        ) { page ->
                            SymbolGridPage(cat.symbols, page, onPick = ::pick)
                        }
                    }
                }
                // 换类目 = 回浏览态：清掉关键词（否则点完类目，网格仍是上一个关键词的结果，
                // 而右竖条的高亮已经跳到新类目了）。
                CategoryStrip(catsShown, selected, onEmoji) { selected = it; query = "" }
            }
        }
    }
}

/** 右竖条的「表情」项 = 特殊下标（`.tsv` 类目从 0 起）。 */
private const val TAB_EMOJI = -1

/** ✕ 关闭（自绘窗没有系统标题栏，关闭入口得自己给）。 */
@Composable
private fun CloseButton(onClose: () -> Unit) {
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
 * 一页符号网格：12 列 × 6 行（`PAGE_SIZE/COLUMNS`）。单字符 20sp、多字符 token（网络
 * 表情等）收到 13sp 并允许省略号。末行/末页补空格子 —— 网格高度稳定，翻页不跳。
 *
 * [emoji] = 表情页：emoji 基本是代理对（UTF-16 长度 2），按「多字符」那档会被缩到 13sp；
 * 表情页改用**单档大字**（组内还会有 👨👩👧 这种多码位序列，统一 22sp 最稳）。
 */
@Composable
internal fun SymbolGridPage(all: List<String>, page: Int, emoji: Boolean = false, onPick: (String) -> Unit) {
    val start = page * SymbolCategories.PAGE_SIZE
    val cells = all.subList(start, minOf(start + SymbolCategories.PAGE_SIZE, all.size))
    val cols = SymbolCategories.COLUMNS
    Column(modifier = Modifier.fillMaxSize().padding(horizontal = 4.dp, vertical = 2.dp)) {
        for (rowStart in cells.indices step cols) {
            Row(modifier = Modifier.weight(1f).fillMaxWidth()) {
                for (i in rowStart until minOf(rowStart + cols, cells.size)) {
                    val s = cells[i]
                    SymbolCell(s, Modifier.weight(1f), emoji) { onPick(s) }
                }
                for (i in minOf(rowStart + cols, cells.size) until rowStart + cols) {
                    Box(Modifier.weight(1f))
                }
            }
        }
        // 末页不足 6 行时补空行（高度稳定）
        for (r in (cells.size + cols - 1) / cols until SymbolCategories.PAGE_SIZE / cols) {
            Row(modifier = Modifier.weight(1f).fillMaxWidth()) {}
        }
    }
}

@Composable
private fun SymbolCell(sym: String, modifier: Modifier, emoji: Boolean, onPick: () -> Unit) {
    val interaction = remember { MutableInteractionSource() }
    val hovered by interaction.collectIsHoveredAsState()
    Box(
        modifier = modifier
            .fillMaxHeight()
            .padding(1.dp)
            .background(if (hovered) SymHover else Color.Transparent)
            .clickable(interactionSource = interaction, indication = null, onClick = onPick)
            .semantics { contentDescription = if (emoji) "表情 $sym" else "符号 $sym" },
        contentAlignment = Alignment.Center,
    ) {
        Text(
            sym,
            fontSize = when {
                emoji -> 22.sp
                sym.length > 1 -> 13.sp
                else -> 20.sp
            },
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            color = SymFg,
        )
    }
}

/** 右：类目条（首项 = 「表情」页，其余 `序号.名称`；可竖滑；选中 = 强调色粗体）。 */
@Composable
private fun CategoryStrip(
    categories: List<SymbolCategories.Category>,
    selected: Int,
    emojiSelected: Boolean,
    onSelect: (Int) -> Unit,
) {
    Column(
        modifier = Modifier.width(STRIP_W.dp).fillMaxHeight()
            .background(SymStrip)
            .verticalScroll(rememberScrollState()),
    ) {
        EmojiStripEntry(emojiSelected) { onSelect(TAB_EMOJI) }
        for ((i, c) in categories.withIndex()) {
            val on = i == selected
            Text(
                "${c.index}.${c.label}",
                modifier = Modifier.fillMaxWidth()
                    .clickable { onSelect(i) }
                    .semantics { contentDescription = "符号类目 ${c.index}.${c.label}${if (on) "，已选中" else ""}" }
                    .padding(horizontal = 10.dp, vertical = 9.dp),
                fontSize = 14.sp,
                fontWeight = if (on) FontWeight.Bold else FontWeight.Normal,
                color = if (on) SymAccent else SymFg,
                maxLines = 1,
            )
        }
    }
}
