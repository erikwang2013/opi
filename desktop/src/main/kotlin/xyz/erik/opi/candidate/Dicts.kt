// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 设置窗「词库」页（2026-10-14，M3 的 2c **桌面半边**）：目录推导 + 列表/计数/导入/
//! 删除的纯函数与 IO + 本页 UI。**不链接引擎**（本窗的传统：`desktop/` 只用 AWT/JNA
//! 碰系统，词库的解析/加载是 IME 侧的事）。
//!
//! **目录契约**（与 IME 侧共用）：
//! · Linux = `$XDG_DATA_HOME/opi/dicts`（缺省 `~/.local/share/opi/dicts`）
//! · Windows = `%LOCALAPPDATA%\opi\dicts`
//! 文件 = `*.tsv`，列序 **`词<TAB>拼音<TAB>频次`**（两列合法：缺频次按 1000）——
//! 与 `scripts/gen_*.py` 的产物、`engine-data::dict_text::parse_report` 同一口径
//!（⚠️ lead 的任务书里曾把列序写反成「拼音<TAB>词」，以解析器与生成器为准）。
//! （**词库名 = 文件名去 `.tsv`**；停用名单 `DisabledDicts` 里存的就是这个名字）。
//!
//! 三个刻意裁决（门禁逐条钉住，别当默认行为看；均经 lead 2026-10-14 批准）：
//! · **条数 = 流式数「合法形状行」**（**1–2 个 TAB** —— 2 列 = 缺频次、解析器按 1000 收，
//!   与 `parse_report` 的 `2..=3` 同口径；坏行不计数也不隐藏 —— 文件照列、数字如实：
//!   隐藏半坏文件才是「导入完什么都没发生」的静默失败）。整文件不进内存：
//!   词库可以很大。超过 [DICT_COUNT_MAX_BYTES] 就不数，如实显示「太大」。
//! · **导入 = 复制**；重名**自动加后缀**（`foo.tsv` → `foo-2.tsv` → …），**绝不覆盖**
//!   —— 覆盖会静默毁掉用户可能改过的词库，加后缀可逆、可解释。选中的源文件本来就
//!   在词库目录里（同一个文件）⇒ 不复制、原样认账（否则会多出一份自我拷贝）。
//! · **停用名单空 = 全部启用**（键 `DisabledDicts`）。提示文案如实：名单由输入法在
//!   **下次启动**时读（Windows 侧生效时机 = 下次激活）——本页只负责把名单写进配置，
//!   在线生效不在本批。
//!
//! ⚠️ 名字含逗号/首尾空白的词库在 `DisabledDicts` 里**表达不了**（逗号是分隔符；空白
//! 会被解析 trim 掉，写进去也永远不命中）—— 停用时**拒绝翻转 + 状态行明说**
//! （[withDictEnabled] 返回 null 的契约）：拒绝 + 告知 > 拨了开关没反应的静默失效。
//!
//! ⚠️ **证据边界**：本页有**纯 JVM 行为门禁**（DictsTest：目录推导/列表/计数/导入/
//! 删除/停用名单）和**离屏渲染烟测**（DictsRenderProbeTest：空目录与两库两种输入下
//! 渲得出、画布上有墨迹）；**真窗观感仍零证据** —— 本机无 X（没开过真窗；离屏证不了
//! 窗的位置/焦点/真机字体渲染），好不好看（间距/配色）也**没有判据**，别把「门禁绿」
//! 读成「页面好看」。
//!
//! ⚠️ 页 UI 与逻辑同放本文件（不塞 SettingsPages.kt）：那个文件已近 400 行，
//! 500 行门禁（opi-ffi/tests/line_limit.rs）就在眼前，拆分是「一份功能一个文件」。

package xyz.erik.opi.candidate

import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.AlertDialog
import androidx.compose.material.Switch
import androidx.compose.material.SwitchDefaults
import androidx.compose.material.Text
import androidx.compose.material.TextButton
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
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import javax.swing.JFileChooser
import javax.swing.SwingUtilities
import javax.swing.filechooser.FileNameExtensionFilter
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** 数行上限：超过就不数（200MB ≈ 数百万行；数它只是拿磁盘 I/O 换一个没人看的数字）。 */
internal const val DICT_COUNT_MAX_BYTES: Long = 200L * 1024 * 1024

/**
 * 随包分发的系统词库（只读）。顺序 = 展示序。
 * ⚠️ 桌面窗拿不到它们的条数（在引擎/安装包里，不是本目录的文件）——**别编**，
 * 列表里写「随包」（见 [SystemDictRow]）。
 */
internal val SYSTEM_DICTS = listOf("luna", "trad", "stroke")

/** 一个导入词库的条数状态：三种**都如实显示** —— 沉默的 0 是假信息。 */
internal sealed class DictCount {
    /** 合法三列行数（坏行/空行不计）。 */
    data class Lines(val n: Long) : DictCount()

    /** 超过 [DICT_COUNT_MAX_BYTES]，没数。 */
    data object TooLarge : DictCount()

    /** 打不开 / 读一半出错（权限、I/O）—— 不冒充 0。 */
    data object Unreadable : DictCount()
}

/** 列表一行（名字 = 停用名单里写的那个名字）。 */
internal data class DictEntry(
    val name: String,
    val path: Path,
    val bytes: Long,
    val count: DictCount,
)

/**
 * 词库目录（**纯函数**：环境量都当参数传，单测钉住两条路径 —— 与 [settingsPathFor]
 * 同一形状、同一份理由：目录错 = 导入的词库 IME 永远读不到，**静默**）。
 */
fun dictsDirFor(
    windows: Boolean,
    xdgDataHome: String?,
    home: String?,
    localAppData: String?,
): Path = if (windows) {
    val base = localAppData?.takeIf { it.isNotEmpty() } ?: "."
    Paths.get(base, "opi", "dicts")
} else {
    // XDG_DATA_HOME 缺省 $HOME/.local/share；空串等同未设置（与 Rust 侧 data_dir、
    // 本文件 settingsPathFor 的 takeIf { isNotEmpty() } 同口径）。
    val base = xdgDataHome?.takeIf { it.isNotEmpty() } ?: ((home ?: "") + "/.local/share")
    Paths.get(base, "opi", "dicts")
}

/** 真实环境下的词库目录（[dictsDirFor] 的取环境量包装）。 */
fun dictsDir(): Path = dictsDirFor(
    windows = isWindows(),
    xdgDataHome = System.getenv("XDG_DATA_HOME"),
    home = System.getenv("HOME"),
    localAppData = System.getenv("LOCALAPPDATA"),
)

/**
 * 流式数「合法形状行」：**1–2 个 TAB**（= 解析器 `parse_report` 收的 2–3 列
 * `if !(2..=3).contains(&cols.len())`；2 列 = 缺频次、按 1000）。0 个 TAB（1 列）、
 * ≥3 个 TAB（≥4 列）、空行都不计。
 *
 * ⚠️ 这是**形状计数**，不是解析器等价：空词/坏频次（如 `好\thao\tabc`）形状合法但
 * 解析器会丢，这里照数；`#` 注释行等解析器另有规矩，这里也没跟着认。差异只在坏行/
 * 注释行上 —— 页面给的是「这文件有多少行像词条」，别把它读成解析器的精确条数。
 * 按行读（BufferedReader）——**整个文件不进内存**。字符集按 UTF-8 且解码失败替换
 * （GBK 词库不抛，TAB 是 ASCII 不受影响）。超上限/打不开见 [DictCount]。
 */
internal fun countDictLines(p: Path): DictCount {
    val size = try {
        Files.size(p)
    } catch (_: Exception) {
        return DictCount.Unreadable
    }
    if (size > DICT_COUNT_MAX_BYTES) return DictCount.TooLarge
    return try {
        var n = 0L
        Files.newBufferedReader(p).use { r ->
            while (true) {
                val line = r.readLine() ?: break
                if (line.count { it == '\t' } in 1..2) n++
            }
        }
        DictCount.Lines(n)
    } catch (_: Exception) {
        DictCount.Unreadable
    }
}

/**
 * 列出导入词库：`*.tsv` 的**普通文件**，按名字排序。非 tsv 不列；目录不在
 * （还没导入过）/打不开 ⇒ 空列表（不是错误，本页照常显示「还没有导入词库」）。
 */
internal fun listDicts(dir: Path): List<DictEntry> = try {
    Files.newDirectoryStream(dir, "*.tsv").use { ds ->
        ds.filter { Files.isRegularFile(it) }
            .map { p ->
                DictEntry(
                    name = p.fileName.toString().removeSuffix(".tsv"),
                    path = p,
                    bytes = try {
                        Files.size(p)
                    } catch (_: Exception) {
                        0L
                    },
                    count = countDictLines(p),
                )
            }
            .sortedBy { it.name }
    }
} catch (_: Exception) {
    emptyList()
}

/**
 * 复制 [src] 进 [dir]（不存在则连同目录创建），返回**最终落点**。
 *
 * 重名策略 = **自动加后缀、绝不覆盖**（见文件头裁决）：`foo.tsv` 已存在 ⇒ 试
 * `foo-2.tsv`、`foo-3.tsv`…（复制不带 REPLACE_EXISTING —— 拿到名字后若仍有并发来客，
 * 撞了就抛、不覆盖，宁可失败不毁数据）。[src] 与已存在的目标是**同一个文件** ⇒
 * 不复制、原样返回它（用户在文件选择器里指了词库目录自己的文件）。
 *
 * 非 `.tsv`/源不存在 ⇒ [IllegalArgumentException]（UI 侧如实显示，不静默）。
 */
internal fun importDict(src: Path, dir: Path): Path {
    require(src.fileName.toString().endsWith(".tsv")) { "只支持 .tsv 词库文件：${src.fileName}" }
    require(Files.isRegularFile(src)) { "源文件不存在或不是普通文件：$src" }
    val stem = src.fileName.toString().removeSuffix(".tsv")
    Files.createDirectories(dir)
    var dst = dir.resolve("$stem.tsv")
    if (Files.exists(dst) && Files.isSameFile(src, dst)) return dst
    var i = 2
    while (Files.exists(dst)) {
        dst = dir.resolve("$stem-$i.tsv")
        i++
    }
    Files.copy(src, dst)
    return dst
}

/** 删一个词库（UI 侧先弹确认框，与「清除用户词库」同一条惯例）。返回是否真删了。 */
internal fun deleteDict(dir: Path, name: String): Boolean =
    try {
        Files.deleteIfExists(dir.resolve("$name.tsv"))
    } catch (_: Exception) {
        false
    }

/** 解析停用名单：逗号切、去空白、丢空段（容错与 `fuzzyPairSet` 同调）。空串 = 空集。 */
internal fun parseDisabledDicts(v: String): Set<String> =
    v.split(',').map { it.trim() }.filter { it.isNotEmpty() }.toCollection(LinkedHashSet())

/** 词库是否启用：**空名单 = 全部启用**（缺省行为）。 */
internal fun OpSettings.dictEnabled(name: String): Boolean = name !in parseDisabledDicts(disabledDicts)

/**
 * 勾/取消停用一个词库；一个都不停用 ⇒ 空串（仍表达「全启用」）。
 *
 * **名单里表达不了的名字 ⇒ 返回 null（拒绝此翻转）**，调用方必须明说而不是照写：
 * 名字含逗号会被解析切成两个名字（拨了开关没反应 + 殃及别的名字），首尾带空白会被
 * `trim` 掉（写进去也永远不命中）—— 两种都是「拨了开关没反应」的静默失效。
 * 判据 = **写完解析回来必须等于打算写的集合**（不枚举坏形状：逗号/空白/将来的分隔符
 * 一起挡）。null 时**什么都不改**（OpSettings 不可变，调用方别调 apply 即可）。
 */
internal fun OpSettings.withDictEnabled(name: String, enabled: Boolean): OpSettings? {
    val next = parseDisabledDicts(disabledDicts).toMutableSet()
    if (enabled) next.remove(name) else next.add(name)
    val written = next.joinToString(",")
    return if (parseDisabledDicts(written) == next) copy(disabledDicts = written) else null
}

/** 条数文案（三态都如实；「太大」不冒充 0）。 */
internal fun countText(c: DictCount): String = when (c) {
    is DictCount.Lines -> "${c.n} 条"
    DictCount.TooLarge -> "太大（超过 ${DICT_COUNT_MAX_BYTES / (1024 * 1024)} MB，未数）"
    DictCount.Unreadable -> "读不了"
}

/** 字节 → 人读（整数量级；不用 String.format —— 小数点符号会跟 locale 走）。 */
internal fun formatBytes(n: Long): String = when {
    n < 1024 -> "$n B"
    n < 1024 * 1024 -> "${n / 1024} KB"
    else -> "${n / (1024 * 1024)} MB"
}

/**
 * 弹 AWT 的 JFileChooser 选一个 .tsv（本窗既有 AWT/Compose 混合先例；无需新依赖）。
 * Swing 的规矩 = 对话框在 EDT 上 show，调用方从 IO 线程 `withContext(Dispatchers.IO)`
 * 包进来（本模块 2026-10-13 EDT 取证轮的纪律：组合线程不碰阻塞 IO）。
 * 取消 / headless（测试机）⇒ null。
 *
 * 父窗口从 KeyboardFocusManager 现取（在 EDT 上、且**在**创建 chooser 之前 —— 之后
 * 活动窗口就是 chooser 自己了）。不用 compose 的 `LocalWindow`：CMP 1.11 里它标
 * internal，Kotlin 侧取不到（javap 能看到 public getter 是假象）。
 */
internal fun pickTsvFile(): Path? {
    var picked: java.io.File? = null
    return try {
        SwingUtilities.invokeAndWait {
            val parent = java.awt.KeyboardFocusManager.getCurrentKeyboardFocusManager().activeWindow
            val fc = JFileChooser()
            fc.dialogTitle = "选择词库文件（*.tsv）"
            fc.fileFilter = FileNameExtensionFilter("OPI 词库 (*.tsv)", "tsv")
            if (fc.showOpenDialog(parent) == JFileChooser.APPROVE_OPTION) picked = fc.selectedFile
        }
        picked?.toPath()
    } catch (_: Exception) {
        null
    }
}

// ---------- 页 UI（Linux/Windows 都出；IO 一律 Dispatchers.IO）----------

/**
 * 「词库」页。列表/计数/复制/删除全是文件 IO ⇒ 全部 `Dispatchers.IO`，组合线程只
 * 拿结果（状态机：`entries == null` = 首次读取还没回来）。
 *
 * [dir] = 词库目录，缺省 = 真环境的 [dictsDir]（纯函数、门禁在 DictsTest）。参数化的
 * 唯一用途是让离屏渲染烟测（DictsRenderProbeTest）拿临时目录画同一路径的两种态 ——
 * 页面行为一字不差，别在别处传自定义值。
 */
@Composable
internal fun DictsPage(
    s: OpSettings,
    apply: (OpSettings) -> Unit,
    status: (String) -> Unit,
    dir: Path = dictsDir(),
) {
    val scope = rememberCoroutineScope()
    var entries by remember { mutableStateOf<List<DictEntry>?>(null) }
    // 待确认删除的名字；非 null ⇒ 弹确认框（与「清除用户词库」同一条：先问再动）。
    var confirmDelete by remember { mutableStateOf<String?>(null) }

    fun refresh() {
        scope.launch { entries = withContext(Dispatchers.IO) { listDicts(dir) } }
    }
    LaunchedEffect(Unit) { refresh() }

    SectionTitle("系统词库")
    for (name in SYSTEM_DICTS) SystemDictRow(name)
    Text(
        "随安装包分发，不可改；条数由输入法引擎管，本窗读不到就不显示。",
        fontSize = 12.sp, color = Muted,
    )

    SectionTitle("导入词库")
    ActionRow(
        "导入词库",
        "选一个 .tsv 复制进词库目录（$dir）；重名自动改名（foo-2.tsv），不会覆盖",
    ) {
        scope.launch {
            val picked = withContext(Dispatchers.IO) { pickTsvFile() } ?: return@launch
            val result = withContext(Dispatchers.IO) {
                runCatching { importDict(picked, dir) }
            }
            status(
                result.fold(
                    onSuccess = { dst ->
                        if (isSameFile(picked, dst)) "该文件已在词库目录里：$dst"
                        else "已导入词库：$dst"
                    },
                    onFailure = { "导入失败：${it.message ?: it}" },
                ),
            )
            refresh()
        }
    }
    when (val list = entries) {
        null -> Text("正在读取词库目录…", fontSize = 12.sp, color = Muted)
        else ->
            if (list.isEmpty()) {
                Text("还没有导入过词库。", fontSize = 12.sp, color = Muted)
            } else {
                for (e in list) {
                    DictRow(
                        entry = e,
                        enabled = s.dictEnabled(e.name),
                        onToggle = { on ->
                            // null = 这个名字在停用名单里表达不了（含逗号/首尾空白）：
                            // 不改文件、开关弹回原位，状态行明说（拒绝 + 告知 > 静默失效）。
                            val next = s.withDictEnabled(e.name, on)
                            if (next == null) {
                                status("「${e.name}」这个名字在停用名单里表达不了（含逗号或首尾空白）；请先重命名文件")
                            } else {
                                apply(next)
                            }
                        },
                        onDelete = { confirmDelete = e.name },
                    )
                }
            }
    }
    Text(
        "停用名单由输入法在下次启动时读；Windows 上生效时机 = 下次激活。配置键名 DisabledDicts。",
        fontSize = 12.sp, color = Muted,
    )

    confirmDelete?.let { name ->
        AlertDialog(
            onDismissRequest = { confirmDelete = null },
            title = { Text("删除词库「$name」") },
            text = { Text("将从词库目录删除 $name.tsv（不可恢复），确定吗？") },
            buttons = {
                Row(
                    Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 8.dp),
                    horizontalArrangement = Arrangement.End,
                ) {
                    TextButton(onClick = { confirmDelete = null }) { Text("取消") }
                    TextButton(onClick = {
                        confirmDelete = null
                        scope.launch {
                            val ok = withContext(Dispatchers.IO) { deleteDict(dir, name) }
                            status(if (ok) "已删除词库「$name」" else "词库目录里已经没有「$name」了")
                            refresh()
                        }
                    }) { Text("删除") }
                }
            },
        )
    }
}

/** 系统词库一行：名字 + 「随包」（拿不到条数就别编 —— 见 [SYSTEM_DICTS]）。 */
@Composable
private fun SystemDictRow(name: String) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(name, fontSize = 14.sp, color = Ink, modifier = Modifier.weight(1f))
        Text("随包", fontSize = 12.sp, color = Muted)
    }
}

/** 导入词库一行：名字 / 条数 · 大小 / 删除 / 启用开关。 */
@Composable
private fun DictRow(
    entry: DictEntry,
    enabled: Boolean,
    onToggle: (Boolean) -> Unit,
    onDelete: () -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(entry.name, fontSize = 14.sp, color = Ink)
            Text("${countText(entry.count)} · ${formatBytes(entry.bytes)}", fontSize = 12.sp, color = Muted)
        }
        // 删除按钮的小样式与 ActionRow 的按钮同款（青边青字）
        Box(
            modifier = Modifier.border(1.dp, Teal).clickable(onClick = onDelete)
                .padding(horizontal = 10.dp, vertical = 4.dp),
        ) {
            Text("删除", fontSize = 12.sp, color = Teal)
        }
        Spacer(Modifier.width(12.dp))
        Switch(
            checked = enabled,
            onCheckedChange = onToggle,
            colors = SwitchDefaults.colors(checkedThumbColor = Color.White, checkedTrackColor = Teal),
        )
    }
}

/** 两个路径是否同一个文件（失败 = 否，调用点只用来挑文案）。 */
private fun isSameFile(a: Path, b: Path): Boolean =
    try {
        Files.isSameFile(a, b)
    } catch (_: Exception) {
        false
    }
