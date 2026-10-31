// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 跨设备剪贴板的**纯逻辑**层：事件解析 / UI 状态归约 / 控制面快照编解码。
//!
//! 这里刻意不碰 FFI、AWT、文件与线程 —— 全是可单测的函数。隐私边界也在这一层：
//! `clipboard_received` 的 `text` 只允许**立刻**交给剪贴板写入方，绝不进 UI 状态
//! （[ClipUiState] 里没有文本字段），更不落盘/打日志。

package xyz.erik.opi.candidate

import java.util.Locale

// ---------- 设备与模式 ----------

internal data class ClipPeer(
    val instanceId: String,
    val name: String,
    val address: String,
    val trusted: Boolean,
    val connected: Boolean,
)

internal data class ClipPending(
    val instanceId: String,
    val sas: String,
    val incoming: Boolean,
)

/** 监听方式。MANUAL = Wayland 下没有全局剪贴板监听，只能由用户点「发送」。 */
internal enum class ClipMode(val wire: String) {
    POLL("poll"),
    MANUAL("manual"),
}

/**
 * Wayland（尤其 GNOME）不提供后台全局剪贴板监听，明确降级为手动发送 ——
 * 别把「AWT 偶尔读得到」当成监听可用。
 */
internal fun clipModeFor(
    windows: Boolean,
    sessionType: String?,
    waylandDisplay: String?,
): ClipMode {
    if (windows) return ClipMode.POLL
    val wayland = sessionType?.lowercase(Locale.ROOT) == "wayland" ||
        !waylandDisplay.isNullOrEmpty()
    return if (wayland) ClipMode.MANUAL else ClipMode.POLL
}

internal fun currentClipMode(
    windows: Boolean = isWindows(),
    env: (String) -> String? = System::getenv,
): ClipMode = clipModeFor(windows, env("XDG_SESSION_TYPE"), env("WAYLAND_DISPLAY"))

/** 设备名：主机名（缺省 opi-desktop），按 **UTF-8 字节**截到内核的 64 字节上限。 */
internal fun clipDeviceName(hostname: String?): String {
    val base = hostname?.trim()?.takeIf { it.isNotEmpty() } ?: "opi-desktop"
    val out = StringBuilder()
    var bytes = 0
    for (c in base) {
        val n = c.toString().toByteArray(Charsets.UTF_8).size
        if (bytes + n > 64) break
        out.append(c)
        bytes += n
    }
    return out.toString()
}

internal fun clipConfigJson(
    dataDir: String,
    deviceName: String,
    tcpPort: Int = 0,
    discoveryPort: Int = 45888,
): String =
    "{\"data_dir\":${jsonString(dataDir)},\"tcp_port\":$tcpPort," +
        "\"discovery_port\":$discoveryPort,\"device_name\":${jsonString(deviceName)}}"

// ---------- 事件 ----------

internal sealed interface ClipEvent {
    data class PeerDiscovered(val peer: ClipPeer) : ClipEvent
    data class PeerOffline(val instanceId: String) : ClipEvent
    data class PairingPrompt(val instanceId: String, val sas: String, val incoming: Boolean) : ClipEvent
    data class PairingFinished(val instanceId: String, val trusted: Boolean, val reason: String?) : ClipEvent

    /** [text] 只给写入剪贴板用；任何状态/日志/控制面都不得包含它。 */
    data class ClipboardReceived(
        val instanceId: String,
        val text: String,
        val eventId: String?,
    ) : ClipEvent

    data class SendFailed(val instanceId: String) : ClipEvent
    data class Error(val message: String) : ClipEvent
}

private fun Map<String, JVal>.str(key: String): String? = (this[key] as? JVal.JStr)?.v
private fun Map<String, JVal>.bool(key: String): Boolean? = (this[key] as? JVal.JBool)?.v
private fun Map<String, JVal>.obj(key: String): Map<String, JVal>? =
    (this[key] as? JVal.JObj)?.v

internal fun parseClipPeer(v: Map<String, JVal>): ClipPeer? {
    val id = v.str("instance_id")?.takeIf { it.isNotEmpty() } ?: return null
    return ClipPeer(
        instanceId = id,
        name = v.str("name").orEmpty(),
        address = v.str("address").orEmpty(),
        trusted = v.bool("trusted") ?: false,
        connected = v.bool("connected") ?: false,
    )
}

/** 解析一条 `opi_clip_poll_event` 的 JSON；未知/坏载荷 → null（调用方丢弃）。 */
internal fun parseClipEvent(json: String): ClipEvent? {
    if (json.isBlank()) return null
    val m = parseLine(json) ?: return null
    return when (m.str("type")) {
        "peer_discovered" -> m.obj("peer")?.let(::parseClipPeer)?.let(ClipEvent::PeerDiscovered)
        "peer_offline" -> m.str("instance_id")?.let(ClipEvent::PeerOffline)
        "pairing_prompt" -> {
            val id = m.str("instance_id") ?: return null
            val sas = m.str("sas") ?: return null
            ClipEvent.PairingPrompt(id, sas, m.bool("incoming") ?: false)
        }
        "pairing_finished" -> m.str("instance_id")?.let {
            ClipEvent.PairingFinished(it, m.bool("trusted") ?: false, m.str("reason"))
        }
        "clipboard_received" -> {
            val id = m.str("instance_id") ?: return null
            val text = m.str("text") ?: return null
            val eventId = m.obj("version")?.str("event_id")
            ClipEvent.ClipboardReceived(id, text, eventId)
        }
        "send_failed" -> m.str("instance_id")?.let(ClipEvent::SendFailed)
        "error" -> m.str("message")?.let(ClipEvent::Error)
        else -> null
    }
}

/** 解析 `opi_clip_peers` 的 JSON 数组（失败 = 空表，不抛）。 */
internal fun parseClipPeers(json: String): List<ClipPeer> {
    if (json.isBlank()) return emptyList()
    val root = parseLine("{\"peers\":$json}") ?: return emptyList()
    val arr = root["peers"] as? JVal.JArr ?: return emptyList()
    return arr.v.mapNotNull { (it as? JVal.JObj)?.v?.let(::parseClipPeer) }
}

// ---------- UI 状态归约 ----------

/**
 * 界面需要的事件派生状态。**没有剪贴板文本字段**；[lastReceived] 只存事件 ID
 * （用于同一条远端内容重复到达时不再提示第二次）。
 */
internal data class ClipUiState(
    val names: Map<String, String> = emptyMap(),
    val pending: Map<String, ClipPending> = emptyMap(),
    val lastSource: String? = null,
    val lastReceived: String? = null,
    val lastError: String? = null,
)

internal fun displayName(
    instanceId: String,
    names: Map<String, String>,
    peers: List<ClipPeer>,
): String {
    val fromPeers = peers.firstOrNull { it.instanceId == instanceId }?.name
    return fromPeers?.takeIf { it.isNotBlank() }
        ?: names[instanceId]?.takeIf { it.isNotBlank() }
        ?: instanceId.take(8)
}

internal fun ClipUiState.applyEvent(event: ClipEvent): ClipUiState = when (event) {
    is ClipEvent.PeerDiscovered -> copy(
        names = names + (event.peer.instanceId to event.peer.name),
    )
    is ClipEvent.PeerOffline -> copy(pending = pending - event.instanceId)
    is ClipEvent.PairingPrompt -> copy(
        pending = pending + (
            event.instanceId to ClipPending(event.instanceId, event.sas, event.incoming)
            ),
        lastError = null,
    )
    is ClipEvent.PairingFinished -> copy(
        pending = pending - event.instanceId,
        lastError = if (event.trusted) {
            null
        } else {
            "与设备 ${displayName(event.instanceId, names, emptyList())} 的配对未完成" +
                (event.reason?.takeIf { it.isNotBlank() }?.let { "：$it" } ?: "")
        },
    )
    is ClipEvent.ClipboardReceived -> {
        // 同一条事件重复投递（或 UI 重放）只提示一次。没有 event_id 的旧内核不设防。
        if (event.eventId != null && event.eventId == lastReceived) {
            this
        } else {
            copy(
                lastSource = displayName(event.instanceId, names, emptyList()),
                lastReceived = event.eventId ?: lastReceived,
                lastError = null,
            )
        }
    }
    is ClipEvent.SendFailed -> copy(
        lastError = "发送到 ${displayName(event.instanceId, names, emptyList())} 失败",
    )
    is ClipEvent.Error -> copy(lastError = event.message.take(300))
}

// ---------- 控制面快照（状态栏宿主 ⇄ 设置窗） ----------

internal data class ClipStatus(
    val enabled: Boolean,
    val running: Boolean,
    val mode: ClipMode,
    val deviceName: String,
    val peers: List<ClipPeer>,
    val pending: List<ClipPending>,
    val lastSource: String?,
    val lastError: String?,
    val ffiError: String?,
)

internal fun peerJson(p: ClipPeer): String =
    "{\"instance_id\":${jsonString(p.instanceId)},\"name\":${jsonString(p.name)}," +
        "\"address\":${jsonString(p.address)},\"trusted\":${p.trusted}," +
        "\"connected\":${p.connected}}"

/** 控制面应答。**只含元数据**；剪贴板文本绝不经过这里。 */
internal fun encodeClipStatus(s: ClipStatus): String = buildString {
    append("{\"ok\":true,\"enabled\":").append(s.enabled)
    append(",\"running\":").append(s.running)
    append(",\"mode\":").append(jsonString(s.mode.wire))
    append(",\"device\":").append(jsonString(s.deviceName))
    append(",\"last_source\":").append(s.lastSource?.let(::jsonString) ?: "null")
    append(",\"last_error\":").append(s.lastError?.let(::jsonString) ?: "null")
    append(",\"ffi_error\":").append(s.ffiError?.let(::jsonString) ?: "null")
    append(",\"peers\":[")
    s.peers.forEachIndexed { i, p -> if (i > 0) append(','); append(peerJson(p)) }
    append("],\"pending\":[")
    s.pending.forEachIndexed { i, p ->
        if (i > 0) append(',')
        append("{\"instance_id\":").append(jsonString(p.instanceId))
        append(",\"sas\":").append(jsonString(p.sas))
        append(",\"incoming\":").append(p.incoming)
        append('}')
    }
    append("]}")
}

internal fun parseClipStatus(line: String): ClipStatus? {
    val m = parseLine(line) ?: return null
    if (m["ok"] != JVal.JBool(true)) return null
    val peers = (m["peers"] as? JVal.JArr)?.v
        ?.mapNotNull { (it as? JVal.JObj)?.v?.let(::parseClipPeer) }
        .orEmpty()
    val pending = (m["pending"] as? JVal.JArr)?.v.orEmpty().mapNotNull { v ->
        val pm = (v as? JVal.JObj)?.v ?: return@mapNotNull null
        val id = pm.str("instance_id") ?: return@mapNotNull null
        val sas = pm.str("sas") ?: return@mapNotNull null
        ClipPending(id, sas, pm.bool("incoming") ?: false)
    }
    return ClipStatus(
        enabled = m.bool("enabled") ?: false,
        running = m.bool("running") ?: false,
        mode = if (m.str("mode") == ClipMode.MANUAL.wire) ClipMode.MANUAL else ClipMode.POLL,
        deviceName = m.str("device").orEmpty(),
        peers = peers,
        pending = pending,
        lastSource = m.str("last_source"),
        lastError = m.str("last_error"),
        ffiError = m.str("ffi_error"),
    )
}
