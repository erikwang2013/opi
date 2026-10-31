// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「跨设备剪贴板」设置页。
//!
//! 宿主的真实状态（对端、SAS 待确认、最近来源、FFI 加载失败）全部来自本机回环控制面
//! （ClipControl.kt）；本页不碰 FFI 句柄，也不显示剪贴板文本 —— 只显示元数据与来源。
//!
//! 隐藏操作：Wayland/GNOME 没有后台全局监听，页面会明说，并提供「发送当前剪贴板」。
//! Windows 的防火墙授权提示只写在这里，不自动加规则。

package xyz.erik.opi.candidate

import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.Text
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
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private val WarnRed = Color(0xFFB3261E)

/** 等宿主控制面就绪（刚 spawn 的状态栏进程要 ~1s）。 */
private suspend fun awaitClipHost(timeoutMs: Long = 2500): ClipStatus? {
    val deadline = System.currentTimeMillis() + timeoutMs
    while (System.currentTimeMillis() < deadline) {
        val st = withContext(Dispatchers.IO) { ClipClient.status() }
        if (st != null) return st
        delay(150)
    }
    return null
}

@Composable
internal fun ClipPage(s: OpSettings, apply: (OpSettings) -> Unit, report: (String) -> Unit) {
    val scope = rememberCoroutineScope()
    val localMode = remember { currentClipMode() }
    var status by remember { mutableStateOf<ClipStatus?>(null) }
    var probed by remember { mutableStateOf(false) }

    // 1s 一次；页面离开组合即取消。宿主可能在另一个进程，没有事件通道。
    LaunchedEffect(Unit) {
        while (true) {
            status = withContext(Dispatchers.IO) { ClipClient.status() }
            probed = true
            delay(1000)
        }
    }

    fun toggle(on: Boolean) {
        apply(s.copy(clipEnabled = on))
        if (!on) {
            report("正在关闭跨设备剪贴板…")
            scope.launch {
                withContext(Dispatchers.IO) { ClipClient.setEnabled(false) }
                report("已关闭跨设备剪贴板")
            }
            return
        }
        report("正在启动跨设备剪贴板…")
        scope.launch {
            val spawnErr = withContext(Dispatchers.IO) {
                if (statusBarRunning()) null else spawnStatusBar()
            }
            if (spawnErr != null) {
                report(spawnErr)
                return@launch
            }
            val st = awaitClipHost()
            status = st
            if (st == null) {
                report("开关已保存；宿主未运行，点「启动宿主」重试")
                return@launch
            }
            val ok = withContext(Dispatchers.IO) { ClipClient.setEnabled(true) }
            report(if (ok) "已开启：本机剪贴板变化会发到已配对设备" else "宿主未接受开启命令")
        }
    }

    fun startHost() {
        report("正在启动宿主…")
        scope.launch {
            val err = withContext(Dispatchers.IO) {
                if (statusBarRunning()) null else spawnStatusBar()
            }
            if (err != null) {
                report(err)
                return@launch
            }
            val st = awaitClipHost()
            status = st
            if (st == null) {
                report("宿主启动超时；请确认应用能从命令行启动")
            } else {
                withContext(Dispatchers.IO) { ClipClient.setEnabled(s.clipEnabled) }
                report("宿主已运行，正在跟随开关")
            }
        }
    }

    fun stopAll() {
        apply(s.copy(clipEnabled = false))
        report("已停止共享（已配对设备保留）")
        scope.launch { withContext(Dispatchers.IO) { ClipClient.stop() } }
    }

    fun pair(peer: String) {
        scope.launch {
            val ok = withContext(Dispatchers.IO) { ClipClient.pair(peer) }
            report(if (ok) "已发起配对；请核对两台设备上的 6 位数字" else "配对命令未送达宿主")
        }
    }

    fun confirm(peer: String, accept: Boolean) {
        scope.launch {
            val ok = withContext(Dispatchers.IO) { ClipClient.confirm(peer, accept) }
            if (!ok) report("配对确认未送达宿主")
        }
    }

    fun forget(peer: String) {
        scope.launch {
            val ok = withContext(Dispatchers.IO) { ClipClient.forget(peer) }
            report(if (ok) "已撤销该设备的信任" else "撤销命令未送达宿主")
        }
    }

    fun sendClipboard() {
        scope.launch {
            val text = withContext(Dispatchers.IO) { readLocalClipboardText() }
            if (text.isNullOrEmpty()) {
                report("本机剪贴板里没有文本")
                return@launch
            }
            if (text.length > CLIP_TEXT_MAX_CHARS) {
                report("文本超过 256 KiB 上限，未发送")
                return@launch
            }
            val ok = withContext(Dispatchers.IO) { ClipClient.sendText(text) }
            report(
                if (ok) "已发送 ${text.length} 字（内容不显示、不记录）" else "发送失败：宿主未运行",
            )
        }
    }

    val st = status
    val mode = st?.mode ?: localMode
    val running = st?.running == true

    SectionTitle("跨设备剪贴板")
    SettingRow(
        "开启跨设备剪贴板",
        "默认关闭。开启后，本机复制/剪切的文本发到已配对设备；远端内容只写入本机剪贴板，" +
            "不会自动粘贴。配置键名 ClipEnabled",
        s.clipEnabled,
    ) { toggle(it) }

    when {
        !probed -> Text("正在连接宿主…", fontSize = 12.sp, color = Muted)
        st == null -> {
            Text(
                "宿主未运行。剪贴板内核挂在状态栏常驻进程里（`--statusbar`），" +
                    "设置窗自己不开内核。",
                fontSize = 12.sp, color = Muted,
            )
            Spacer(Modifier.height(6.dp))
            ClipButton("启动宿主", enabled = true) { startHost() }
        }
        else -> {
            Text(
                "本机设备：${st.deviceName} · " +
                    (if (running) "运行中" else if (st.enabled) "启动中" else "已停止") +
                    " · 监听方式：" + if (mode == ClipMode.MANUAL) "手动（Wayland）" else "轮询",
                fontSize = 12.sp, color = Muted,
            )
            if (s.clipEnabled && !st.enabled) {
                Text("开关已保存，宿主将在 1 秒内跟随。", fontSize = 12.sp, color = Muted)
            }
            st.ffiError?.let { ErrorLine(it) }
            st.lastError?.let { ErrorLine(it) }
            st.lastSource?.let {
                Spacer(Modifier.height(4.dp))
                Text(
                    "最近一次来源：$it（内容已写入本机剪贴板，未自动粘贴）",
                    fontSize = 12.sp, color = Teal,
                )
            }
        }
    }

    if (mode == ClipMode.MANUAL) {
        SectionTitle("Wayland 手动发送")
        Text(
            "GNOME/Wayland 不允许后台进程全局监听剪贴板，所以这里没有自动发送。" +
                "复制文本后点下面的按钮；远端内容能否写入本机剪贴板取决于桌面授权。",
            fontSize = 12.sp, color = Muted,
        )
        Spacer(Modifier.height(6.dp))
        ClipButton("发送当前剪贴板", enabled = running) { sendClipboard() }
    }

    val pending = st?.pending.orEmpty()
    if (pending.isNotEmpty()) {
        SectionTitle("配对确认")
        for (p in pending) {
            PairingCard(
                pending = p,
                name = displayName(p.instanceId, emptyMap(), st?.peers.orEmpty()),
                enabled = running,
            ) { accept -> confirm(p.instanceId, accept) }
        }
    }

    SectionTitle("已发现设备")
    when {
        st == null -> Text("宿主运行后，这里会列出同一局域网内发现的小欧设备。", fontSize = 12.sp, color = Muted)
        st.peers.isEmpty() -> Text(
            "还没有发现设备。确认两台设备在同一局域网、都已开启本开关；" +
                "Windows 首次开启时请在防火墙弹窗里允许「专用网络」。",
            fontSize = 12.sp, color = Muted,
        )
        else -> for (p in st.peers) {
            PeerRow(
                peer = p,
                canControl = running,
                onPair = { pair(p.instanceId) },
                onForget = { forget(p.instanceId) },
            )
        }
    }

    SectionTitle("一键停止")
    Text(
        "关闭开关并释放监听端口。已配对设备仍保留信任，重新开启后可直接使用。",
        fontSize = 12.sp, color = Muted,
    )
    Spacer(Modifier.height(6.dp))
    ClipButton("停止共享", enabled = s.clipEnabled || running) { stopAll() }

    SectionTitle("安全与网络")
    Text("· 默认关闭；开启后只在局域网内通信（UDP 发现 + TCP 加密会话）。", fontSize = 12.sp, color = Muted)
    Text("· 配对必须由两端核对同一个 6 位数字；远端内容只写剪贴板，不自动粘贴。", fontSize = 12.sp, color = Muted)
    Text("· 不记录剪贴板内容，不做剪贴板历史；状态文件只存设备与开关。", fontSize = 12.sp, color = Muted)
    if (isWindows()) {
        Text(
            "· 第一次监听时 Windows 会弹防火墙授权，请允许「专用网络」。" +
                "本程序不会自动修改防火墙规则；拒绝后请到 Windows 安全中心手动放行。",
            fontSize = 12.sp, color = Muted,
        )
    }
}

@Composable
private fun ClipButton(label: String, enabled: Boolean, onClick: () -> Unit) {
    Box(
        modifier = Modifier
            .border(1.dp, if (enabled) Teal else Hairline)
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 6.dp),
    ) {
        Text(label, fontSize = 12.sp, color = if (enabled) Teal else Muted)
    }
}

@Composable
private fun ErrorLine(text: String) {
    Spacer(Modifier.height(4.dp))
    Text(text, fontSize = 12.sp, color = WarnRed, maxLines = 8)
}

@Composable
private fun PeerRow(
    peer: ClipPeer,
    canControl: Boolean,
    onPair: () -> Unit,
    onForget: () -> Unit,
) {
    Row(
        Modifier.fillMaxWidth().padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                peer.name.ifBlank { "未命名设备" },
                fontSize = 14.sp, color = Ink,
            )
            Text(
                buildString {
                    append(peer.instanceId.take(8))
                    append(if (peer.connected) " · 在线" else " · 离线")
                    append(if (peer.trusted) " · 已信任" else " · 未配对")
                    if (peer.address.isNotBlank()) append(" · ").append(peer.address.take(48))
                },
                fontSize = 11.sp, color = Muted,
            )
        }
        if (peer.trusted) {
            ClipButton("撤销", enabled = canControl) { onForget() }
        } else {
            ClipButton("配对", enabled = canControl && peer.connected) { onPair() }
        }
    }
}

@Composable
private fun PairingCard(
    pending: ClipPending,
    name: String,
    enabled: Boolean,
    onAnswer: (Boolean) -> Unit,
) {
    Column(
        Modifier.fillMaxWidth().padding(vertical = 6.dp)
            .border(1.dp, Teal.copy(alpha = 0.45f)).padding(14.dp),
    ) {
        Text(
            if (pending.incoming) "$name 请求配对" else "与 $name 配对",
            fontSize = 14.sp, color = Ink, fontWeight = FontWeight.Bold,
        )
        Text(
            pending.sas,
            fontSize = 30.sp, color = Teal, fontWeight = FontWeight.Bold,
        )
        Text(
            "请确认另一台设备显示相同的 6 位数字，再决定是否信任。",
            fontSize = 12.sp, color = Muted,
        )
        Spacer(Modifier.height(8.dp))
        Row {
            ClipButton("信任", enabled = enabled) { onAnswer(true) }
            Spacer(Modifier.width(10.dp))
            ClipButton("拒绝", enabled = enabled) { onAnswer(false) }
        }
    }
}
