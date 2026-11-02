// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「跨设备传输」操作区：选择目标设备，发送文件/文件夹/媒体/文本/剪贴板，
//! 处理入站 offer 与传输进度。所有耗时操作（文件扫描、对话框、控制面请求）
//! 都在 IO 线程；FFI 只由宿主线程执行，本页只发控制命令。

package xyz.erik.opi.candidate

import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.OutlinedTextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.nio.file.Path

@Composable
internal fun TransferPage(status: ClipStatus, report: (String) -> Unit) {
    val scope = rememberCoroutineScope()
    val targets = sendablePeers(status.peers)
    var target by remember { mutableStateOf("") }
    var text by remember { mutableStateOf("") }

    LaunchedEffect(targets.joinToString { it.instanceId }) {
        if (targets.none { it.instanceId == target }) {
            target = targets.firstOrNull()?.instanceId.orEmpty()
        }
    }

    fun sendPaths(paths: List<Path>) {
        if (paths.isEmpty()) return
        if (target.isEmpty()) {
            report("请先选择一台已配对且在线的设备")
            return
        }
        val peer = target
        scope.launch {
            val prep = withContext(Dispatchers.IO) {
                prepareTransferPaths(paths.map { it.toString() })
            }
            when (prep) {
                is TransferPrep.Rejected -> report(prep.reason)
                is TransferPrep.Ready -> {
                    val ok = withContext(Dispatchers.IO) { ClipClient.sendFiles(peer, prep.json) }
                    report(
                        if (ok) {
                            "已提交 ${prep.entries} 项 / ${formatTransferBytes(prep.bytes)}"
                        } else {
                            "发送失败：宿主未运行或设备未连接"
                        },
                    )
                }
            }
        }
    }

    fun pickAndSend(media: Boolean) {
        scope.launch {
            val paths = withContext(Dispatchers.IO) {
                if (media) pickTransferMedia() else pickTransferFiles()
            }
            sendPaths(paths)
        }
    }

    fun sendFolder() {
        scope.launch {
            val folder = withContext(Dispatchers.IO) { pickTransferFolder() } ?: return@launch
            sendPaths(listOf(folder))
        }
    }

    fun accept(t: TransferUi, chooseDir: Boolean) {
        scope.launch {
            val dir = (if (chooseDir) {
                withContext(Dispatchers.IO) { pickTransferSaveDir()?.toString() }
            } else {
                defaultTransferSaveDir().toString()
            }) ?: return@launch
            val ok = withContext(Dispatchers.IO) { ClipClient.respondTransfer(t.id, true, dir) }
            report(if (ok) "已接受，保存到 $dir" else "接收命令未送达宿主")
        }
    }

    fun reject(t: TransferUi) {
        scope.launch {
            val ok = withContext(Dispatchers.IO) {
                ClipClient.respondTransfer(t.id, false, null)
            }
            if (!ok) report("拒绝命令未送达宿主")
        }
    }

    fun cancel(t: TransferUi) {
        scope.launch {
            val ok = withContext(Dispatchers.IO) { ClipClient.cancelTransfer(t.id) }
            if (!ok) report("取消命令未送达宿主")
        }
    }

    fun open(t: TransferUi) {
        val dir = t.savedDir ?: return
        scope.launch {
            val ok = withContext(Dispatchers.IO) { openTransferFolder(dir) }
            if (!ok) report("无法打开文件夹：$dir")
        }
    }

    SectionTitle("传输")
    Text(
        "文件/文件夹/媒体发给选中的已配对设备；文本与剪贴板发给所有已信任设备。" +
            "单次最多 $TRANSFER_MAX_ENTRIES 项、单文件 2 GiB、总计 8 GiB。",
        fontSize = 12.sp, color = Muted,
    )
    if (targets.isEmpty()) {
        Spacer(Modifier.height(4.dp))
        Text("没有已配对且在线的设备。请先在上方完成配对。", fontSize = 12.sp, color = Muted)
    } else {
        Spacer(Modifier.height(6.dp))
        for (p in targets) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                ClipButton(
                    label = "发送到 ${p.name.ifBlank { p.instanceId.take(8) }}",
                    enabled = p.instanceId != target,
                ) { target = p.instanceId }
                Spacer(Modifier.width(8.dp))
            }
        }
    }
    Spacer(Modifier.height(6.dp))
    Row {
        ClipButton("选择文件", enabled = status.running && targets.isNotEmpty()) { pickAndSend(media = false) }
        Spacer(Modifier.width(8.dp))
        ClipButton("选择文件夹", enabled = status.running && targets.isNotEmpty()) { sendFolder() }
        Spacer(Modifier.width(8.dp))
        ClipButton("图片/音频/视频", enabled = status.running && targets.isNotEmpty()) {
            pickAndSend(media = true)
        }
    }

    Spacer(Modifier.height(8.dp))
    Row(verticalAlignment = Alignment.CenterVertically) {
        OutlinedTextField(
            value = text,
            onValueChange = { text = it },
            singleLine = true,
            label = { Text("要发送的文本", fontSize = 11.sp) },
            modifier = Modifier.weight(1f),
        )
        Spacer(Modifier.width(8.dp))
        ClipButton(
            label = "发送文本",
            enabled = status.running && text.isNotEmpty() && hasTrustedPeer(status),
        ) {
            val payload = text
            scope.launch {
                val ok = withContext(Dispatchers.IO) { ClipClient.sendText(payload) }
                if (ok) {
                    text = ""
                    report("文本已发送给已信任设备（内容不显示、不记录）")
                } else {
                    report("发送失败：宿主未运行")
                }
            }
        }
    }
    Row(verticalAlignment = Alignment.CenterVertically) {
        ClipButton("发送当前剪贴板", enabled = status.running && hasTrustedPeer(status)) {
            scope.launch {
                val clip = withContext(Dispatchers.IO) { readLocalClipboardText() }
                if (clip.isNullOrEmpty()) {
                    report("本机剪贴板里没有文本")
                } else if (clip.length > CLIP_TEXT_MAX_CHARS) {
                    report("文本超过 256 KiB 上限，未发送")
                } else {
                    val ok = withContext(Dispatchers.IO) { ClipClient.sendText(clip) }
                    report(if (ok) "剪贴板文本已发送" else "发送失败：宿主未运行")
                }
            }
        }
    }

    val items = status.transfers.asReversed()
    if (items.isNotEmpty()) {
        SectionTitle("传输记录")
        for (t in items) {
            TransferCard(
                t = t,
                name = displayName(t.peerId, emptyMap(), status.peers),
                running = status.running,
                onAccept = { accept(t, chooseDir = false) },
                onChooseDir = { accept(t, chooseDir = true) },
                onReject = { reject(t) },
                onCancel = { cancel(t) },
                onOpen = { open(t) },
            )
        }
    }
}

/** 文本/剪贴板是广播：至少要有一台已信任设备，否则内核也无处可发。 */
private fun hasTrustedPeer(s: ClipStatus): Boolean = s.peers.any { it.trusted }

@Composable
private fun TransferCard(
    t: TransferUi,
    name: String,
    running: Boolean,
    onAccept: () -> Unit,
    onChooseDir: () -> Unit,
    onReject: () -> Unit,
    onCancel: () -> Unit,
    onOpen: () -> Unit,
) {
    Column(
        Modifier.fillMaxWidth().padding(vertical = 6.dp)
            .border(1.dp, Hairline).padding(14.dp),
    ) {
        Text(
            (if (t.outgoing) "发送 · " else "接收 · ") + name,
            fontSize = 14.sp, color = Ink, fontWeight = FontWeight.Bold,
        )
        Text(
            buildString {
                append("${t.fileCount} 项 · ${formatTransferBytes(t.totalBytes)}")
                if (t.state == TransferState.ACTIVE || t.state == TransferState.DONE) {
                    append(" · 已传 ${formatTransferBytes(t.transferredBytes)}")
                }
            },
            fontSize = 12.sp, color = Muted,
        )
        when (t.state) {
            TransferState.WAITING -> {
                Text(
                    if (t.outgoing) "等待对方接收…" else "对方请求发送以上内容；请确认来源后接收。",
                    fontSize = 12.sp, color = Ink,
                )
                Spacer(Modifier.height(8.dp))
                if (!t.outgoing) {
                    ClipButton("接收（默认目录）", enabled = running, onClick = onAccept)
                    Spacer(Modifier.width(8.dp))
                    ClipButton("选择目录接收", enabled = running, onClick = onChooseDir)
                    Spacer(Modifier.width(8.dp))
                    ClipButton("拒绝", enabled = running, onClick = onReject)
                } else {
                    ClipButton("取消", enabled = running, onClick = onCancel)
                }
            }
            TransferState.ACTIVE -> {
                val pct = if (t.totalBytes > 0) {
                    (t.transferredBytes * 100 / t.totalBytes).coerceIn(0, 100)
                } else {
                    0
                }
                Text("传输中 $pct%", fontSize = 12.sp, color = Teal)
                Spacer(Modifier.height(8.dp))
                ClipButton("取消", enabled = running, onClick = onCancel)
            }
            TransferState.DONE -> {
                if (t.outgoing) {
                    Text("已发送", fontSize = 12.sp, color = Teal)
                } else {
                    Text("已保存到 ${t.savedDir ?: "（未知目录）"}", fontSize = 12.sp, color = Teal)
                    Spacer(Modifier.height(8.dp))
                    ClipButton("打开文件夹", enabled = t.savedDir != null, onClick = onOpen)
                }
            }
            TransferState.FAILED -> Text(
                "失败：${t.error ?: "未知原因"}",
                fontSize = 12.sp, color = TransferWarnRed,
            )
            TransferState.CANCELED -> Text("已取消", fontSize = 12.sp, color = Muted)
        }
    }
}

private val TransferWarnRed = Color(0xFFB3261E)
