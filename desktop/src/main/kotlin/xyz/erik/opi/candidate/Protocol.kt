// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! NDJSON 行解析（线协议全貌见 Main.kt 头部注释）。

package xyz.erik.opi.candidate

// ---------- 最小 JSON 解析（协议仅需字符串/数字/字符串数组，零额外依赖） ----------

internal sealed interface JVal {
    data class JStr(val v: String) : JVal
    data class JNum(val v: Long) : JVal
    /** 布尔（2026-10-20 加：控制面应答带 `"ok":true`）。此前协议里没有布尔，解析器只认字面 true/false。 */
    data class JBool(val v: Boolean) : JVal
    /** JSON `null`（2026-11-01 加：opi-clip 的 `pairing_finished.reason` 成功时为 null）。 */
    data object JNull : JVal
    data class JArr(val v: List<JVal>) : JVal
    data class JObj(val v: Map<String, JVal>) : JVal
}

/**
 * 容器嵌套上限，镜像 serde_json 的 `remaining_depth: 128`（判定在 JParse.nestedContainer）。
 * 递归下降的每一层栈都来自容器嵌套：不设上限则上万层 `[[[[…` 直接吃光线程栈。
 */
private const val MAX_DEPTH = 128

/** 递归下降解析器：支持对象/字符串数组/数字/转义（\" \\uXXXX 等）。 */
private class JParse(private val s: String) {
    private var i = 0

    /** 剩余可嵌套层数（镜像 serde_json 的 `remaining_depth`）。 */
    private var remaining = MAX_DEPTH

    fun parse(): JVal? {
        skipWs()
        val v = parseValue()
        skipWs()
        return v
    }

    /**
     * 整串解析：[parse] 之后必须已经吃完整串（尾部还有残渣 = 这份载荷坏了）。
     * 为什么另开一个入口而不是改 [parse]：行协议那边要的正是「容忍尾部」（一行进来的
     * 就是一行，宽容不吃亏），而载荷那边要的是「必须是整串」（截断的半个数组必须报失败，
     * 不能被当成合法前缀静默通过）。
     */
    fun parseWhole(): JVal? {
        val v = parse()
        skipWs()
        return if (i >= s.length) v else null
    }

    private fun skipWs() {
        while (i < s.length && s[i].isWhitespace()) i++
    }

    /** 精确吃下一个字面量（`true`/`false`）；不是它 → null，不动游标。 */
    private fun literal(text: String): Boolean? {
        if (!s.startsWith(text, i)) return null
        i += text.length
        return true
    }


    private fun parseValue(): JVal? {
        if (i >= s.length) return null
        return when (s[i]) {
            '"' -> JVal.JStr(parseString() ?: return null)
            '{' -> nestedContainer { parseObject() }
            '[' -> nestedContainer { parseArray() }
            't' -> literal("true")?.let { JVal.JBool(true) }
            'f' -> literal("false")?.let { JVal.JBool(false) }
            'n' -> literal("null")?.let { JVal.JNull }
            else -> parseNumber()
        }
    }

    /**
     * 递归下降的**唯一**入口收口处：进出容器时维护 [remaining]。
     * 镜像 serde_json 的 `check_recursion!`（先减再判 0）—— 最多 127 层容器；
     * 第 128 层返回 null（该行丢弃），而不是让递归继续吃栈。
     * 退出时必须归还，否则同一层的兄弟节点会从上一分支的深度起算并被误判。
     */
    private fun nestedContainer(parse: () -> JVal?): JVal? {
        if (--remaining == 0) return null
        val v = parse()
        remaining++
        return v
    }

    private fun parseObject(): JVal.JObj? {
        expect('{') ?: return null
        val out = LinkedHashMap<String, JVal>()
        skipWs()
        if (i < s.length && s[i] == '}') { i++; return JVal.JObj(out) }
        while (true) {
            skipWs()
            val key = parseString() ?: return null
            skipWs()
            expect(':') ?: return null
            skipWs()
            out[key] = parseValue() ?: return null
            skipWs()
            if (i >= s.length) return null
            when (s[i]) {
                ',' -> i++
                '}' -> { i++; return JVal.JObj(out) }
                else -> return null
            }
        }
    }

    private fun parseArray(): JVal.JArr? {
        expect('[') ?: return null
        val out = ArrayList<JVal>()
        skipWs()
        if (i < s.length && s[i] == ']') { i++; return JVal.JArr(out) }
        while (true) {
            skipWs()
            out.add(parseValue() ?: return null)
            skipWs()
            if (i >= s.length) return null
            when (s[i]) {
                ',' -> i++
                ']' -> { i++; return JVal.JArr(out) }
                else -> return null
            }
        }
    }

    private fun parseString(): String? {
        expect('"') ?: return null
        val sb = StringBuilder()
        while (i < s.length) {
            when (val c = s[i++]) {
                '"' -> return sb.toString()
                '\\' -> {
                    if (i >= s.length) return null
                    when (val e = s[i++]) {
                        '"' -> sb.append('"')
                        '\\' -> sb.append('\\')
                        '/' -> sb.append('/')
                        'n' -> sb.append('\n')
                        't' -> sb.append('\t')
                        'r' -> sb.append('\r')
                        'u' -> {
                            if (i + 4 > s.length) return null
                            // 非法十六进制（如 \uZZZZ）→ 解析失败返回 null，而非抛
                            // NumberFormatException 杀死管道服务器线程。
                            val hex = s.substring(i, i + 4).toIntOrNull(16) ?: return null
                            sb.append(hex.toChar())
                            i += 4
                        }
                        else -> return null
                    }
                }
                else -> sb.append(c)
            }
        }
        return null
    }

    private fun parseNumber(): JVal.JNum? {
        val start = i
        while (i < s.length && (s[i].isDigit() || s[i] == '-')) i++
        if (i == start) return null
        // 非法/超范围数字 → 返回 null（该行丢弃），而非抛 NumberFormatException。
        return JVal.JNum(s.substring(start, i).toLongOrNull() ?: return null)
    }

    private fun expect(c: Char): Boolean = if (i < s.length && s[i] == c) { i++; true } else false
}

/** 解析一行消息为对象（失败返回 null）。 */
internal fun parseLine(text: String): Map<String, JVal>? =
    (JParse(text).parse() as? JVal.JObj)?.v

// ---------- JSON 字符串**编码**（解析的反向；commit_text 载荷要用它） ----------

/**
 * 把一个字符串编成合法的 JSON 字符串字面量（**含两侧引号**）。
 *
 * 为什么必须有它：符号页要发的符号里就有 `"` 与 `\`（英文类目整行都是），
 * 直接拼会让收到的整行 JSON 解析失败 —— 而对端（serde_json）解析失败即静默丢弃
 * 那行，表现是「点了某个符号没反应」，查起来毫无线索。
 *
 * 口径与 serde_json 的写出对齐：非 ASCII **原样**（两边约定走 UTF-8，不转义成
 * `\uXXXX`，与 WireContractTest 里「中文不转义」的实测一致）；必须转义的只有
 * `"` `\` 与 < 0x20 的控制字符（`\b \f \n \r \t`，其余走 `\u00XX`）。
 * `/` 不转义（JSON 允许，serde_json 也不转）。
 */
internal fun jsonString(s: String): String {
    val sb = StringBuilder(s.length + 2)
    sb.append('"')
    for (c in s) {
        when {
            c == '"' -> sb.append("\\\"")
            c == '\\' -> sb.append("\\\\")
            c == '\b' -> sb.append("\\b")
            c == '\u000C' -> sb.append("\\f")
            c == '\n' -> sb.append("\\n")
            c == '\r' -> sb.append("\\r")
            c == '\t' -> sb.append("\\t")
            c < ' ' -> sb.append("\\u%04x".format(c.code))
            else -> sb.append(c)
        }
    }
    sb.append('"')
    return sb.toString()
}

// ---------- JSON 字符串数组**解码**（`opi-ctl search` 的载荷，2026-10-13） ----------

/**
 * 解析**一整串** JSON 字符串数组（`["…","…"]`）；形状不对 → null。
 *
 * 与 [parseLine] 的分工：那个是行协议（对象 + 容忍尾部），这个是搜索载荷。二者共用
 * [JParse]，所以转义口径必然一致 —— 而 `opi-ctl` 侧只做了首尾括号的轻量形状检查
 * （它没有 serde_json，手写半个词法器必然与这份漂），真正的解析在这里。
 *
 * **返回 null 与空列表不是一回事**：null = 载荷坏了（界面上如实报失败），
 * 空列表 = 引擎说没搜到（界面报「没有匹配」）。所以坏载荷绝不许降级成空列表。
 */
internal fun parseJsonStringArray(text: String): List<String>? {
    val arr = JParse(text).parseWhole() as? JVal.JArr ?: return null
    val out = ArrayList<String>(arr.v.size)
    for (e in arr.v) out.add((e as? JVal.JStr)?.v ?: return null)
    return out
}
