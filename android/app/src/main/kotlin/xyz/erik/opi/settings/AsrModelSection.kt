// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xyz.erik.opi.asr.AsrDownloadState
import xyz.erik.opi.asr.AsrModelDownloader
import xyz.erik.opi.asr.AsrModels

/**
 * 离线语音模型（不随 APK 分发）。SenseVoice 中英混识，Whisper 仅英文高精度；
 * 两者都依赖同一个 silero VAD。下载到 `filesDir/asr`，逐文件校验 SHA-256。
 */
@Composable
internal fun AsrModelSection(prefs: ImePreferences) {
    if (!AsrModels.abiSupported()) {
        ListItem(
            headlineContent = { Text("离线语音输入") },
            supportingContent = { Text("当前设备 ABI 不支持（仅 arm64-v8a / x86_64）") },
        )
        HorizontalDivider()
        return
    }
    val context = LocalContext.current
    val download = AsrModelDownloader.state
    var whisper by remember { mutableStateOf(prefs.asrWhisper) }
    val busy = download is AsrDownloadState.Running
    val vadReady = AsrModels.vadReady(context)

    ListItem(
        headlineContent = { Text("离线语音输入") },
        supportingContent = {
            Text("长按空格键录音，识别全部在本机完成；模型不随安装包分发。")
        },
    )
    for (pack in AsrModels.packs) {
        val ready = AsrModels.packReady(context, pack)
        val running = (download as? AsrDownloadState.Running)?.takeIf { it.packId == pack.id }
        val failed = (download as? AsrDownloadState.Failed)?.takeIf { it.packId == pack.id }
        ListItem(
            headlineContent = { Text(pack.title) },
            supportingContent = {
                Column {
                    Text(
                        if (ready) {
                            "已就绪 · ${AsrModels.mb(pack.bytes)}"
                        } else {
                            "未下载 · 约 ${AsrModels.mb(pack.bytes + AsrModels.VAD_BYTES)} · 建议 Wi-Fi"
                        },
                    )
                    Text("SHA-256 ${pack.sha256.take(16)}…", fontSize = 12.sp)
                    if (!vadReady) Text("首次下载附带约 0.6 MB 静音检测模型", fontSize = 12.sp)
                    if (running != null) {
                        LinearProgressIndicator(
                            progress = {
                                if (running.total > 0) {
                                    running.received.toFloat() / running.total
                                } else {
                                    0f
                                }
                            },
                            modifier = Modifier.fillMaxWidth().height(4.dp),
                        )
                        Text(
                            "${running.received * 100 / running.total.coerceAtLeast(1)}%",
                            fontSize = 12.sp,
                        )
                    }
                    if (failed != null) {
                        Text(
                            "下载失败：${failed.message}",
                            color = MaterialTheme.colorScheme.error,
                            fontSize = 12.sp,
                        )
                    }
                }
            },
            trailingContent = {
                if (ready) {
                    Text("已就绪")
                } else {
                    TextButton(
                        onClick = { AsrModelDownloader.download(context, pack.id) },
                        enabled = !busy,
                    ) { Text(if (running != null) "下载中" else "下载") }
                }
            },
        )
    }
    ListItem(
        headlineContent = { Text("英文高精度（Whisper）") },
        supportingContent = {
            Text("开：仅英文，模型约 358 MB，延迟约 15 秒/条；关：SenseVoice 中英混识，更快。")
        },
        trailingContent = {
            Switch(
                checked = whisper,
                onCheckedChange = {
                    whisper = it
                    prefs.asrWhisper = it
                },
            )
        },
    )
    Text(
        text = "模型存于应用私有目录 files/asr，卸载应用即删除；下载完成前会逐文件校验 SHA-256。",
        fontSize = 12.sp,
        modifier = Modifier.fillMaxWidth().padding(16.dp),
    )
    HorizontalDivider()
}
