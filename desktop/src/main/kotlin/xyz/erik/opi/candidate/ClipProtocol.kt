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
    val name: String,
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
    advertiseIp: String? = null,
    advertisePrefix: Int? = null,
): String = buildString {
    append("{\"data_dir\":").append(jsonString(dataDir))
    append(",\"tcp_port\":").append(tcpPort)
    append(",\"discovery_port\":").append(discoveryPort)
    append(",\"device_name\":").append(jsonString(deviceName))
    if (!advertiseIp.isNullOrBlank()) {
        append(",\"advertise_ipv4\":").append(jsonString(advertiseIp))
    }
    if (advertisePrefix != null) {
        append(",\"advertise_prefix\":").append(advertisePrefix)
    }
    append('}')
}

/**
 * 本机局域网 IPv4 + 网卡名。
 *
 * Windows 上 `NetworkInterface.name` **不是**网卡显示名：JDK < 21 是 `eth0/eth1`，
 * JDK 21+ 是 `ethernet_32768`。只按 `name` 过滤 WSL 时 `vEthernet (WSL)` 永远
 * 漏过去，于是组播/探测都发到 WSL 网段，局域网里一台设备也发现不了。
 * 这里同时看 `displayName`，并优先选「默认出口地址所属的网卡」，最后才回退。
 */
internal data class LanAddress(val iface: String, val ip: String, val prefix: Int)

internal data class LanCandidate(
    val name: String,
    val display: String,
    val ip: String,
    val prefix: Int,
    val virtual: Boolean = false,
)

private val virtualIfaceTokens = listOf(
    "docker", "veth", "br-", "virbr", "vmnet", "vbox", "virtualbox", "vmware",
    "vethernet", "hyper-v", "wsl", "tun", "tap", "wg", "wireguard", "tailscale",
    "zerotier", "zt", "hamachi", "npcap", "bluetooth", "openvpn", "nordlynx",
    "globalprotect", "anyconnect", "fortinet",
)

private fun isLanCandidate(c: LanCandidate): Boolean {
    val parts = c.ip.split('.')
    if (parts.size != 4) return false
    val o = parts.mapNotNull { it.toIntOrNull()?.takeIf { n -> n in 0..255 } }
    if (o.size != 4) return false
    // 10/8、172.16/12、192.168/16，以及运营商级 NAT 常用的 100.64/10。
    return o[0] == 10 ||
        (o[0] == 172 && o[1] in 16..31) ||
        (o[0] == 192 && o[1] == 168) ||
        (o[0] == 100 && o[1] in 64..127)
}

private fun isVirtualIface(c: LanCandidate): Boolean {
    if (c.virtual) return true
    val text = "${c.name} ${c.display}".lowercase(Locale.ROOT)
    return virtualIfaceTokens.any { text.contains(it) }
}

private fun looksPhysical(c: LanCandidate): Boolean {
    val text = "${c.name} ${c.display}".lowercase(Locale.ROOT)
    return listOf(
        "wlan", "wi-fi", "wifi", "wireless", "ethernet", "eth", "enp", "eno",
        "ens", "wlp", "enx",
    ).any { text.contains(it) }
}

/** 纯判定：候选 + 默认出口地址 → 该用的局域网地址。可单测。 */
internal fun pickLanAddress(candidates: List<LanCandidate>, routeIp: String?): LanAddress? {
    val lan = candidates.filter(::isLanCandidate)
    val physical = lan.filterNot(::isVirtualIface)
    val pool = physical.ifEmpty { lan }
    val chosen = pool.firstOrNull { it.ip == routeIp }
        ?: pool.firstOrNull(::looksPhysical)
        ?: pool.firstOrNull()
    return chosen?.let { LanAddress(it.name, it.ip, it.prefix) }
}

/** 默认出口地址（UDP connect 不真正发包；失败 = null）。 */
private fun routeLanIpv4(): String? = runCatching {
    java.net.DatagramSocket().use { socket ->
        socket.connect(java.net.InetAddress.getByName("223.5.5.5"), 53)
        (socket.localAddress as? java.net.Inet4Address)?.hostAddress
    }
}.getOrNull()

internal fun localLanAddress(): LanAddress? {
    val nifs = runCatching {
        java.util.Collections.list(java.net.NetworkInterface.getNetworkInterfaces())
    }.getOrDefault(emptyList())
    val candidates = ArrayList<LanCandidate>()
    for (n in nifs) {
        if (!n.isUp || n.isLoopback) continue
        for (ia in n.interfaceAddresses) {
            val addr = ia.address as? java.net.Inet4Address ?: continue
            if (addr.isLoopbackAddress || addr.isLinkLocalAddress) continue
            candidates += LanCandidate(
                name = n.name.orEmpty(),
                display = n.displayName.orEmpty(),
                ip = addr.hostAddress ?: continue,
                prefix = ia.networkPrefixLength.toInt(),
                virtual = n.isVirtual,
            )
        }
    }
    return pickLanAddress(candidates, routeLanIpv4())
}

// ---------- 事件 ----------

internal sealed interface ClipEvent {
    data class PeerDiscovered(val peer: ClipPeer) : ClipEvent
    data class PeerOffline(val instanceId: String) : ClipEvent
    data class PairingPrompt(val instanceId: String, val name: String, val sas: String, val incoming: Boolean) : ClipEvent
    data class PairingFinished(val instanceId: String, val trusted: Boolean, val reason: String?) : ClipEvent

    /** [text] 只给写入剪贴板用；任何状态/日志/控制面都不得包含它。 */
    data class ClipboardReceived(
        val instanceId: String,
        val text: String,
        val eventId: String?,
    ) : ClipEvent

    data class SendFailed(val instanceId: String) : ClipEvent
    data class Error(val message: String) : ClipEvent

    /** 文件/文件夹传输事件（字段与状态归约见 TransferProtocol.kt）。 */
    data class Transfer(val event: TransferEvent) : ClipEvent
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
            ClipEvent.PairingPrompt(id, m.str("name").orEmpty(), sas, m.bool("incoming") ?: false)
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
        else -> parseTransferEvent(m)?.let(ClipEvent::Transfer)
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
            event.instanceId to ClipPending(event.instanceId, event.name, event.sas, event.incoming)
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
    // 传输状态由宿主单独的 TransferService 维护，不混进剪贴板 UI 状态。
    is ClipEvent.Transfer -> this
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
    /** 核心自报的发现计数与实际绑定地址；用于「一台设备都没有」时定位。 */
    val beaconsSent: Long = 0,
    val beaconsReceived: Long = 0,
    val advertiseIp: String? = null,
    val advertisePrefix: Int? = null,
    /** 传输 offer/进度/结果；设置窗每秒轮询本快照。 */
    val transfers: List<TransferUi> = emptyList(),
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
    append(",\"beacons_sent\":").append(s.beaconsSent)
    append(",\"beacons_received\":").append(s.beaconsReceived)
    append(",\"advertise_ipv4\":").append(s.advertiseIp?.let(::jsonString) ?: "null")
    append(",\"advertise_prefix\":").append(s.advertisePrefix ?: "null")
    append(",\"transfers\":").append(encodeTransferStatuses(s.transfers))
    append(",\"peers\":[")
    s.peers.forEachIndexed { i, p -> if (i > 0) append(','); append(peerJson(p)) }
    append("],\"pending\":[")
    s.pending.forEachIndexed { i, p ->
        if (i > 0) append(',')
        append("{\"instance_id\":").append(jsonString(p.instanceId))
        append(",\"name\":").append(jsonString(p.name))
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
        ClipPending(id, pm.str("name").orEmpty(), sas, pm.bool("incoming") ?: false)
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
        beaconsSent = (m["beacons_sent"] as? JVal.JNum)?.v ?: 0L,
        beaconsReceived = (m["beacons_received"] as? JVal.JNum)?.v ?: 0L,
        advertiseIp = m.str("advertise_ipv4"),
        advertisePrefix = (m["advertise_prefix"] as? JVal.JNum)?.v?.toInt(),
        transfers = parseTransferStatuses(m["transfers"]),
    )
}
