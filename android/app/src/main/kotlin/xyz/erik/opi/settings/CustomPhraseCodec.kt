// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

/**
 * 一条自定义短语：触发字母 + 上屏文本。
 *
 * 触发字母合同：**ASCII 字母、大小写不敏感**（解码时统一小写；引擎按小写匹配整段缓冲）。
 * 文本非空。这条判定在 [CustomPhraseCodec.validTrigger]，设置页与导入共用同一份。
 */
data class CustomPhrase(val trigger: String, val text: String)

/** [CustomPhraseCodec.decode] 的结果。[Invalid.reason] 直接面向用户。 */
sealed interface CustomPhraseDecode {
    /** [enabled] 只在宿主文件（含 `enabled` 字段）里出现；导入文件通常为 null。 */
    data class Ok(val phrases: List<CustomPhrase>, val enabled: Boolean?) : CustomPhraseDecode

    data class Invalid(val reason: String) : CustomPhraseDecode
}

/**
 * 自定义短语的编解码（2026-10-23）：**纯 Kotlin、无 Android 依赖**，JVM 单测直接跑。
 *
 * 三类文本共用一套判定：
 *  · 规范化 JSON（引擎合同）`{"version":1,"phrases":[{"trigger":"dz","text":"地址"}]}`；
 *  · 宿主文件 = 规范化 JSON + `"enabled"`（宿主是启用态的唯一真源）；
 *  · 导入格式：上面的 JSON（对象/数组、对象/二元数组条目）、TSV、CSV（可带引号）、
 *    逐行 `字母=短语` 或 `字母<空白>短语`。
 *
 * 规则只有一份：解析先统一成 `List<CustomPhrase>`，再由 [merge] 合并；任何一条非法
 * **整份拒收**（不部分写入）。JSON 解析器是本文件里那个最小的递归下降实现 —— 项目没有
 * JSON 依赖，为几十行引入一条新依赖不值当。
 */
object CustomPhraseCodec {

    const val VERSION = 1

    /** 列表分页大小（设置页「自定义短语」每页条数）。 */
    const val PAGE_SIZE = 10

    /**
     * 引擎的硬上限（镜像 `engine-core/src/custom_phrases.rs` 的
     * `MAX_CUSTOM_PHRASES` / `MAX_TRIGGER_LEN`，同值）。
     *
     * 为什么宿主也要挡：引擎对越限**整表拒收**。宿主若照单全收，用户能存下一份
     * 引擎永远吃不下（或换条 trigger 就再也推不进去）的表 —— 屏幕上看得到、输入时
     * 却永远不出现，且没有任何一处会说是哪条坏了。
     */
    const val MAX_PHRASES = 2000
    const val MAX_TRIGGER_LEN = 32

    /** 触发字母是否合法：非空、纯 ASCII 字母（大小写都收，解码时统一小写）。 */
    fun validTrigger(raw: String): Boolean =
        raw.isNotEmpty() && raw.all { it in 'a'..'z' || it in 'A'..'Z' }

    /** 引擎合同 JSON（**不含** enabled —— 引擎只认这个形状）。 */
    fun encodeCanonical(phrases: List<CustomPhrase>): String =
        """{"version":$VERSION,"phrases":[${phrases.joinToString(",") { entryJson(it) }}]}"""

    /** 宿主文件 JSON（规范化形状 + enabled）。 */
    fun encodeState(enabled: Boolean, phrases: List<CustomPhrase>): String =
        """{"version":$VERSION,"enabled":$enabled,"phrases":[${phrases.joinToString(",") { entryJson(it) }}]}"""

    /** TSV：每行 `字母<TAB>短语`（末尾带换行；表头不写，导入侧也不认表头）。 */
    fun encodeTsv(phrases: List<CustomPhrase>): String =
        if (phrases.isEmpty()) "" else phrases.joinToString("\n") { "${it.trigger}\t${it.text}" } + "\n"

    /**
     * 自动识别并解析。JSON（首字符 `{`/`[`）优先；否则逐行按**最早出现的分隔符**判定：
     * 制表符 = TSV、等号 = `字母=短语`、逗号 = CSV、其余空白 = `字母 短语`。
     * 空行跳过（导出文件末尾的换行不该变成「非法条目」）。
     */
    fun decode(raw: String): CustomPhraseDecode {
        val text = raw.removePrefix("\uFEFF").trim()
        if (text.isEmpty()) return CustomPhraseDecode.Invalid("文件是空的")
        return if (text.startsWith("{") || text.startsWith("[")) decodeJson(text) else decodeLines(text)
    }

    /**
     * 合并导入（后导入的 trigger 覆盖已有；同一份导入内也以后出现者为准）。
     * added = 新 trigger 数；updated = 已有 trigger 且文本**确实变了**的数；
     * 文本一字不差的重复条目既不计新增也不计更新。
     */
    data class MergeResult(val phrases: List<CustomPhrase>, val added: Int, val updated: Int)

    fun merge(existing: List<CustomPhrase>, imported: List<CustomPhrase>): MergeResult {
        // 输出一律是规范化形状（小写、无重复 trigger）：引擎整表拒收重复键，
        // 调用方即便递进来未规范化的表，也不能把重复原样留下
        val out = normalize(existing).toMutableList()
        val index = HashMap<String, Int>(out.size)
        out.forEachIndexed { i, p -> index[p.trigger.lowercase()] = i }
        var added = 0
        var updated = 0
        for (p in normalize(imported)) {
            val key = p.trigger.lowercase()
            val at = index[key]
            when {
                at == null -> {
                    index[key] = out.size
                    out += p
                    added++
                }
                out[at].text != p.text -> {
                    out[at] = p
                    updated++
                }
            }
        }
        return MergeResult(out, added, updated)
    }

    /** 去重并规范化（trigger 小写为准，后出现者覆盖文本；位置取首次出现处）。 */
    fun normalize(phrases: List<CustomPhrase>): List<CustomPhrase> {
        val byTrigger = LinkedHashMap<String, CustomPhrase>(phrases.size)
        for (p in phrases) {
            val t = p.trigger.lowercase()
            byTrigger[t] = if (p.trigger == t) p else p.copy(trigger = t)
        }
        return byTrigger.values.toList()
    }

    // ---------- 分页（设置页用；纯算术，单测钉边界） ----------

    /** 页数：空表也算 1 页（页码显示「第 1 / 1 页」而不是 0）。 */
    fun pageCount(total: Int, pageSize: Int = PAGE_SIZE): Int =
        if (total <= 0) 1 else (total + pageSize - 1) / pageSize

    fun clampPage(page: Int, total: Int, pageSize: Int = PAGE_SIZE): Int =
        page.coerceIn(0, pageCount(total, pageSize) - 1)

    fun pageSlice(phrases: List<CustomPhrase>, page: Int, pageSize: Int = PAGE_SIZE): List<CustomPhrase> {
        val p = clampPage(page, phrases.size, pageSize)
        return phrases.drop(p * pageSize).take(pageSize)
    }

    // ---------- 编码细节 ----------

    private fun entryJson(p: CustomPhrase): String =
        """{"trigger":"${escape(p.trigger)}","text":"${escape(p.text)}"}"""

    private fun escape(s: String): String {
        val sb = StringBuilder(s.length + 8)
        for (c in s) when (c) {
            '"' -> sb.append("\\\"")
            '\\' -> sb.append("\\\\")
            '\n' -> sb.append("\\n")
            '\r' -> sb.append("\\r")
            '\t' -> sb.append("\\t")
            else -> if (c < ' ') {
                sb.append("\\u").append(c.code.toString(16).padStart(4, '0'))
            } else {
                sb.append(c)
            }
        }
        return sb.toString()
    }

    // ---------- 解码：JSON ----------

    private fun decodeJson(text: String): CustomPhraseDecode {
        val root = try {
            Json.parse(text)
        } catch (e: IllegalArgumentException) {
            return CustomPhraseDecode.Invalid("JSON 格式错误：${e.message}")
        }
        val entries: List<Any?>
        var enabled: Boolean? = null
        when (root) {
            is Map<*, *> -> {
                val version = root["version"]
                if (version != null && (version !is Number || version.toDouble() != VERSION.toDouble())) {
                    return CustomPhraseDecode.Invalid("不支持的文件版本：$version")
                }
                val list = root["phrases"] ?: return CustomPhraseDecode.Invalid("缺少 phrases 字段")
                if (list !is List<*>) return CustomPhraseDecode.Invalid("phrases 必须是数组")
                entries = list
                enabled = root["enabled"] as? Boolean
            }
            is List<*> -> entries = root
            else -> return CustomPhraseDecode.Invalid("顶层必须是 JSON 对象或数组")
        }
        val out = ArrayList<CustomPhrase>(entries.size)
        entries.forEachIndexed { i, e ->
            try {
                out += jsonEntry(e, i)
            } catch (ex: IllegalArgumentException) {
                return CustomPhraseDecode.Invalid(ex.message ?: "第 ${i + 1} 条格式不对")
            }
        }
        val normalized = normalize(out)
        if (normalized.size > MAX_PHRASES) {
            return CustomPhraseDecode.Invalid("短语达到上限（最多 $MAX_PHRASES 条）")
        }
        // 空数组是合法的：宿主文件在「关掉开关且删光短语」时就是 phrases:[] ——
        // 这里拒收会让那次保存的 enabled=false 在重启后悄悄变回 true。
        return CustomPhraseDecode.Ok(normalized, enabled)
    }

    private fun jsonEntry(e: Any?, index: Int): CustomPhrase {
        val label = "第 ${index + 1} 条："
        val (trigger, text) = when (e) {
            is Map<*, *> -> e["trigger"] to e["text"]
            is List<*> -> {
                if (e.size != 2) throw IllegalArgumentException("${label}应为 [字母, 短语]")
                e[0] to e[1]
            }
            else -> throw IllegalArgumentException("${label}格式不对")
        }
        if (trigger !is String || text !is String) {
            throw IllegalArgumentException("${label}字母和短语都必须是字符串")
        }
        return checked(label, trigger, text)
    }

    // ---------- 解码：行式（TSV / CSV / = / 空白） ----------

    private fun decodeLines(text: String): CustomPhraseDecode {
        val out = ArrayList<CustomPhrase>()
        val lines = text.lines()
        for ((i, raw) in lines.withIndex()) {
            val line = raw.trim()
            if (line.isEmpty()) continue
            try {
                out += lineEntry(line)
            } catch (e: IllegalArgumentException) {
                return CustomPhraseDecode.Invalid("第 ${i + 1} 行：${e.message}")
            }
        }
        if (out.isEmpty()) return CustomPhraseDecode.Invalid("没有可导入的短语")
        val normalized = normalize(out)
        if (normalized.size > MAX_PHRASES) {
            return CustomPhraseDecode.Invalid("短语达到上限（最多 $MAX_PHRASES 条）")
        }
        return CustomPhraseDecode.Ok(normalized, null)
    }

    private fun lineEntry(line: String): CustomPhrase {
        // 以引号开头的行一定是 CSV（引号内可能就有逗号，先找分隔符会切错）
        if (line.startsWith("\"")) return csvEntry(line)
        var sepAt = -1
        var sep = ' '
        for ((i, c) in line.withIndex()) {
            if (c == '\t' || c == '=' || c == ',' || c.isWhitespace()) {
                sepAt = i
                sep = c
                break
            }
        }
        if (sepAt < 0) {
            throw IllegalArgumentException("缺少分隔符（应为 字母=短语 / 字母 短语 / TSV / CSV）")
        }
        if (sep == ',') return csvEntry(line)
        val trigger = line.substring(0, sepAt)
        val text = line.substring(sepAt + 1)
        return checked("", trigger, text)
    }

    /** CSV 单行：支持引号字段与 `""` 转义；必须恰好两列。 */
    private fun csvEntry(line: String): CustomPhrase {
        val fields = ArrayList<String>(2)
        val sb = StringBuilder()
        var i = 0
        var quoted = false
        while (i < line.length) {
            val c = line[i]
            if (quoted) {
                if (c == '"') {
                    if (i + 1 < line.length && line[i + 1] == '"') {
                        sb.append('"')
                        i += 2
                        continue
                    }
                    quoted = false
                    i++
                    continue
                }
                sb.append(c)
                i++
                continue
            }
            when (c) {
                '"' -> {
                    if (sb.isNotEmpty()) throw IllegalArgumentException("引号只能出现在字段开头")
                    quoted = true
                }
                ',' -> {
                    fields += sb.toString()
                    sb.setLength(0)
                }
                else -> sb.append(c)
            }
            i++
        }
        if (quoted) throw IllegalArgumentException("引号未闭合")
        fields += sb.toString()
        if (fields.size != 2) throw IllegalArgumentException("应为两列（字母,短语）")
        return checked("", fields[0], fields[1])
    }

    private fun checked(label: String, trigger: String, text: String): CustomPhrase {
        val t = trigger.trim()
        if (t.isEmpty()) throw IllegalArgumentException("${label}字母不能为空")
        if (!validTrigger(t)) throw IllegalArgumentException("${label}字母只能是英文字母")
        if (t.length > MAX_TRIGGER_LEN) {
            throw IllegalArgumentException("${label}字母过长（上限 $MAX_TRIGGER_LEN 个）")
        }
        val x = text.trim()
        if (x.isEmpty()) throw IllegalArgumentException("${label}短语不能为空")
        return CustomPhrase(t.lowercase(), x)
    }

    // ---------- 最小 JSON 解析器 ----------
    //
    // 只服务本文件的两种输入（规范化对象 / 外部导入的数组）。返回
    // Map/List/String/Double/Boolean/null。**不是通用 JSON 库**：数字一律 Double，
    // 不校验前导零这类细节 —— 解析失败才重要，这里失败 = 整份拒收。

    private object Json {
        fun parse(text: String): Any? {
            val p = Parser(text)
            val v = p.value()
            p.skipWs()
            if (!p.atEnd()) throw IllegalArgumentException("多余内容（位置 ${p.pos}）")
            return v
        }

        /** 嵌套上限：`[[[[…` 一兆个方括号会让递归下降栈溢出（Error，不是可捕获的解析失败）。 */
        private const val MAX_DEPTH = 64

        private class Parser(private val s: String) {
            var pos = 0
            private var depth = 0

            fun atEnd(): Boolean = pos >= s.length

            fun skipWs() {
                while (pos < s.length && s[pos].isWhitespace()) pos++
            }

            private fun peek(): Char {
                if (atEnd()) throw IllegalArgumentException("意外的结尾")
                return s[pos]
            }

            fun value(): Any? {
                skipWs()
                return when (peek()) {
                    '{' -> nested { obj() }
                    '[' -> nested { arr() }
                    '"' -> str()
                    't' -> {
                        expect("true"); true
                    }
                    'f' -> {
                        expect("false"); false
                    }
                    'n' -> {
                        expect("null"); null
                    }
                    else -> num()
                }
            }

            private fun nested(block: () -> Any?): Any? {
                if (++depth > MAX_DEPTH) throw IllegalArgumentException("嵌套过深（上限 $MAX_DEPTH 层）")
                try {
                    return block()
                } finally {
                    depth--
                }
            }

            private fun obj(): Map<String, Any?> {
                pos++ // {
                val map = LinkedHashMap<String, Any?>()
                skipWs()
                if (peek() == '}') {
                    pos++
                    return map
                }
                while (true) {
                    skipWs()
                    if (peek() != '"') throw IllegalArgumentException("键必须是字符串（位置 $pos）")
                    val key = str()
                    skipWs()
                    if (peek() != ':') throw IllegalArgumentException("缺少冒号（位置 $pos）")
                    pos++
                    map[key] = value()
                    skipWs()
                    when (peek()) {
                        ',' -> pos++
                        '}' -> {
                            pos++
                            return map
                        }
                        else -> throw IllegalArgumentException("缺少逗号或 }（位置 $pos）")
                    }
                }
            }

            private fun arr(): List<Any?> {
                pos++ // [
                val list = ArrayList<Any?>()
                skipWs()
                if (peek() == ']') {
                    pos++
                    return list
                }
                while (true) {
                    list += value()
                    skipWs()
                    when (peek()) {
                        ',' -> pos++
                        ']' -> {
                            pos++
                            return list
                        }
                        else -> throw IllegalArgumentException("缺少逗号或 ]（位置 $pos）")
                    }
                }
            }

            private fun str(): String {
                pos++ // "
                val sb = StringBuilder()
                while (true) {
                    if (atEnd()) throw IllegalArgumentException("字符串未闭合")
                    when (val c = s[pos++]) {
                        '"' -> return sb.toString()
                        '\\' -> {
                            if (atEnd()) throw IllegalArgumentException("转义不完整")
                            when (val e = s[pos++]) {
                                '"' -> sb.append('"')
                                '\\' -> sb.append('\\')
                                '/' -> sb.append('/')
                                'b' -> sb.append('\b')
                                'f' -> sb.append('\u000C')
                                'n' -> sb.append('\n')
                                'r' -> sb.append('\r')
                                't' -> sb.append('\t')
                                'u' -> {
                                    if (pos + 4 > s.length) throw IllegalArgumentException("\\u 转义不完整")
                                    val hex = s.substring(pos, pos + 4)
                                    val code = hex.toIntOrNull(16)
                                        ?: throw IllegalArgumentException("\\u 转义非法：$hex")
                                    sb.append(code.toChar())
                                    pos += 4
                                }
                                else -> throw IllegalArgumentException("未知转义 \\$e")
                            }
                        }
                        else -> sb.append(c)
                    }
                }
            }

            private fun num(): Double {
                val start = pos
                if (peek() == '-') pos++
                while (!atEnd() && (s[pos].isDigit() || s[pos] in ".eE+-")) pos++
                val raw = s.substring(start, pos)
                return raw.toDoubleOrNull() ?: throw IllegalArgumentException("非法数字：$raw")
            }

            private fun expect(word: String) {
                if (!s.startsWith(word, pos)) throw IllegalArgumentException("非法值（位置 $pos）")
                pos += word.length
            }
        }
    }
}
