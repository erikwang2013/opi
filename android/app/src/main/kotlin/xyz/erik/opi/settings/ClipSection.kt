// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ListItem
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
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
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xyz.erik.opi.clip.ClipPeer
import xyz.erik.opi.clip.ClipRuntime
import xyz.erik.opi.clip.ClipSyncService

/**
 * 设置页「跨设备剪贴板」一节（2026-11-01）。**默认关闭**：只有用户在这里打开，
 * 才拉起内核线程与 connectedDevice 前台服务。
 *
 * 配对安全：本页只展示内核算出的 6 位 SAS，必须用户手动点「数字一致，允许」；
 * 关掉对话框 = 拒绝。没有任何自动信任路径。设备名/指纹只用于识别，不是信任依据
 * （信任依据是 Noise 握手 + SAS）。
 */
@Composable
internal fun ClipSection(prefs: ImePreferences, toast: (String) -> Unit) {
    val context = LocalContext.current
    var enabled by remember { mutableStateOf(prefs.clipEnabled) }
    var confirmForget by remember { mutableStateOf<ClipPeer?>(null) }
    // 通知栏「停止」也会关掉偏好；内核运行状态变化时把开关同步回来。
    LaunchedEffect(ClipRuntime.running) { enabled = prefs.clipEnabled }
    val notificationPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted ->
        // 拒绝不阻止服务：Android 13+ 只是不显示前台服务通知。
        if (!granted) toast("未授予通知权限：共享仍会运行，但通知不可见")
    }

    fun setEnabled(on: Boolean) {
        enabled = on
        prefs.clipEnabled = on
        if (on) {
            ClipRuntime.start(context)
            ClipSyncService.start(context)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
                context.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) !=
                PackageManager.PERMISSION_GRANTED
            ) {
                notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
            }
        } else {
            ClipRuntime.stop(context)
            ClipSyncService.stop(context)
            toast("已停止跨设备剪贴板")
        }
    }

    ListItem(
        headlineContent = { Text("跨设备剪贴板") },
        supportingContent = { Text("同一局域网内已配对设备之间共享纯文本；默认关闭") },
    )
    ListItem(
        headlineContent = { Text("启用") },
        supportingContent = {
            val device = ClipRuntime.status?.deviceName.orEmpty()
            Text(
                when {
                    ClipRuntime.running && device.isNotEmpty() -> "运行中 · $device"
                    ClipRuntime.running -> "运行中"
                    enabled -> "已启用，正在启动内核…"
                    else -> "关闭：不监听剪贴板，也不联网"
                },
            )
        },
        trailingContent = { Switch(checked = enabled, onCheckedChange = ::setEnabled) },
    )

    if (enabled) {
        val fingerprint = ClipRuntime.status?.fingerprint
        if (!fingerprint.isNullOrEmpty()) {
            ListItem(
                headlineContent = { Text("本机指纹") },
                supportingContent = { Text(fingerprint.chunked(4).joinToString(" ")) },
            )
        }
        ListItem(
            headlineContent = { Text("上次收到") },
            supportingContent = {
                // 只显示来源设备，不显示内容（剪贴板内容不落到任何 UI 文案/日志）。
                Text(
                    if (ClipRuntime.lastSource.isEmpty()) "还没有收到过"
                    else "来自 ${ClipRuntime.lastSource}",
                )
            },
        )
    }
    if (ClipRuntime.lastError.isNotEmpty()) {
        ListItem(
            headlineContent = { Text("跨设备剪贴板错误") },
            supportingContent = { Text(ClipRuntime.lastError) },
        )
    }
    if (ClipRuntime.lastPairing.isNotEmpty()) {
        ListItem(
            headlineContent = { Text("配对状态") },
            supportingContent = { Text(ClipRuntime.lastPairing) },
        )
    }

    if (enabled) {
        ListItem(
            headlineContent = { Text("附近设备") },
            supportingContent = { Text("配对时双方必须核对同一组 6 位数字；未配对设备收不到内容") },
        )
        if (ClipRuntime.peers.isEmpty()) {
            ListItem(
                headlineContent = { Text("没有发现设备") },
                supportingContent = { Text("确认两台设备在同一 Wi-Fi，且两边都打开了本功能") },
            )
        }
        for (peer in ClipRuntime.peers) {
            HorizontalDivider()
            val name = peer.name.ifEmpty { peer.instanceId.take(8) }
            ListItem(
                headlineContent = { Text(name) },
                supportingContent = {
                    Text(
                        buildString {
                            append(if (peer.trusted) "已配对" else "未配对")
                            append(" · ")
                            append(if (peer.connected) "已连接" else "未连接")
                            if (peer.address.isNotEmpty()) append(" · ").append(peer.address)
                        },
                    )
                },
                trailingContent = {
                    if (peer.trusted) {
                        TextButton(onClick = { confirmForget = peer }) { Text("移除") }
                    } else {
                        TextButton(onClick = { ClipRuntime.pairStart(peer.instanceId) }) { Text("配对") }
                    }
                },
            )
        }
        HorizontalDivider()
        ListItem(
            headlineContent = { Text("停止共享") },
            supportingContent = { Text("立即断开所有会话并停止局域网监听") },
            trailingContent = {
                TextButton(onClick = { setEnabled(false) }) { Text("停止") }
            },
        )
    }

    ClipRuntime.pairingPrompt?.let { prompt ->
        val name = ClipRuntime.peers.firstOrNull { it.instanceId == prompt.instanceId }
            ?.name?.takeIf { it.isNotEmpty() }
            ?: prompt.instanceId.take(8)
        AlertDialog(
            // 返回键/点外面 = 拒绝；不可能「什么都不做」地把配对悬着。
            onDismissRequest = { ClipRuntime.pairConfirm(prompt.instanceId, accept = false) },
            title = { Text("核对配对数字") },
            text = {
                Column {
                    Text("与「$name」核对下面这组数字。两台设备显示完全相同才允许配对。")
                    Text(
                        text = prompt.sas.chunked(3).joinToString(" "),
                        fontSize = 34.sp,
                        fontWeight = FontWeight.Bold,
                        textAlign = TextAlign.Center,
                        modifier = Modifier.fillMaxWidth().padding(vertical = 16.dp),
                    )
                    Text("数字不一致说明可能存在中间人，务必拒绝。", fontSize = 12.sp)
                }
            },
            confirmButton = {
                TextButton(onClick = { ClipRuntime.pairConfirm(prompt.instanceId, accept = true) }) {
                    Text("数字一致，允许")
                }
            },
            dismissButton = {
                TextButton(onClick = { ClipRuntime.pairConfirm(prompt.instanceId, accept = false) }) {
                    Text("拒绝")
                }
            },
        )
    }

    confirmForget?.let { peer ->
        val name = peer.name.ifEmpty { peer.instanceId.take(8) }
        AlertDialog(
            onDismissRequest = { confirmForget = null },
            title = { Text("移除配对设备") },
            text = { Text("「$name」将立即断开，之后需要重新核对数字才能再同步。") },
            confirmButton = {
                TextButton(onClick = {
                    confirmForget = null
                    ClipRuntime.forgetPeer(peer.instanceId)
                    toast("已移除「$name」")
                }) { Text("移除") }
            },
            dismissButton = {
                TextButton(onClick = { confirmForget = null }) { Text("取消") }
            },
        )
    }
}
