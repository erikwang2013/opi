// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xyz.erik.opi.clip.ClipRouting
import xyz.erik.opi.clip.ClipRuntime
import xyz.erik.opi.clip.ClipTransfer
import xyz.erik.opi.clip.TransferActivity
import xyz.erik.opi.clip.TransferPhase
import xyz.erik.opi.clip.formatTransferBytes
import xyz.erik.opi.clip.openSavedFolder
import xyz.erik.opi.clip.transferSaveSubdirError
import xyz.erik.opi.i18n.Text

/**
 * 设置 → 传输里的「发送 / 接收 / 传输记录」（2026-11-04 用户要求与键盘传输面板对齐）。
 *
 * 逻辑全部复用 [ClipRuntime] 与 [TransferActivity]：文件/文件夹/媒体/文本走同一个
 * 透明 Activity（SAF 结果由它交给内核），剪贴板走 publish_text；入站 offer 直接
 * 在记录里接收/拒绝，接收落点由 [ClipRuntime.respondTransfer] 计算并导入公共 Download/opi。
 */
@Composable
internal fun TransferSettingsSection(toast: (String) -> Unit) {
    val context = LocalContext.current
    val connected = ClipRuntime.peers.filter { it.trusted && it.connected }
    val target = connected.firstOrNull { it.instanceId == ClipRouting.selectedPeer }
        ?: connected.firstOrNull()
    var folderNote by remember { mutableStateOf("") }
    val prefs = remember { ImePreferences(AndroidPrefStore(context)) }
    var saveSubdirDraft by remember { mutableStateOf(prefs.transferSaveSubdir) }

    LaunchedEffect(connected.joinToString { it.instanceId }) {
        if (target != null && target.instanceId != ClipRouting.selectedPeer) {
            ClipRouting.select(target.instanceId)
        }
    }

    ListItem(
        headlineContent = { Text("发送与接收") },
        supportingContent = { Text("文件/文件夹/媒体/文本/剪贴板只发给已配对且已连接的设备") },
    )
    if (connected.isEmpty()) {
        ListItem(
            headlineContent = { Text("没有已连接的设备") },
            supportingContent = { Text("先在上方完成配对；对方也要打开传输功能") },
        )
    } else {
        for (peer in connected) {
            ListItem(
                headlineContent = { Text(peer.name.ifEmpty { peer.instanceId.take(8) }) },
                supportingContent = { Text(peer.address.ifEmpty { "已连接" }) },
                trailingContent = {
                    TextButton(onClick = { ClipRouting.select(peer.instanceId) }) {
                        Text(if (peer == target) "已选" else "选择")
                    }
                },
            )
        }
        ListItem(
            headlineContent = { Text("只和选中设备收发文本/剪贴板") },
            supportingContent = { Text("关闭时发给所有已配对设备，也接收所有设备") },
            trailingContent = {
                Switch(
                    checked = ClipRouting.onlySelected,
                    onCheckedChange = { ClipRouting.setOnlySelectedMode(it) },
                )
            },
        )
    }

    SendRow("照片 / 视频", "系统选择器，可多选", target != null) {
        target?.let { TransferActivity.start(context, TransferActivity.MODE_MEDIA, it.instanceId) }
    }
    SendRow("文件", "任意文件，可多选", target != null) {
        target?.let { TransferActivity.start(context, TransferActivity.MODE_FILES, it.instanceId) }
    }
    SendRow("文件夹", "选择整棵目录树", target != null) {
        target?.let { TransferActivity.start(context, TransferActivity.MODE_FOLDER, it.instanceId) }
    }
    SendRow("文本", "输入一段文字发送", target != null) {
        TransferActivity.start(context, TransferActivity.MODE_TEXT)
    }
    SendRow("剪贴板", "发送当前系统剪贴板的纯文本", target != null) {
        toast(ClipRuntime.publishCurrentClipboard(context))
    }
    ListItem(
        headlineContent = { Text("接收目录") },
        supportingContent = { Text("公共 Download 下的自定义子目录；留空 = Download/opi") },
    )
    OutlinedTextField(
        value = saveSubdirDraft,
        onValueChange = { saveSubdirDraft = it },
        singleLine = true,
        label = { Text("子目录，例如 我的传输") },
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
    )
    transferSaveSubdirError(saveSubdirDraft)?.let {
        Text(it, color = androidx.compose.ui.graphics.Color.Red)
    }
    Row {
        TextButton(onClick = {
            val err = transferSaveSubdirError(saveSubdirDraft)
            if (err != null) {
                toast(err)
            } else {
                prefs.transferSaveSubdir = saveSubdirDraft.trim()
                toast(if (saveSubdirDraft.isBlank()) "已恢复默认接收目录" else "接收目录已保存")
            }
        }) { Text("保存目录") }
        if (saveSubdirDraft.isNotEmpty()) {
            TextButton(onClick = {
                saveSubdirDraft = ""
                prefs.transferSaveSubdir = ""
                toast("已恢复默认接收目录")
            }) { Text("恢复默认") }
        }
    }
    if (ClipRuntime.transferMessage.isNotEmpty()) {
        ListItem(headlineContent = { Text(ClipRuntime.transferMessage) })
    }

    HorizontalDivider()
    ListItem(
        headlineContent = { Text("传输记录") },
        supportingContent = { Text("最近的发送与接收；接收文件保存到系统公共 Download/opi") },
    )
    if (ClipRuntime.transfers.isEmpty()) {
        ListItem(headlineContent = { Text("暂无传输记录") })
    } else {
        for (transfer in ClipRuntime.transfers) {
            TransferRecord(transfer) { folderNote = it }
            HorizontalDivider()
        }
    }
    if (folderNote.isNotEmpty()) {
        ListItem(headlineContent = { Text(folderNote) })
    }
}

@Composable
private fun SendRow(label: String, hint: String, enabled: Boolean, onClick: () -> Unit) {
    ListItem(
        headlineContent = { Text(label) },
        supportingContent = { Text(hint) },
        trailingContent = {
            TextButton(enabled = enabled, onClick = onClick) { Text("发送") }
        },
    )
}

@Composable
private fun TransferRecord(transfer: ClipTransfer, note: (String) -> Unit) {
    val context = LocalContext.current
    ListItem(
        headlineContent = {
            Text(
                (if (transfer.outgoing) "发送给 " else "来自 ") + transfer.peerName,
                fontWeight = FontWeight.SemiBold,
            )
        },
        supportingContent = {
            Column {
                Text(
                    "${transfer.files.size} 项 · ${formatTransferBytes(transfer.totalBytes)}",
                    fontSize = 12.sp,
                )
                when (transfer.phase) {
                    TransferPhase.OFFERED -> {
                        Text("对方正在等待你接收")
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
                        LinearProgressIndicator(
                            progress = { percent / 100f },
                            modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp).height(4.dp),
                        )
                        Text(
                            "$percent% · ${formatTransferBytes(transfer.transferredBytes)} / " +
                                formatTransferBytes(transfer.totalBytes),
                            fontSize = 12.sp,
                        )
                        TextButton(onClick = { ClipRuntime.cancelTransfer(transfer.id) }) {
                            Text("取消")
                        }
                    }
                    TransferPhase.COMPLETED -> {
                        Text(if (transfer.outgoing) "已发送" else "已接收")
                        val dir = transfer.savedDir
                        if (!transfer.outgoing && !dir.isNullOrEmpty()) {
                            TextButton(onClick = { note(openSavedFolder(context, dir)) }) {
                                Text("打开文件夹")
                            }
                        }
                    }
                    TransferPhase.FAILED -> Text(
                        "失败：${transfer.reason.ifEmpty { "未知原因" }}",
                        color = androidx.compose.material3.MaterialTheme.colorScheme.error,
                    )
                    TransferPhase.CANCELED -> Text("已取消")
                }
            }
        },
    )
}
