// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

/**
 * 符号页类目：`assets/symbol_categories.tsv`（`序号<TAB>名称<TAB>符号 符号 …`，由
 * `scripts/gen_symbol_categories.py` 生成、**入库**）的解析。
 *
 * 与 [RareIndex] 同一条先例：解析是**纯函数**（`SymbolCategoriesTest` 用内联夹具 +
 * 真资产双向钉住），读资产由调用方做（面板第一次打开时读，别在服务启动时读）。
 *
 * 符号按**空格分隔的 token** 存（不是逐码点切）—— 组合序列（`o͘`、`╮(╯▽╰)╭`）必须
 * 整块显示；逐码点切会把代理对拆坏、也会把组合字符拆成裸基字。
 */
object SymbolCategories {

    /** 一个类目：序号（1 起，与展示的「1.常用」一致）、名称、符号表（表序 = 生成序）。 */
    data class Category(val index: Int, val label: String, val symbols: List<String>)

    /**
     * 解析 TSV。坏行（列数不对/序号非数字/名称为空/无符号）**跳过而不抛** —— 资产是
     * 生成物，但用户设备上的文件可能被替换/截断，页面该降级成少几个类目而不是崩 IME
     * （判据同 RareIndex.parse）。
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

    /** 每页符号数（8 列 × 4 行）。页数 = ⌈符号数 / 本值⌉（滑动翻页的页大小）。 */
    const val PAGE_SIZE = 32
}
