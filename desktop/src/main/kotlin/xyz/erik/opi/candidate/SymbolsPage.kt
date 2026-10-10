// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 符号页（Windows 路径）：22 类目右竖条 + 网格左右滑动翻页 + 点符号上屏。
//! 交互与布局对齐 Android 的 `SymbolPage.kt`（那边是键区内嵌面板，这里是独立窗）。
//!
//! 上屏通路 = 命名管道的 `commit_text`（协议条目见 Main.kt 头注释；TSF 侧
//! 在插入点插入，不动候选/composition）。
//!
//! ⚠️ **本页只有在候选窗进程里才上得了屏**：管道单实例（PipeServer 的
//! MAX_INSTANCES=1）、由候选窗进程持有。状态栏进程里的符号页会把
//! 「上屏失败」如实显示出来（见 StatusBar.kt 的 `symbolCommit`）。
//! lead 裁决（2026-10-10）：本轮**不开第二条 UI→TSF 通路**（TSF 插入面还没接线，
//! 通了也到不了文档），现状如实保留；**TSF 接线落地时再定 UI 进程拓扑**（候选窗/
//! 状态栏/符号页同进程是首选）。Windows 侧本轮没有拉起触发器（lead 裁决「代码就绪、
//! 无拉起触发器」），接线时连同上面这条一起定。
//!
//! 数据文件（真源 = `android/app/src/main/assets/symbol_categories.tsv`，由
//! `scripts/gen_symbol_categories.py` 生成）。读取**顺位**见 [symbolDataCandidates]：
//! · Windows = ① `%LOCALAPPDATA%\opi\`（与 settings.ini 同目录）→ ② **exe 同目录**
//!   （windows.yml 打包时把 tsv 装在启动器旁）→ 都缺就显示「没有类目数据」
//!   （本侧不硬编兜底数据）。
//! · Linux = `/usr/share/opi/symbol_categories.tsv`（fcitx5-glue 的 CMake install
//!   在装）。Linux 上的符号页是 fcitx5 的候选面板，不走本窗 —— 这里是共用代码。

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
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.WindowPosition
import androidx.compose.ui.window.rememberWindowState
import kotlinx.coroutines.delay
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

    /** 每页符号数（8 列 × 4 行，与 Android 同值）。页数 = ⌈符号数 / 本值⌉。 */
    const val PAGE_SIZE = 32

    /** 网格列数（行数由 [PAGE_SIZE] 推出）。 */
    const val COLUMNS = 8
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

private val SymBg = Color(0xFF2D2D2D)
private val SymFg = Color(0xFFE8E8E8)
private val SymDim = Color(0xFF9E9E9E)
private val SymStrip = Color(0xFF3A3A3A)
private val SymHover = Color(0xFF454545)
private val SymAccent = Color(0xFF4C8DFF)
private val SymErr = Color(0xFFFF8A80)

private const val WIN_W = 520
private const val WIN_H = 430
private const val STRIP_W = 96

/** 屏幕右下角往内缩一截（别压任务栏）。 */
private fun symbolPosition(): WindowPosition {
    val screen = java.awt.Toolkit.getDefaultToolkit().screenSize
    return WindowPosition((screen.width - WIN_W - 40).dp, (screen.height - WIN_H - 120).dp)
}

/**
 * 符号页窗。
 *
 * [onCommit] 返回 null = 已送出去，非 null = **给用户看的**失败原因（页面显示它）。
 * 点完**不收页也不回第 1 页**：连着点几个符号是常态；收页由用户按 ✕（这是本窗自己的
 * 页级状态决策 —— 引擎侧只负责把 text 插到插入点，不管本页停在哪）。
 */
@Composable
fun SymbolWindow(onCommit: (String) -> String?, onClose: () -> Unit) {
    // 开窗读一次（同 Android「面板第一次打开时读资产」）；窗口重开才会重读。
    val categories = remember { loadSymbolCategories() }
    var selected by remember { mutableStateOf(0) }
    var notice by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(notice) {
        if (notice != null) {
            delay(3500)
            notice = null
        }
    }
    val windowState = rememberWindowState(
        width = WIN_W.dp,
        height = WIN_H.dp,
        position = remember { symbolPosition() },
    )
    Window(
        onCloseRequest = onClose,
        state = windowState,
        undecorated = true,
        alwaysOnTop = true,
        resizable = false,
        // 不抢焦点：上屏要求文档窗口仍持有焦点（没焦点时 TSF 会跳过插入）。
        focusable = false,
        title = "opi输入法 符号大全",
        icon = OpiPetWindowIcon, // 窗口/任务栏图标 = 小欧（PetIcon.kt）
    ) {
        Column(Modifier.fillMaxSize().background(SymBg)) {
            Row(
                modifier = Modifier.fillMaxWidth().padding(start = 14.dp, end = 6.dp, top = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text("符号大全", fontSize = 15.sp, fontWeight = FontWeight.Bold, color = SymFg)
                Spacer(Modifier.width(10.dp))
                Text("点符号上屏；左右滑动翻页", fontSize = 11.sp, color = SymDim)
                Spacer(Modifier.weight(1f))
                CloseButton(onClose)
            }
            // 固定高的提示行：失败提示出现/消失不该让整页跳。
            Box(Modifier.fillMaxWidth().height(18.dp).padding(horizontal = 14.dp), contentAlignment = Alignment.CenterStart) {
                notice?.let { Text(it, fontSize = 11.sp, color = SymErr, maxLines = 1) }
            }
            Row(Modifier.weight(1f).fillMaxWidth()) {
                val cat = categories.getOrNull(selected)
                if (cat == null) {
                    Box(
                        modifier = Modifier.weight(1f).fillMaxHeight(),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text("没有类目数据", fontSize = 14.sp, color = SymDim)
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
                        SymbolGridPage(cat.symbols, page) { sym ->
                            onCommit(sym)?.let { notice = it }
                        }
                    }
                }
                CategoryStrip(categories, selected) { selected = it }
            }
        }
    }
}

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
 * 一页符号网格：8 列 × 4 行。单字符 20sp、多字符 token（网络表情等）收到 13sp 并允许
 * 省略号。末行/末页补空格子 —— 网格高度稳定，翻页不跳。
 */
@Composable
private fun SymbolGridPage(all: List<String>, page: Int, onPick: (String) -> Unit) {
    val start = page * SymbolCategories.PAGE_SIZE
    val cells = all.subList(start, minOf(start + SymbolCategories.PAGE_SIZE, all.size))
    val cols = SymbolCategories.COLUMNS
    Column(modifier = Modifier.fillMaxSize().padding(horizontal = 4.dp, vertical = 2.dp)) {
        for (rowStart in cells.indices step cols) {
            Row(modifier = Modifier.weight(1f).fillMaxWidth()) {
                for (i in rowStart until minOf(rowStart + cols, cells.size)) {
                    val s = cells[i]
                    SymbolCell(s, Modifier.weight(1f)) { onPick(s) }
                }
                for (i in minOf(rowStart + cols, cells.size) until rowStart + cols) {
                    Box(Modifier.weight(1f))
                }
            }
        }
        // 末页不足 4 行时补空行（高度稳定）
        for (r in (cells.size + cols - 1) / cols until SymbolCategories.PAGE_SIZE / cols) {
            Row(modifier = Modifier.weight(1f).fillMaxWidth()) {}
        }
    }
}

@Composable
private fun SymbolCell(sym: String, modifier: Modifier, onPick: () -> Unit) {
    val interaction = remember { MutableInteractionSource() }
    val hovered by interaction.collectIsHoveredAsState()
    Box(
        modifier = modifier
            .fillMaxHeight()
            .padding(1.dp)
            .background(if (hovered) SymHover else Color.Transparent)
            .clickable(interactionSource = interaction, indication = null, onClick = onPick)
            .semantics { contentDescription = "符号 $sym" },
        contentAlignment = Alignment.Center,
    ) {
        Text(
            sym,
            fontSize = if (sym.length > 1) 13.sp else 20.sp,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            color = SymFg,
        )
    }
}

/** 右：类目条（`序号.名称`，可竖滑；选中 = 强调色粗体）。 */
@Composable
private fun CategoryStrip(
    categories: List<SymbolCategories.Category>,
    selected: Int,
    onSelect: (Int) -> Unit,
) {
    Column(
        modifier = Modifier.width(STRIP_W.dp).fillMaxHeight()
            .background(SymStrip)
            .verticalScroll(rememberScrollState()),
    ) {
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
