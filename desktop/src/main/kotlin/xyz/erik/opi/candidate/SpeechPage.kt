// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 设置 → 语音（2026-11-02）：总开关（默认关）、语言路由（auto/zh/en）与两个
//! 离线模型的下载卡。模型不进安装包，体积/许可在这里如实写出来；下载/校验状态由
//! [SpeechModels] 单例持有，切页不丢进度。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.roundToInt

private val ErrorRed = Color(0xFFB3261E)

@Composable
internal fun SpeechPage(s: OpSettings, apply: (OpSettings) -> Unit, report: (String) -> Unit) {
    val userRoot = remember { speechModelRoot() }
    // 模型下载完成即自动启用语音（用户不用再回设置拨一次开关）。
    LaunchedEffect(SpeechModels.state.phase) {
        if (SpeechModels.state.phase == DownloadPhase.DONE && !s.speechEnabled) {
            apply(s.copy(speechEnabled = true))
        }
    }
    SectionTitle("语音输入")
    SettingRow(
        "启用语音输入",
        "本地离线识别；音频只在内存中处理，不落盘、不记录正文。默认关闭（配置键 SpeechEnabled）",
        s.speechEnabled,
    ) {
        apply(s.copy(speechEnabled = it))
        report(if (it) "语音输入已开启：点状态栏话筒开始/停止" else "语音输入已关闭")
    }
    Text(
        "引擎跟随输入模式：中文/繁体用 SenseVoice；英文 Pro 用 Whisper，普通英文用 SenseVoice。" +
            "切换输入法模式即切换引擎，默认中文引擎。",
        fontSize = 12.sp, color = Muted,
    )

    SectionTitle("语音引擎（原生库）")
    val engineState = SpeechEnginePack.state
    val engineReady = remember(engineState) { SpeechEnginePack.ready() }
    val engineRunning = engineState as? SpeechEnginePack.State.Running
    val engineFailed = engineState as? SpeechEnginePack.State.Failed
    Text(
        when {
            SpeechEnginePack.bundled() -> "已随完整包提供，无需下载。"
            engineReady -> "已下载，可直接使用。"
            !SpeechEnginePack.supported() -> "当前平台暂无语音引擎组件。"
            else -> "普通包/基础包需先下载语音引擎（约 ${SpeechEnginePack.approxMb()} MB），再下载模型。"
        },
        fontSize = 12.sp, color = Muted,
    )
    if (engineRunning != null) {
        Text(
            "下载中 ${engineRunning.received * 100 / engineRunning.total.coerceAtLeast(1)}%" +
                "（${engineRunning.received / 1_048_576} / ${engineRunning.total / 1_048_576} MB）",
            fontSize = 12.sp, color = Ink,
        )
        Box(Modifier.fillMaxWidth().height(6.dp).background(Hairline)) {
            Box(
                Modifier.fillMaxWidth(
                    if (engineRunning.total <= 0) 0f
                    else (engineRunning.received.toFloat() / engineRunning.total).coerceIn(0f, 1f),
                ).fillMaxHeight().background(Teal),
            )
        }
    }
    if (engineFailed != null) {
        Text("下载失败：${engineFailed.message}", fontSize = 12.sp, color = ErrorRed)
    }
    if (!SpeechEnginePack.bundled() && !engineReady && SpeechEnginePack.supported() &&
        engineRunning == null
    ) {
        TextButton(onClick = { SpeechEnginePack.download() }) {
            Text("下载语音引擎（约 ${SpeechEnginePack.approxMb()} MB）", color = Teal)
        }
    }

    SectionTitle("离线模型（首次使用前下载）")
    Text(
        "完整包已内置引擎与两套模型（首次使用解压）；普通/基础包需先下载引擎，再在设置里下载模型，" +
            "逐文件校验 SHA-256。SenseVoice 约 ${SpeechEngine.SENSEVOICE.approxMb()} MB，" +
            "Whisper 约 ${SpeechEngine.WHISPER.approxMb()} MB（均含 VAD）。",
        fontSize = 12.sp, color = Muted,
    )
    ModelCard(SENSEVOICE_MODEL, userRoot, report)
    ModelCard(WHISPER_MODEL, userRoot, report)
    Text("模型目录：$userRoot", fontSize = 11.sp, color = Muted)
    Text(
        "VAD：Silero（MIT）。Whisper 权重：MIT（OpenAI）。sherpa-onnx 运行时：Apache-2.0。",
        fontSize = 11.sp, color = Muted,
    )

    SectionTitle("扩展词库（基础包）")
    val dictState = DictPack.state
    val dictInstalled = remember(dictState) { DictPack.installed() }
    val dictPackaged = remember { DictPack.packaged() }
    val dictRunning = dictState as? DictPack.State.Running
    val dictFailed = dictState as? DictPack.State.Failed
    Text(
        when {
            dictInstalled -> "已下载：繁体 / 笔画 / 五笔 / 生僻字索引（重开输入法生效）。"
            dictPackaged -> "当前安装包已带扩展词库，无需下载。"
            else -> "基础包未带扩展词库：下载约 ${DictPack.mb(DictPack.BYTES)} MB，解压到 ${DictPack.installRoot()}。"
        },
        fontSize = 12.sp, color = Muted,
    )
    if (dictRunning != null) {
        Text("下载中 ${dictRunning.received * 100 / dictRunning.total.coerceAtLeast(1)}%", fontSize = 12.sp, color = Ink)
    }
    if (dictFailed != null) {
        Text("下载失败：${dictFailed.message}", fontSize = 12.sp, color = ErrorRed)
    }
    if (!dictInstalled && !dictPackaged) {
        TextButton(enabled = dictRunning == null, onClick = { DictPack.download() }) {
            Text(if (dictRunning != null) "下载中" else "下载扩展词库", color = Teal)
        }
    }
}

@Composable
private fun ModelCard(model: SpeechModel, userRoot: java.nio.file.Path, report: (String) -> Unit) {
    val state = SpeechModels.state
    val root = modelRoot(model.engine, userRoot)
    val bundled = bundledJar()
    val ready = modelReady(model.engine, root)
    val mine = state.engine == model.engine
    val running = state.phase == DownloadPhase.RUNNING
    Column(
        Modifier.fillMaxWidth().padding(vertical = 6.dp)
            .border(1.dp, Hairline).padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(model.engine.title, fontSize = 15.sp, fontWeight = FontWeight.Bold, color = Ink)
            Spacer(Modifier.width(8.dp))
            Text(
                when {
                    bundled -> "已随安装包提供"
                    ready -> "已下载"
                    else -> "未下载"
                },
                fontSize = 12.sp, color = if (ready) Teal else Muted,
            )
            Spacer(Modifier.weight(1f))
            Text("约 ${model.engine.approxMb()} MB", fontSize = 12.sp, color = Muted)
        }
        Text(model.engine.detail, fontSize = 12.sp, color = Muted)
        Text("许可：${model.license}", fontSize = 11.sp, color = Muted)
        if (mine && running) {
            Text(
                "${state.message}  ${(state.fraction * 100).roundToInt()}%" +
                    "（${state.done / 1_048_576} / ${state.total / 1_048_576} MB）",
                fontSize = 12.sp, color = Ink,
            )
            Box(Modifier.fillMaxWidth().height(6.dp).background(Hairline)) {
                Box(Modifier.fillMaxWidth(state.fraction).fillMaxHeight().background(Teal))
            }
            TextButton(onClick = { SpeechModels.cancel() }) { Text("取消下载", color = Teal) }
        } else if (bundled) {
            Text("已内置，无需下载", fontSize = 12.sp, color = Teal)
        } else {
            TextButton(
                enabled = !running,
                onClick = {
                    SpeechModels.start(model.engine, userRoot)
                    report("开始下载 ${model.engine.title}（约 ${model.engine.approxMb()} MB）")
                },
            ) {
                Text(if (ready) "校验 / 重新下载" else "下载", color = if (running) Muted else Teal)
            }
            if (mine && state.phase == DownloadPhase.DONE) {
                Text(state.message, fontSize = 12.sp, color = Teal)
            }
            if (mine && state.phase == DownloadPhase.ERROR) {
                Text(state.message, fontSize = 12.sp, color = ErrorRed)
            }
        }
    }
}
