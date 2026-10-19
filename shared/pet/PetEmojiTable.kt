// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.pet

/**
 * `shared/pet/emoji-font/` 里两份 TSV 的**纯 JVM 解析器**（Android/桌面/测试共用）。
 *
 * 文件由 `scripts/gen_pet_emoji_table.py` 生成；调用方只负责把字节读成字符串
 * （Android = assets，桌面 = classpath）。这里不碰 IO，也不认识平台路径。
 *
 * `map.tsv` 的 `codepoint` 列只是十二个形态的**稳定内部 ID**（BMP PUA），
 * 不再对应任何内置字体，也**绝不**上屏到输入框 —— 自有表情走 GIF/PNG/文字标签。
 *
 * 解析是**严格**的：表头不对、列数不对、ID 不在 PUA、短码重复都抛
 * [IllegalArgumentException]。内置数据坏掉要响亮地死，不能悄悄少两行。
 */
object PetEmojiTable {

    /** 资源目录名（Android assets 与桌面 classpath 同名；目录名是历史遗留）。 */
    const val DIR = "pet-font"
    const val MAP_FILE = "map.tsv"
    const val SHORTCODES_FILE = "shortcodes.tsv"

    private const val MAP_HEADER = "mood\tcodepoint\tfile\tlabel_zh\tshortcode"
    private const val SHORT_HEADER = "shortcode\tfile\tlabel_zh"

    /** `map.tsv` 的一行（情绪名、稳定 ID、贴纸文件名、中文标签、短码）。 */
    data class Glyph(
        val mood: String,
        val codepoint: Int,
        val file: String,
        val label: String,
        val shortcode: String,
    )

    /** `shortcodes.tsv` 的一行。 */
    data class Shortcode(val shortcode: String, val file: String, val label: String)

    /** 解析 `map.tsv`；顺序即文件顺序（= `PetMood` 枚举序）。 */
    fun parseMap(tsv: String): List<Glyph> {
        val rows = table(tsv, MAP_HEADER, 5)
        val glyphs = rows.map { cells ->
            val codepoint = parseCodepoint(cells[1])
            require(cells[2].isNotBlank()) { "map.tsv: ${cells[0]} 的 file 为空" }
            require(cells[4].matches(Regex("^:opi_[a-z0-9_]+:$"))) {
                "map.tsv: ${cells[0]} 的 shortcode 形状不对：${cells[4]}"
            }
            Glyph(cells[0], codepoint, cells[2], cells[3], cells[4])
        }
        require(glyphs.isNotEmpty()) { "map.tsv: 没有数据行" }
        require(glyphs.map { it.mood }.toSet().size == glyphs.size) { "map.tsv: mood 重复" }
        require(glyphs.map { it.codepoint }.toSet().size == glyphs.size) { "map.tsv: ID 重复" }
        require(glyphs.map { it.shortcode }.toSet().size == glyphs.size) { "map.tsv: 短码重复" }
        return glyphs
    }

    /** 解析 `shortcodes.tsv`；顺序即文件顺序。 */
    fun parseShortcodes(tsv: String): List<Shortcode> {
        val rows = table(tsv, SHORT_HEADER, 3)
        val codes = rows.map { cells ->
            require(cells[0].matches(Regex("^:opi_[a-z0-9_]+:$"))) {
                "shortcodes.tsv: 短码形状不对：${cells[0]}"
            }
            require(cells[1].isNotBlank()) { "shortcodes.tsv: ${cells[0]} 的 file 为空" }
            Shortcode(cells[0], cells[1], cells[2])
        }
        require(codes.isNotEmpty()) { "shortcodes.tsv: 没有数据行" }
        require(codes.map { it.shortcode }.toSet().size == codes.size) {
            "shortcodes.tsv: 短码重复"
        }
        return codes
    }

    private fun table(tsv: String, header: String, columns: Int): List<List<String>> {
        val lines = tsv.removeSuffix("\r\n").removeSuffix("\n").split('\n')
        require(lines.isNotEmpty() && lines[0] == header) {
            "表头不对：期望 \"$header\"，实际 \"${lines.firstOrNull()}\""
        }
        return lines.drop(1).mapIndexed { index, line ->
            val cells = line.split('\t')
            require(cells.size == columns) {
                "第 ${index + 2} 行应有 $columns 列，实际 ${cells.size} 列：$line"
            }
            cells
        }
    }

    /** `U+E000` → `0xE000`；只接受 BMP PUA（生成器分配的稳定 ID 范围）。 */
    private fun parseCodepoint(value: String): Int {
        require(value.matches(Regex("^U\\+[0-9A-F]{4}$"))) { "ID 形状不对：$value" }
        val code = value.removePrefix("U+").toInt(16)
        require(code in 0xE000..0xF8FF) { "ID 不在 BMP PUA：$value" }
        return code
    }
}
