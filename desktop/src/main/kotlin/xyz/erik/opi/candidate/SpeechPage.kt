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
import androidx.compose.material.Text
import androidx.compose.material.TextButton
import androidx.compose.runtime.Composable
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
    val root = remember { speechModelRoot() }
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
        "识别语言（配置键 SpeechLanguage）",
        fontSize = 13.sp, color = Ink, fontWeight = FontWeight.Bold,
    )
    Spacer(Modifier.height(6.dp))
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        for (lang in SpeechLanguage.entries) {
            LayoutChip(lang.label, s.speechLanguage == lang.value) {
                apply(s.copy(speechLanguage = lang.value))
                report("识别语言：${lang.label}")
            }
        }
    }
    Spacer(Modifier.height(6.dp))
    Text(
        when (s.speechLanguage) {
            "en" -> "Whisper 仅英文：适合长句/听写；15 秒内短句可能不如「自动」，建议短句用自动。"
            "zh" -> "SenseVoice：中文优先，中英混合短句也能识别（本地、快）。"
            else -> "自动：中文与英文短句走 SenseVoice；只有明确选 English 才用 Whisper。"
        },
        fontSize = 12.sp, color = Muted,
    )

    SectionTitle("离线模型（首次使用前下载）")
    Text(
        "模型下载到本机私有目录，不进安装包；下载后逐文件校验 SHA-256。" +
            "SenseVoice 约 ${SpeechEngine.SENSEVOICE.approxMb()} MB，Whisper 约 ${SpeechEngine.WHISPER.approxMb()} MB（均含 VAD）。",
        fontSize = 12.sp, color = Muted,
    )
    ModelCard(SENSEVOICE_MODEL, root, report)
    ModelCard(WHISPER_MODEL, root, report)
    Text("模型目录：$root", fontSize: 11.sp, color = Muted)
    Text(
        "VAD：Silero（MIT）。Whisper 权重：MIT（OpenAI）。sherpa-onnx 运行时：Apache-2.0。",
        fontSize: 11.sp, color = Muted,
    )
}

@Composable
private fun ModelCard(model: SpeechModel, root: java.nio.file.Path, report: (String) -> Unit) {
    val state = SpeechModels.state
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
                if (ready) "已下载" else "未下载",
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
        } else {
            TextButton(
                enabled = !running,
                onClick = {
                    SpeechModels.start(model.engine, root)
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
