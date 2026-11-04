// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「跨设备传输」操作区：选择目标设备，发送文件/文件夹/媒体/文本/剪贴板，
//! 处理入站 offer 与传输进度。所有耗时操作（文件扫描、对话框、控制面请求）
//! 都在 IO 线程；FFI 只由宿主线程执行，本页只发控制命令。

package xyz.erik.opi.candidate

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.draganddrop.dragAndDropTarget
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.OutlinedTextField
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.draganddrop.DragAndDropEvent
import androidx.compose.ui.draganddrop.DragAndDropTarget
import androidx.compose.ui.draganddrop.awtTransferable
import androidx.compose.ui.draw.clip
import java.awt.datatransfer.DataFlavor
import java.io.File
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

/** 设置窗的 AWT 窗口（拖拽入站用；Compose 的 LocalWindow 是 internal）。 */
internal val LocalSettingsWindow = staticCompositionLocalOf<java.awt.Window?> { null }

@Composable
internal fun TransferPage(status: ClipStatus, report: (String) -> Unit) {
    val scope = rememberCoroutineScope()
    val targets = sendablePeers(status.peers)
    var target by remember { mutableStateOf(ClipRouteUi.target) }
    var onlySelected by remember { mutableStateOf(ClipRouteUi.onlySelected) }
    // 首次打开不急着下发路由：状态栏宿主里的路由可能来自上一次设置，别把它清成广播。
    var routeTouched by remember { mutableStateOf(false) }
    var text by remember { mutableStateOf("") }
    var picker by remember { mutableStateOf<TransferPickerMode?>(null) }
    var saveDirFor by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(targets.joinToString { it.instanceId }) {
        if (targets.none { it.instanceId == target }) {
            target = targets.firstOrNull()?.instanceId.orEmpty()
        }
    }
    // 文本/剪贴板路由：只和选中设备收发；关闭时保持内核的广播/全收行为。
    LaunchedEffect(target, onlySelected, targets.isEmpty(), routeTouched) {
        if (!routeTouched) return@LaunchedEffect
        if (targets.isEmpty() && onlySelected) {
            onlySelected = false
            return@LaunchedEffect
        }
        ClipRouteUi.target = target
        ClipRouteUi.onlySelected = onlySelected
        val peer = if (onlySelected && targets.isNotEmpty()) target else ""
        withContext(Dispatchers.IO) { ClipClient.setTextRoutes(peer, peer) }
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

    var dropActive by remember { mutableStateOf(false) }
    val dropSend = rememberUpdatedState<(List<Path>) -> Unit> { sendPaths(it) }
    @OptIn(ExperimentalFoundationApi::class, ExperimentalComposeUiApi::class)
    val dropTarget = remember {
        object : DragAndDropTarget {
            override fun onEntered(event: DragAndDropEvent) {
                dropActive = true
            }

            override fun onExited(event: DragAndDropEvent) {
                dropActive = false
            }

            override fun onEnded(event: DragAndDropEvent) {
                dropActive = false
            }

            override fun onDrop(event: DragAndDropEvent): Boolean {
                dropActive = false
                @Suppress("UNCHECKED_CAST")
                val paths = runCatching {
                    (event.awtTransferable.getTransferData(DataFlavor.javaFileListFlavor)
                        as List<File>).map { it.toPath() }
                }.getOrDefault(emptyList())
                if (paths.isEmpty()) return false
                dropSend.value(paths)
                return true
            }
        }
    }
    @OptIn(ExperimentalFoundationApi::class, ExperimentalComposeUiApi::class)
    val dropModifier = Modifier.dragAndDropTarget(
        shouldStartDragAndDrop = { event ->
            event.awtTransferable.isDataFlavorSupported(DataFlavor.javaFileListFlavor)
        },
        target = dropTarget,
    )

    fun choose(mode: TransferPickerMode) {
        if (target.isEmpty()) {
            report("请先选择一台已配对且在线的设备")
            return
        }
        picker = mode
    }

    fun accept(t: TransferUi) {
        scope.launch {
            val dir = defaultTransferSaveDir().toString()
            val ok = withContext(Dispatchers.IO) { ClipClient.respondTransfer(t.id, true, dir) }
            report(if (ok) "已接受，保存到 $dir" else "接收命令未送达宿主")
        }
    }

    fun chooseSaveDir(t: TransferUi) {
        saveDirFor = t.id
        picker = TransferPickerMode.FOLDER
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

    Box(Modifier.fillMaxWidth().then(dropModifier)) {
    Column(Modifier.fillMaxWidth()) {
    SectionTitle("传输")
    Text(
        "文件/文件夹/媒体发给选中的已配对设备；文本与剪贴板默认发给所有已信任设备，" +
            "打开“只和选中设备收发”后只与选中设备互通。" +
            "单次最多 $TRANSFER_MAX_ENTRIES 项、单文件 2 GiB、总计 8 GiB。",
        fontSize = 12.sp, color = Muted,
    )
    Box(
        Modifier.fillMaxWidth().padding(vertical = 6.dp)
            .clip(RoundedCornerShape(8.dp))
            .border(
                if (dropActive) 2.dp else 1.dp,
                Teal.copy(alpha = if (dropActive) 0.65f else 0.35f),
                RoundedCornerShape(8.dp),
            )
            .background(Teal.copy(alpha = if (dropActive) 0.10f else 0.04f))
            .padding(14.dp),
    ) {
        Text(
            if (dropActive) {
                "松开鼠标，发送到 ${targets.firstOrNull { it.instanceId == target }?.name ?: "已选设备"}"
            } else {
                "拖拽文件、文件夹或照片到这里发送"
            },
            fontSize = 12.sp,
            color = if (dropActive) Teal else Muted,
            fontWeight = if (dropActive) FontWeight.SemiBold else FontWeight.Normal,
        )
    }
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
                ) {
                    target = p.instanceId
                    routeTouched = true
                }
                Spacer(Modifier.width(8.dp))
            }
        }
    }
    Spacer(Modifier.height(6.dp))
    Row(verticalAlignment = Alignment.CenterVertically) {
        ClipButton(
            label = if (onlySelected) "只和选中设备收发文本/剪贴板 ✓" else "只和选中设备收发文本/剪贴板",
            enabled = targets.isNotEmpty(),
        ) {
            onlySelected = !onlySelected
            routeTouched = true
        }
    }
    Spacer(Modifier.height(6.dp))
    Column(Modifier.fillMaxWidth()) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            ClipButton("选择文件", enabled = status.running && targets.isNotEmpty()) {
                choose(TransferPickerMode.FILES)
            }
            Spacer(Modifier.width(8.dp))
            Text("任意文件，可多选", fontSize = 11.sp, color = Muted)
        }
        Spacer(Modifier.height(6.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            ClipButton("选择文件夹", enabled = status.running && targets.isNotEmpty()) {
                choose(TransferPickerMode.FOLDER)
            }
            Spacer(Modifier.width(8.dp))
            Text("整棵目录树", fontSize = 11.sp, color = Muted)
        }
        Spacer(Modifier.height(6.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            ClipButton("照片/视频", enabled = status.running && targets.isNotEmpty()) {
                choose(TransferPickerMode.MEDIA)
            }
            Spacer(Modifier.width(8.dp))
            Text("只显示图片和视频，可多选", fontSize = 11.sp, color = Muted)
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
                onAccept = { accept(t) },
                onChooseDir = { chooseSaveDir(t) },
                onReject = { reject(t) },
                onCancel = { cancel(t) },
                onOpen = { open(t) },
            )
        }
    }

    picker?.let { mode ->
        TransferPickerWindow(
            mode = mode,
            initialDir = if (saveDirFor != null) defaultTransferSaveDir() else null,
            onCancel = { picker = null; saveDirFor = null },
        ) { paths ->
            val dir = paths.firstOrNull()?.toString()
            val id = saveDirFor
            picker = null
            saveDirFor = null
            if (id != null && dir != null) {
                scope.launch {
                    val ok = withContext(Dispatchers.IO) { ClipClient.respondTransfer(id, true, dir) }
                    report(if (ok) "已接受，保存到 $dir" else "接收命令未送达宿主")
                }
            } else {
                sendPaths(paths)
            }
        }
    }
    } // Column（拖拽接收区域）
    } // Box
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
