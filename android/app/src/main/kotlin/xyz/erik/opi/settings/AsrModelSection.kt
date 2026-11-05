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
import xyz.erik.opi.i18n.Text
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
import xyz.erik.opi.asr.AsrBundled
import xyz.erik.opi.asr.AsrDownloadState
import xyz.erik.opi.asr.AsrEngine
import xyz.erik.opi.asr.AsrEngineDownloader
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
    val busy = download is AsrDownloadState.Running
    val vadReady = AsrModels.vadReady(context)

    ListItem(
        headlineContent = { Text("离线语音输入") },
        supportingContent = {
            Text("长按空格键录音，识别全部在本机完成。三档都内置引擎；完整包内置模型，普通/基础包在下方下载模型。")
        },
    )

    // 语音引擎（原生库）：三档都随包提供；只有模型按档位下载。
    val engineState = AsrEngineDownloader.state
    val engineReady = AsrEngine.ready(context)
    val engineRunning = engineState as? AsrEngineDownloader.State.Running
    val engineFailed = engineState as? AsrEngineDownloader.State.Failed
    ListItem(
        headlineContent = { Text("语音引擎") },
        supportingContent = {
            Column {
                Text(
                    when {
                        AsrEngine.bundled() -> "随包提供"
                        engineReady -> "已下载 · 约 ${AsrEngine.mb(AsrEngine.APPROX_BYTES)}"
                        else -> "未下载 · 约 ${AsrEngine.mb(AsrEngine.APPROX_BYTES)} · 建议 Wi-Fi"
                    },
                )
                if (engineRunning != null) {
                    LinearProgressIndicator(
                        progress = {
                            if (engineRunning.total > 0) {
                                engineRunning.received.toFloat() / engineRunning.total
                            } else {
                                0f
                            }
                        },
                        modifier = Modifier.fillMaxWidth().height(4.dp),
                    )
                    Text(
                        "${engineRunning.received * 100 / engineRunning.total.coerceAtLeast(1)}%",
                        fontSize = 12.sp,
                    )
                }
                if (engineFailed != null) {
                    Text(
                        "下载失败：${engineFailed.message}",
                        color = MaterialTheme.colorScheme.error,
                        fontSize = 12.sp,
                    )
                }
            }
        },
        trailingContent = {
            when {
                AsrEngine.bundled() -> Text("已内置")
                engineReady -> Text("已就绪")
                else -> TextButton(
                    onClick = { AsrEngineDownloader.download(context) },
                    enabled = !busy && engineRunning == null,
                ) { Text(if (engineRunning != null) "下载中" else "下载") }
            }
        },
    )

    for (pack in AsrModels.packs) {
        val ready = AsrModels.packReady(context, pack)
        val bundled = !ready && AsrBundled.ready(context, pack)
        val running = (download as? AsrDownloadState.Running)?.takeIf { it.packId == pack.id }
        val failed = (download as? AsrDownloadState.Failed)?.takeIf { it.packId == pack.id }
        ListItem(
            headlineContent = { Text(pack.title) },
            supportingContent = {
                Column {
                    Text(
                        when {
                            ready -> "已就绪 · ${AsrModels.mb(pack.bytes)}"
                            bundled -> "随安装包提供 · 首次长按空格自动准备"
                            else -> "未下载 · 约 ${AsrModels.mb(pack.bytes + AsrModels.VAD_BYTES)} · 建议 Wi-Fi"
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
                when {
                    ready -> Text("已就绪")
                    bundled -> Text("已内置")
                    else -> TextButton(
                        onClick = { AsrModelDownloader.download(context, pack.id) },
                        enabled = !busy,
                    ) { Text(if (running != null) "下载中" else "下载") }
                }
            },
        )
    }
    Text(
        text = "语音引擎跟随输入模式：中文/繁体用 SenseVoice；英文 Pro 用 Whisper，普通英文用 SenseVoice。切换模式即切换引擎。",
        fontSize = 12.sp,
        modifier = Modifier.fillMaxWidth().padding(16.dp),
    )
    Text(
        text = "模型存于应用私有目录 files/asr，卸载应用即删除；下载完成前会逐文件校验 SHA-256。",
        fontSize = 12.sp,
        modifier = Modifier.fillMaxWidth().padding(16.dp),
    )
    HorizontalDivider()
}
