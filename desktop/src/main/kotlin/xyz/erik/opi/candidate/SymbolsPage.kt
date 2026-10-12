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
import kotlinx.coroutines.withContext
import xyz.erik.opi.pet.OpiPetWindowIcon
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths

// ---------- 类目数据 ----------

/**
 * 符号类目解析。**逐行镜像 Android 的 `keyboard/SymbolCategories.kt`**（同一条
 * 「解析是纯函数、读资产归调用方」的先例）—— 那边改了口径这边要同改，否则两端同一份
 * TSV 会解析出不同的页。
 *
 * 符号按**空格分隔的 token** 存（不是逐码点切）：组合序列（`o͘`、`╮(╯▽╰)╭`）必须整块
 * 显示；逐码点切会把代理对拆坏、也会把组合字符拆成裸基字。
 */
object SymbolCategories {

    /** 一个类目：序号（1 起，与展示的「1.常用」一致）、名称、符号表（表序 = 生成序）。 */
    data class Category(val index: Int, val label: String, val symbols: List<String>)

    /**
     * 解析 TSV。坏行（列数不对/序号非数字/名称为空/无符号）**跳过而不抛** —— 文件是
     * 生成物，但可能被替换/截断，页面该降级成少几个类目而不是打不开。
     */
    fun parse(text: String): List<Category> {
        val out = ArrayList<Category>()
        for (line in text.lineSequence()) {
            if (line.isBlank()) continue
            val parts = line.split('\t')
            if (parts.size < 3) continue
            val index = parts[0].toIntOrNull() ?: continue
            val label = parts[1]
            if (label.isEmpty()) continue
            val symbols = parts[2].split(' ').filter { it.isNotEmpty() }
            if (symbols.isEmpty()) continue
            out.add(Category(index, label, symbols))
        }
        return out
    }

    /**
     * 每页符号数（**12 列 × 6 行**；2026-10-12 用户裁决「符号按钮点击后要是个**大的**
     * 页面，可以翻页选择」后从 8×4=32 放大到 72 —— 桌面窗比手机屏大得多，32 个一页
     * 要翻太多次）。⚠️ 与 Android 的 `PAGE_SIZE`（32）**有意不同**了：页大小跟**屏**
     * 走，两端不再同值（原先「同值」那句注释随本次裁决作废）。
     */
    const val PAGE_SIZE = 72

    /**
     * 按「本机画不画得出这个字形」过滤每类的符号表（2026-10-13）。保序、幂等。
     *
     * 类目本身**不删**（哪怕整类被摘空）—— 与表情页组条同一裁决：右竖条是按序号的
     * 固定格位，格位随字体变化就没法靠位置记「第三个是数学符号」了；摘空的类目显示成
     * 空网格（不是「没有类目数据」，那是我一句都没有的降级）。
     *
     * 为什么符号页也要过这一遍：Android 只过滤表情列表（那边的符号块是 Unicode 块展出，
     * 缺字形罕见），而桌面这两页是同一份数据、同一个字体问题 —— 空心的方框在哪一页都
     * 不该出现。页索引不用另算：页数由**过滤后**的 `symbols.size` 现推（见调用点）。
     */
    fun filter(categories: List<Category>, canRender: (String) -> Boolean): List<Category> {
        var changed = false
        val out = categories.map { c ->
            val kept = c.symbols.filter(canRender)
            if (kept.size == c.symbols.size) c else { changed = true; c.copy(symbols = kept) }
        }
        return if (changed) out else categories
    }

    /** 网格列数（行数由 [PAGE_SIZE] 推出）。 */
    const val COLUMNS = 12
}

/**
 * 数据文件候选（**纯函数**：环境量当参数传，单测才能钉住 —— 这是「本页读哪个文件」
 * 与「打包往哪拷」的接缝，错了就是静默显示「没有类目数据」）。**顺序即优先级**：
 * Windows = ① `%LOCALAPPDATA%\opi\` → ② exe 同目录（对齐 `crates/tsf-opi/src/dict_path.rs`
 * 的「DLL 同目录」候选：LOCALAPPDATA 那份可能没装上/被删，装机里自带的一份还能顶上）；
 * `exeDir` 为 null（非 jpackage 启动：IDE/命令行）时该顺位退出。
 */
fun symbolDataCandidates(windows: Boolean, localAppData: String?, exeDir: Path?): List<Path> =
    if (windows) {
        val base = localAppData?.takeIf { it.isNotEmpty() } ?: "."
        listOfNotNull(
            Paths.get(base, "opi", "symbol_categories.tsv"),
            exeDir?.resolve("symbol_categories.tsv"),
        )
    } else {
        listOf(Paths.get("/usr/share/opi/symbol_categories.tsv"))
    }

/**
 * exe 同目录 = jpackage 启动器所在目录（启动器会设 `jpackage.app-path`；本机 JDK17 的
 * jdk.jpackage 模块里 grep 得到这个字面串，非杜撰）。从 IDE/命令行直接跑时没有它。
 */
internal fun exeDir(): Path? =
    System.getProperty("jpackage.app-path")?.takeIf { it.isNotEmpty() }?.let { Paths.get(it).parent }

/**
 * 读盘 + 解析：按候选顺位取**第一个真有数据**的文件；全都不行 → 空列表
 * （页面显示「没有类目数据」，不崩、不硬编兜底）。
 *
 * 坏/空的候选**不算命中**、进下一顺位 —— 与 dict_path.rs 的 `load_dict` 同一语义
 * （那边把坏原因记进 notes 再换候选；这边页面只有那一种降级显示，故不留 note）。
 */
fun loadSymbolCategories(
    paths: List<Path> = symbolDataCandidates(isWindows(), System.getenv("LOCALAPPDATA"), exeDir()),
): List<SymbolCategories.Category> {
    for (p in paths) {
        val cats = try {
            SymbolCategories.parse(Files.readString(p))
        } catch (_: Exception) {
            continue // 缺/读不动 ⇒ 不是「有数据」，看下一顺位
        }
        if (cats.isNotEmpty()) return cats
    }
    return emptyList()
}

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
    // 字形过滤（2026-10-13）：**渲染前**摘掉本机字体画不出的字形（判据见 GlyphCoverage，
    // 规则逐条对齐 Android 的 canRenderGlyph）。与 categories/emoji 同寿命 —— 数据一份
    // 读一次，过滤也就一次（每帧重滤 = 每帧几千次字体查询）。
    val catsShown = remember(categories) { SymbolCategories.filter(categories, GlyphCoverage::canRender) }
    val emojiShown = remember(emoji) { EmojiData.filter(emoji, GlyphCoverage::canRender) }
    // 表情搜索的「像不像表情」判据（见 searchView）：用**未过滤**的 emoji 表建集。
    // 判据同 Android 的 `isEmoji`（那边用过滤后的列表，差别只在本机画不出的那些条目 ——
    // 它们随后会被 canRender 再摘一次，两种取法的结果相同）。
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
    val focus = LocalFocusManager.current
    val scope = rememberCoroutineScope()
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
                    // 也照上。没搜过时本窗没拿过焦点，走同步那条（与改动前同一条路）。
                    focus.clearFocus()
                    if (!window.isFocused) {
                        onCommit(sym)?.let { notice = it }
                        return
                    }
                    scope.launch {
                        var waited = 0
                        while (window.isFocused && waited < 300) { delay(20); waited += 20 }
                        onCommit(sym)?.let { notice = it }
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
                    EmojiPane(emojiShown, Modifier.weight(1f).fillMaxHeight(), ::pick)
                } else {
                    val cat = catsShown.getOrNull(selected)
                    if (cat == null) {
                        Box(
                            modifier = Modifier.weight(1f).fillMaxHeight(),
                            contentAlignment = Alignment.Center,
                        ) {
                            Text("没有类目数据", fontSize = 14.sp, color = SymDim)
                        }
                    } else if (cat.symbols.isEmpty()) {
                        // 整类被字形过滤摘空（这一类的字本机一个都画不出）：既不是「没有类目
                        // 数据」，也不能给分页器一个 0 页数（pageCount=0 是未定义用法）。
                        Box(
                            modifier = Modifier.weight(1f).fillMaxHeight(),
                            contentAlignment = Alignment.Center,
                        ) {
                            Text("这一类在本机没有可显示的字形", fontSize = 13.sp, color = SymDim)
                        }
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
