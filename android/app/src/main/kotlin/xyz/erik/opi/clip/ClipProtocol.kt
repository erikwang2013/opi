// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.clip

/**
 * opi-clip 的宿主侧协议面（纯 JVM，无 android.*）：FFI 的 JSON 出口 → Kotlin 模型，
 * 以及反向的配置 JSON。
 *
 * 手写解析而不引 `org.json`：JVM 单测里 android.jar 的 org.json 只有空壳
 * （一调就抛 "not mocked"，见 `VersionCheck.kt` 头注）；这里只吃自家 serde 的输出，
 * schema 固定，一份最小 JSON 解析器足够。事件 JSON 上限 1 MiB、文本上限 256 KiB
 * 由 Rust 侧保证。
 */

// ---------- 模型 ----------

/** `peers()` 的一项。`instanceId` 是 32 位十六进制，配对接口原样回传。 */
internal data class ClipPeer(
    val instanceId: String,
    val name: String,
    val address: String,
    val fingerprint: String,
    val trusted: Boolean,
    val connected: Boolean,
)

/** `status()` 的投影；未启动时 `instanceId`/`fingerprint` 为 null。 */
internal data class ClipStatus(
    val running: Boolean,
    val instanceId: String?,
    val fingerprint: String?,
    val deviceName: String,
    val peers: Int,
)

/** 需要用户核对的 6 位 SAS。`incoming` = 这次配对由对端发起。 */
internal data class ClipPairingPrompt(
    val instanceId: String,
    val sas: String,
    val incoming: Boolean,
)

/** `ClipEvent` 的宿主投影（真源 `crates/opi-clip/src/types.rs`）。 */
internal sealed interface ClipEvent {
    data class PeerDiscovered(val peer: ClipPeer) : ClipEvent
    data class PeerOffline(val instanceId: String) : ClipEvent
    data class PairingPrompt(val instanceId: String, val sas: String, val incoming: Boolean) : ClipEvent
    data class PairingFinished(val instanceId: String, val trusted: Boolean, val reason: String?) : ClipEvent
    data class ClipboardReceived(val instanceId: String, val text: String) : ClipEvent
    data class SendFailed(val instanceId: String) : ClipEvent
    data class Error(val message: String) : ClipEvent
    data class Unknown(val type: String) : ClipEvent
}

// ---------- 解码 ----------

/** 解析一条事件 JSON；格式坏 → null（调用方忽略这一条）。 */
internal fun parseClipEvent(json: String): ClipEvent? {
    val root = runCatching { ClipJson.parse(json) as? Map<*, *> }.getOrNull() ?: return null
    val type = root.str("type") ?: return null
    return when (type) {
        "peer_discovered" -> (root["peer"] as? Map<*, *>)?.let { ClipEvent.PeerDiscovered(peerOf(it)) }
        "peer_offline" -> root.str("instance_id")?.let { ClipEvent.PeerOffline(it) }
        "pairing_prompt" -> {
            val id = root.str("instance_id")
            val sas = root.str("sas")
            if (id == null || sas == null) null
            else ClipEvent.PairingPrompt(id, sas, root.bool("incoming"))
        }
        "pairing_finished" -> root.str("instance_id")?.let {
            ClipEvent.PairingFinished(it, root.bool("trusted"), root.str("reason"))
        }
        "clipboard_received" -> {
            val id = root.str("instance_id")
            val text = root.str("text")
            if (id == null || text == null) null else ClipEvent.ClipboardReceived(id, text)
        }
        "send_failed" -> root.str("instance_id")?.let { ClipEvent.SendFailed(it) }
        "error" -> ClipEvent.Error(root.str("message") ?: "")
        else -> ClipEvent.Unknown(type)
    }
}

/** `peers()` → 列表；非数组/坏项整份 → 空（宁少不崩）。 */
internal fun parseClipPeers(json: String): List<ClipPeer> {
    val root = runCatching { ClipJson.parse(json) as? List<*> }.getOrNull() ?: return emptyList()
    return root.mapNotNull { (it as? Map<*, *>)?.let(::peerOf) }
}

/** `status()` → 投影；格式坏 → null。 */
internal fun parseClipStatus(json: String): ClipStatus? {
    val root = runCatching { ClipJson.parse(json) as? Map<*, *> }.getOrNull() ?: return null
    return ClipStatus(
        running = root.bool("running"),
        instanceId = root.str("instance_id"),
        fingerprint = root.str("fingerprint"),
        deviceName = root.str("device_name") ?: "",
        peers = (root["peers"] as? Number)?.toInt() ?: 0,
    )
}

private fun peerOf(raw: Map<*, *>): ClipPeer = ClipPeer(
    instanceId = raw.str("instance_id") ?: "",
    name = raw.str("name") ?: "",
    address = raw.str("address") ?: "",
    fingerprint = raw.str("fingerprint") ?: "",
    trusted = raw.bool("trusted"),
    connected = raw.bool("connected"),
)

private fun Map<*, *>.str(key: String): String? = this[key] as? String
private fun Map<*, *>.bool(key: String): Boolean = this[key] as? Boolean ?: false

// ---------- 编码 ----------

/** `ClipConfig` 的 JSON（字段名与 `crates/opi-clip/src/config.rs` 一致）。 */
internal fun clipConfigJson(
    dataDir: String,
    deviceName: String,
    discoveryPort: Int = 45888,
): String = buildString {
    append("{\"data_dir\":").append(clipJsonString(dataDir))
    append(",\"tcp_port\":0,\"discovery_port\":").append(discoveryPort)
    append(",\"device_name\":").append(clipJsonString(deviceName)).append('}')
}

/** 只用于配置字符串（路径/设备名）；转义控制字符与引号，非 ASCII 原样输出。 */
internal fun clipJsonString(value: String): String = buildString {
    append('"')
    for (c in value) {
        when {
            c == '"' -> append("\\\"")
            c == '\\' -> append("\\\\")
            c == '\n' -> append("\\n")
            c == '\r' -> append("\\r")
            c == '\t' -> append("\\t")
            c < ' ' -> append("\\u").append(c.code.toString(16).padStart(4, '0'))
            else -> append(c)
        }
    }
    append('"')
}

// ---------- 最小 JSON 解析器（固定 schema 消费用） ----------

internal object ClipJson {
    fun parse(text: String): Any? {
        val p = Parser(text)
        val value = p.value()
        p.skipWs()
        require(p.atEnd) { "JSON 末尾有多余内容" }
        return value
    }

    private class Parser(private val s: String) {
        private var i = 0

        val atEnd: Boolean get() = i >= s.length

        fun skipWs() {
            while (i < s.length && s[i].isWhitespace()) i++
        }

        fun value(): Any? {
            skipWs()
            require(i < s.length) { "JSON 意外结束" }
            return when (val c = s[i]) {
                '{' -> obj()
                '[' -> arr()
                '"' -> str()
                't' -> literal("true", true)
                'f' -> literal("false", false)
                'n' -> literal("null", null)
                else -> if (c == '-' || c.isDigit()) num() else throw IllegalArgumentException("JSON 非法字符 '$c'")
            }
        }

        private fun obj(): Map<String, Any?> {
            i++ // {
            val out = LinkedHashMap<String, Any?>()
            skipWs()
            if (peek('}')) { i++; return out }
            while (true) {
                skipWs()
                val key = str()
                skipWs()
                expect(':')
                out[key] = value()
                skipWs()
                when {
                    peek(',') -> i++
                    peek('}') -> { i++; return out }
                    else -> throw IllegalArgumentException("JSON 对象缺 , 或 }")
                }
            }
        }

        private fun arr(): List<Any?> {
            i++ // [
            val out = ArrayList<Any?>()
            skipWs()
            if (peek(']')) { i++; return out }
            while (true) {
                out += value()
                skipWs()
                when {
                    peek(',') -> i++
                    peek(']') -> { i++; return out }
                    else -> throw IllegalArgumentException("JSON 数组缺 , 或 ]")
                }
            }
        }

        private fun str(): String {
            expect('"')
            val sb = StringBuilder()
            while (i < s.length) {
                when (val c = s[i++]) {
                    '"' -> return sb.toString()
                    '\\' -> {
                        require(i < s.length) { "JSON 转义未完成" }
                        when (val e = s[i++]) {
                            '"' -> sb.append('"')
                            '\\' -> sb.append('\\')
                            '/' -> sb.append('/')
                            'b' -> sb.append('\b')
                            'f' -> sb.append('\u000C')
                            'n' -> sb.append('\n')
                            'r' -> sb.append('\r')
                            't' -> sb.append('\t')
                            'u' -> {
                                require(i + 4 <= s.length) { "JSON \\u 不完整" }
                                sb.append(s.substring(i, i + 4).toInt(16).toChar())
                                i += 4
                            }
                            else -> throw IllegalArgumentException("JSON 非法转义 \\$e")
                        }
                    }
                    else -> sb.append(c)
                }
            }
            throw IllegalArgumentException("JSON 字符串未闭合")
        }

        private fun num(): Double {
            val start = i
            if (peek('-')) i++
            while (i < s.length && s[i].isDigit()) i++
            if (peek('.')) {
                i++
                while (i < s.length && s[i].isDigit()) i++
            }
            if (peek('e') || peek('E')) {
                i++
                if (peek('+') || peek('-')) i++
                while (i < s.length && s[i].isDigit()) i++
            }
            return s.substring(start, i).toDouble()
        }

        private fun literal(word: String, value: Any?): Any? {
            require(s.startsWith(word, i)) { "JSON 非法字面量" }
            i += word.length
            return value
        }

        private fun expect(c: Char) {
            skipWs()
            require(i < s.length && s[i] == c) { "JSON 期望 '$c'" }
            i++
        }

        private fun peek(c: Char): Boolean = i < s.length && s[i] == c
    }
}
