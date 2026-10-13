// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 符号类目一族的**数据侧**：解析（`parse`）→ 文件顺位（`symbolDataCandidates`）→
//! 读盘（`loadSymbolCategories`）。2026-10-13 从 `SymbolsPage.kt` **整块搬来**（那边
//! 撞了 500 行门禁 507 行；按职责拆，本仓同惯例），内容一字未改 —— 页面侧
//! （`SymbolWindow` / 网格 / 搜索）仍留在 SymbolsPage.kt。

package xyz.erik.opi.candidate

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
