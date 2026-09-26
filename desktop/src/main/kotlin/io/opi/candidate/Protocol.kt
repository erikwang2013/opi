// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! NDJSON 行解析（线协议全貌见 Main.kt 头部注释）。

package io.opi.candidate

// ---------- 最小 JSON 解析（协议仅需字符串/数字/字符串数组，零额外依赖） ----------

internal sealed interface JVal {
    data class JStr(val v: String) : JVal
    data class JNum(val v: Long) : JVal
    data class JArr(val v: List<JVal>) : JVal
    data class JObj(val v: Map<String, JVal>) : JVal
}

/** 递归下降解析器：支持对象/字符串数组/数字/转义（\" \\uXXXX 等）。 */
private class JParse(private val s: String) {
    private var i = 0

    fun parse(): JVal? {
        skipWs()
        val v = parseValue()
        skipWs()
        return v
    }

    private fun skipWs() {
        while (i < s.length && s[i].isWhitespace()) i++
    }


    private fun parseValue(): JVal? {
        if (i >= s.length) return null
        return when (s[i]) {
            '"' -> JVal.JStr(parseString() ?: return null)
            '{' -> parseObject()
            '[' -> parseArray()
            else -> parseNumber()
        }
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
