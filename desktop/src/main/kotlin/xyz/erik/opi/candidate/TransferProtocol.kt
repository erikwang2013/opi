// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 传输事件的纯逻辑层：解析 / UI 状态归约 / 控制面快照编解码。
//!
//! 与 ClipProtocol.kt 同一纪律：不碰 FFI、AWT、文件与线程，全部可单测。事件里的
//! 文件只保留**数量与总字节**，不把相对路径塞进每秒轮询的控制面快照。

package xyz.erik.opi.candidate

import java.util.Locale

internal enum class TransferState(val wire: String) {
    WAITING("waiting"),
    ACTIVE("active"),
    DONE("done"),
    FAILED("failed"),
    CANCELED("canceled"),
    ;

    companion object {
        fun fromWire(value: String?): TransferState =
            entries.firstOrNull { it.wire == value } ?: WAITING
    }
}

internal data class TransferUi(
    val id: String,
    val peerId: String,
    val outgoing: Boolean,
    val fileCount: Int,
    val totalBytes: Long,
    val transferredBytes: Long = 0,
    val state: TransferState = TransferState.WAITING,
    val savedDir: String? = null,
    val error: String? = null,
)

internal sealed interface TransferEvent {
    data class Offer(
        val instanceId: String,
        val transferId: String,
        val fileCount: Int,
        val totalBytes: Long,
        val outgoing: Boolean,
    ) : TransferEvent

    data class Progress(
        val instanceId: String,
        val transferId: String,
        val transferredBytes: Long,
        val totalBytes: Long,
        val outgoing: Boolean,
    ) : TransferEvent

    data class Completed(
        val instanceId: String,
        val transferId: String,
        val fileCount: Int,
        val totalBytes: Long,
        val outgoing: Boolean,
        val savedDir: String?,
    ) : TransferEvent

    data class Failed(
        val instanceId: String,
        val transferId: String,
        val reason: String,
        val outgoing: Boolean,
    ) : TransferEvent

    data class Canceled(
        val instanceId: String,
        val transferId: String,
        val outgoing: Boolean,
    ) : TransferEvent
}

private fun Map<String, JVal>.str(key: String): String? = (this[key] as? JVal.JStr)?.v
private fun Map<String, JVal>.bool(key: String): Boolean? = (this[key] as? JVal.JBool)?.v
private fun Map<String, JVal>.num(key: String): Long? = (this[key] as? JVal.JNum)?.v
private fun Map<String, JVal>.fileCount(): Int = (this["files"] as? JVal.JArr)?.v?.size ?: 0

/** 解析一条 `transfer_*` 事件；不是传输事件/坏载荷 → null。 */
internal fun parseTransferEvent(m: Map<String, JVal>): TransferEvent? {
    val type = m.str("type") ?: return null
    if (!type.startsWith("transfer_")) return null
    val id = m.str("transfer_id") ?: return null
    val peer = m.str("instance_id") ?: return null
    val outgoing = m.bool("outgoing") ?: false
    return when (type) {
        "transfer_offer" -> TransferEvent.Offer(
            peer, id, m.fileCount(), m.num("total_bytes") ?: 0L, outgoing,
        )
        "transfer_progress" -> TransferEvent.Progress(
            peer, id, m.num("transferred_bytes") ?: 0L, m.num("total_bytes") ?: 0L, outgoing,
        )
        "transfer_completed" -> TransferEvent.Completed(
            peer, id, m.fileCount(), m.num("total_bytes") ?: 0L, outgoing, m.str("saved_dir"),
        )
        "transfer_failed" -> TransferEvent.Failed(
            peer, id, m.str("reason").orEmpty().take(300), outgoing,
        )
        "transfer_canceled" -> TransferEvent.Canceled(peer, id, outgoing)
        else -> null
    }
}

private const val MAX_TRANSFER_STATUS = 24

private val TransferUi.terminal: Boolean
    get() = state == TransferState.DONE ||
        state == TransferState.FAILED ||
        state == TransferState.CANCELED

/** 设置窗看到的事件派生状态（宿主进程内维护，随控制面快照出去）。 */
internal data class TransferUiState(val items: List<TransferUi> = emptyList()) {
    fun apply(event: TransferEvent): TransferUiState = when (event) {
        is TransferEvent.Offer -> upsert(
            TransferUi(event.transferId, event.instanceId, event.outgoing, event.fileCount, event.totalBytes),
        )
        is TransferEvent.Progress -> edit(event.transferId, {
            TransferUi(
                event.transferId, event.instanceId, event.outgoing, 0, event.totalBytes,
                event.transferredBytes, TransferState.ACTIVE,
            )
        }) { current ->
            if (current.terminal) {
                current
            } else {
                current.copy(
                    transferredBytes = event.transferredBytes.coerceAtLeast(0L),
                    totalBytes = event.totalBytes.takeIf { it > 0 } ?: current.totalBytes,
                    state = TransferState.ACTIVE,
                )
            }
        }
        is TransferEvent.Completed -> edit(event.transferId, {
            TransferUi(
                event.transferId, event.instanceId, event.outgoing, event.fileCount,
                event.totalBytes, event.totalBytes, TransferState.DONE, event.savedDir,
            )
        }) { current ->
            if (current.terminal) {
                current
            } else {
                val total = event.totalBytes.takeIf { it > 0 } ?: current.totalBytes
                current.copy(
                    fileCount = event.fileCount.takeIf { it > 0 } ?: current.fileCount,
                    totalBytes = total,
                    transferredBytes = total,
                    state = TransferState.DONE,
                    savedDir = event.savedDir ?: current.savedDir,
                    error = null,
                )
            }
        }
        is TransferEvent.Failed -> edit(event.transferId, {
            TransferUi(
                event.transferId, event.instanceId, event.outgoing, 0, 0L,
                state = TransferState.FAILED, error = event.reason,
            )
        }) { current ->
            if (current.terminal) current else {
                current.copy(state = TransferState.FAILED, error = event.reason)
            }
        }
        is TransferEvent.Canceled -> edit(event.transferId, {
            TransferUi(
                event.transferId, event.instanceId, event.outgoing, 0, 0L,
                state = TransferState.CANCELED,
            )
        }) { current ->
            if (current.terminal) current else current.copy(state = TransferState.CANCELED)
        }
    }

    /** 接受/拒绝的即时回显（真正的事件由内核随后异步发出）。 */
    fun mark(id: String, state: TransferState, error: String? = null): TransferUiState =
        edit(id, { TransferUi(id, "", false, 0, 0L, state = state, error = error) }) {
            if (it.terminal) it else it.copy(state = state, error = error)
        }

    private fun edit(
        id: String,
        fallback: () -> TransferUi,
        change: (TransferUi) -> TransferUi,
    ): TransferUiState {
        val index = items.indexOfFirst { it.id == id }
        val current = if (index >= 0) items[index] else fallback()
        val next = change(current)
        return copy(
            items = if (index < 0) {
                (items + next).takeLast(MAX_TRANSFER_STATUS)
            } else {
                items.toMutableList().also { it[index] = next }
            },
        )
    }

    private fun upsert(item: TransferUi): TransferUiState = edit(item.id, { item }) { item }
}

/** 只有已配对且在线的设备能收文件（内核 `send_files` 同一条门槛）。 */
internal fun sendablePeers(peers: List<ClipPeer>): List<ClipPeer> =
    peers.filter { it.trusted && it.connected }

/**
 * 自动剪贴板闸门：开关打开、本机有新文本、且至少有一台**已信任**设备时才 publish。
 * 逐会话的信任过滤仍在内核（`publish_text` 只投给已信任 session）；这里先挡住
 * 「一台都没配对」时的无谓广播与 pending 堆积。
 */
internal class AutoClipboardGate(
    private val enabled: () -> Boolean,
    private val trustedPeerCount: () -> Int,
    private val publish: (String) -> Unit,
) {
    fun onLocalText(text: String?) {
        if (!enabled() || text.isNullOrEmpty()) return
        if (trustedPeerCount() <= 0) return
        publish(text)
    }
}

internal fun formatTransferBytes(bytes: Long): String {
    if (bytes < 1024L) return "$bytes B"
    val units = arrayOf("KiB", "MiB", "GiB", "TiB")
    var value = bytes.toDouble()
    var unit = 0
    while (value >= 1024.0 && unit < units.lastIndex) {
        value /= 1024.0
        unit++
    }
    return String.format(Locale.ROOT, "%.1f %s", value, units[unit])
}

// ---------- 控制面快照里的传输表 ----------

internal fun encodeTransferStatuses(items: List<TransferUi>): String = buildString {
    append('[')
    items.forEachIndexed { i, t ->
        if (i > 0) append(',')
        append("{\"id\":").append(jsonString(t.id))
        append(",\"peer\":").append(jsonString(t.peerId))
        append(",\"outgoing\":").append(t.outgoing)
        append(",\"file_count\":").append(t.fileCount)
        append(",\"total_bytes\":").append(t.totalBytes)
        append(",\"transferred_bytes\":").append(t.transferredBytes)
        append(",\"state\":").append(jsonString(t.state.wire))
        append(",\"saved_dir\":").append(t.savedDir?.let(::jsonString) ?: "null")
        append(",\"error\":").append(t.error?.let(::jsonString) ?: "null")
        append('}')
    }
    append(']')
}

internal fun parseTransferStatuses(value: JVal?): List<TransferUi> {
    val arr = (value as? JVal.JArr)?.v ?: return emptyList()
    return arr.take(64).mapNotNull { item ->
        val m = (item as? JVal.JObj)?.v ?: return@mapNotNull null
        val id = m.str("id") ?: return@mapNotNull null
        val peer = m.str("peer") ?: return@mapNotNull null
        TransferUi(
            id = id,
            peerId = peer,
            outgoing = m.bool("outgoing") ?: false,
            fileCount = (m.num("file_count") ?: 0L).toInt().coerceAtLeast(0),
            totalBytes = (m.num("total_bytes") ?: 0L).coerceAtLeast(0L),
            transferredBytes = (m.num("transferred_bytes") ?: 0L).coerceAtLeast(0L),
            state = TransferState.fromWire(m.str("state")),
            savedDir = m.str("saved_dir"),
            error = m.str("error"),
        )
    }
}
