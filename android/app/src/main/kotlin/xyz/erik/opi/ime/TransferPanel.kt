// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.provider.DocumentsContract
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.FileProvider
import xyz.erik.opi.clip.ClipRuntime
import xyz.erik.opi.clip.ClipTransfer
import xyz.erik.opi.clip.TransferPhase
import xyz.erik.opi.clip.TransferActivity
import xyz.erik.opi.clip.formatTransferBytes
import xyz.erik.opi.clip.hasTrustedTarget
import xyz.erik.opi.i18n.Text
import xyz.erik.opi.keyboard.LocalKeyboardPalette
import java.io.File

/**
 * 跨设备传输面板（2026-11-02）：附近设备、五种发送入口、收发状态。
 *
 * 文件/文件夹/媒体经 [TransferActivity]（透明 Activity）拿 SAF 结果，因为 IME 是 Service，
 * 收不到 `onActivityResult`。文本输入也走它 —— IME 窗口里的输入框无法稳定拉起自己的键盘。
 * 剪贴板和文本用内核 `publish_text`，只会发给已配对会话；未配对设备在会话层就被拒绝。
 */
@Composable
fun TransferPanel(onClose: () -> Unit, modifier: Modifier = Modifier) {
    val p = LocalKeyboardPalette.current
    val context = LocalContext.current
    var selected by remember { mutableStateOf("") }
    var folderNote by remember { mutableStateOf("") }
    val connected = ClipRuntime.peers.filter { it.trusted && it.connected }
    val target = connected.firstOrNull { it.instanceId == selected } ?: connected.firstOrNull()

    fun pick(mode: String) {
        val peer = target
        if (peer == null) {
            ClipRuntime.transferMessage = "请先选择一台已配对且已连接的设备"
            return
        }
        TransferActivity.start(context, mode, peer.instanceId)
    }

    fun requirePairing(): Boolean {
        if (hasTrustedTarget(ClipRuntime.peers)) return true
        ClipRuntime.transferMessage = "没有已配对设备；请先在应用设置里核对数字配对"
        return false
    }

    fun publish(block: () -> String) {
        if (requirePairing()) ClipRuntime.transferMessage = block()
    }

    Column(modifier = modifier.fillMaxSize().opiPanelBackground(p.panelBg)) {
        PanelHeader("跨设备传输", "仅局域网 · 已配对设备", onClose)
        Column(
            modifier = Modifier.fillMaxWidth().weight(1f)
                .verticalScroll(rememberScrollState())
                .padding(start = 14.dp, end = 14.dp, bottom = 10.dp),
        ) {
            SectionTitle("附近设备")
            if (ClipRuntime.peers.isEmpty()) {
                Hint("正在发现局域网设备…两边都打开传输，或用应用设置页的手动 IP 探测。")
            } else {
                for (peer in ClipRuntime.peers) {
                    val name = peer.name.ifEmpty { peer.instanceId.take(8) }
                    val state = when {
                        !peer.trusted -> "未配对"
                        peer.connected -> "已连接"
                        else -> "连接中…"
                    }
                    Row(
                        modifier = Modifier.fillMaxWidth()
                            .clip(RoundedCornerShape(6.dp))
                            .clickable(
                                enabled = peer.trusted && peer.connected,
                                role = Role.RadioButton,
                            ) { selected = peer.instanceId }
                            .padding(vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Column(Modifier.weight(1f)) {
                            Text(name, fontSize = 13.sp, color = p.panelFg)
                            Text(
                                buildString {
                                    append(state)
                                    if (peer.address.isNotEmpty()) append(" · ").append(peer.address)
                                    if (!peer.trusted) append(" · 请到应用设置页配对")
                                },
                                fontSize = 11.sp,
                                color = p.barDimFg,
                            )
                        }
                        if (peer.trusted && peer.connected) {
                            Text(
                                if (peer == target) "已选 ✓" else "选择",
                                fontSize = 12.sp,
                                color = if (peer == target) p.accent else p.barDimFg,
                            )
                        }
                    }
                }
            }

            HorizontalDivider(Modifier.padding(vertical = 6.dp))
            SectionTitle("发送")
            ActionRow("照片 / 视频", "系统选择器，可多选") { pick(TransferActivity.MODE_MEDIA) }
            ActionRow("文件", "任意文件，可多选") { pick(TransferActivity.MODE_FILES) }
            ActionRow("文件夹", "选择整棵目录树") { pick(TransferActivity.MODE_FOLDER) }
            ActionRow("文本", "输入一段文字发送") {
                if (requirePairing()) TransferActivity.start(context, TransferActivity.MODE_TEXT)
            }
            ActionRow("剪贴板", "发送当前系统剪贴板的纯文本") {
                publish { ClipRuntime.publishCurrentClipboard(context) }
            }
            if (target == null && connected.isEmpty() && ClipRuntime.peers.isNotEmpty()) {
                Hint("文件/文件夹/媒体需要一台已配对且已连接的设备。")
            }
            if (ClipRuntime.transferMessage.isNotEmpty()) {
                Hint(ClipRuntime.transferMessage, accent = true)
            }

            if (ClipRuntime.transfers.isNotEmpty()) {
                HorizontalDivider(Modifier.padding(vertical = 6.dp))
                SectionTitle("传输记录")
                for (transfer in ClipRuntime.transfers) {
                    TransferCard(transfer) { folderNote = it }
                }
            }
            if (folderNote.isNotEmpty()) Hint(folderNote, accent = true)

            HorizontalDivider(Modifier.padding(vertical = 6.dp))
            Hint(
                "接收文件保存到应用专属外部下载目录 Download/opi。" +
                    "Android 不允许普通应用直接写公共 Downloads，卸载应用会删除该目录。",
            )
        }
    }
}

@Composable
private fun TransferCard(transfer: ClipTransfer, note: (String) -> Unit) {
    val p = LocalKeyboardPalette.current
    val context = LocalContext.current
    Column(
        modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp)
            .border(1.dp, p.barDimFg.copy(alpha = 0.3f), RoundedCornerShape(8.dp))
            .padding(10.dp),
    ) {
        Text(
            (if (transfer.outgoing) "发送给 " else "来自 ") + transfer.peerName,
            fontSize = 13.sp,
            fontWeight = FontWeight.Bold,
            color = p.panelFg,
        )
        Text(
            "${transfer.files.size} 项 · ${formatTransferBytes(transfer.totalBytes)}",
            fontSize = 11.sp,
            color = p.barDimFg,
        )
        when (transfer.phase) {
            TransferPhase.OFFERED -> {
                Text("对方正在等待你接收", fontSize = 11.sp, color = p.accent)
                Row {
                    TextButton(onClick = {
                        ClipRuntime.respondTransfer(context, transfer.id, accept = true)
                    }) { Text("接收") }
                    TextButton(onClick = {
                        ClipRuntime.respondTransfer(context, transfer.id, accept = false)
                    }) { Text("拒绝") }
                }
            }
            TransferPhase.ACTIVE -> {
                val percent = if (transfer.totalBytes > 0) {
                    (transfer.transferredBytes * 100 / transfer.totalBytes).coerceIn(0, 100)
                } else {
                    0
                }
                val filePart = if (transfer.fileCount > 0) {
                    " · 文件 ${transfer.fileIndex.coerceAtLeast(0) + 1}/${transfer.fileCount}"
                } else {
                    ""
                }
                Text(
                    "$percent% · ${formatTransferBytes(transfer.transferredBytes)} / " +
                        "${formatTransferBytes(transfer.totalBytes)}$filePart",
                    fontSize = 11.sp,
                    color = p.barDimFg,
                )
                TextButton(onClick = { ClipRuntime.cancelTransfer(transfer.id) }) { Text("取消") }
            }
            TransferPhase.COMPLETED -> {
                Text(
                    if (transfer.outgoing) "已发送" else "已接收",
                    fontSize = 11.sp,
                    color = p.accent,
                )
                val dir = transfer.savedDir
                if (!transfer.outgoing && !dir.isNullOrEmpty()) {
                    TextButton(onClick = { note(openSavedFolder(context, dir)) }) {
                        Text("打开文件夹")
                    }
                }
            }
            TransferPhase.FAILED -> Text(
                "失败：${transfer.reason.ifEmpty { "未知原因" }}",
                fontSize = 11.sp,
                color = p.accent,
            )
            TransferPhase.CANCELED -> Text("已取消", fontSize = 11.sp, color = p.barDimFg)
        }
    }
}

@Composable
private fun SectionTitle(text: String) {
    Text(
        text,
        fontSize = 12.sp,
        fontWeight = FontWeight.Bold,
        color = LocalKeyboardPalette.current.panelFg,
        modifier = Modifier.padding(top = 8.dp, bottom = 4.dp),
    )
}

@Composable
private fun Hint(text: String, accent: Boolean = false) {
    val p = LocalKeyboardPalette.current
    Text(
        text,
        fontSize = 11.sp,
        color = if (accent) p.accent else p.barDimFg,
        modifier = Modifier.fillMaxWidth().padding(vertical = 3.dp),
    )
}

@Composable
private fun ActionRow(label: String, hint: String, onClick: () -> Unit) {
    val p = LocalKeyboardPalette.current
    Row(
        modifier = Modifier.fillMaxWidth()
            .clip(RoundedCornerShape(6.dp))
            .background(p.keyBg.copy(alpha = 0.35f))
            .clickable(role = Role.Button, onClick = onClick)
            .padding(horizontal = 10.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(label, fontSize = 13.sp, color = p.panelFg, modifier = Modifier.weight(1f))
        Text(hint, fontSize = 10.sp, color = p.barDimFg)
    }
}

/** 打开接收目录；没有能列目录的查看器时回退为显示路径（不静默失败）。 */
private fun openSavedFolder(context: Context, dir: String): String {
    val file = File(dir)
    if (!file.isDirectory) return "目录不存在（可能已被系统清理）：$dir"
    return try {
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
        context.startActivity(
            Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, DocumentsContract.Document.MIME_TYPE_DIR)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
            },
        )
        ""
    } catch (_: ActivityNotFoundException) {
        "已保存到：$dir"
    } catch (_: IllegalArgumentException) {
        "已保存到：$dir"
    }
}
