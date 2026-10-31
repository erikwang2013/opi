// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.asr

import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xyz.erik.opi.keyboard.OpiIcons

private val Scrim = Color.Black.copy(alpha = 0.45f)
private val BarColor = Color(0xF01B1F24)
private val ActionColor = Color(0xFFE7EEF8)
private val ActionContent = Color(0xFF11161C)
private val LevelColor = Color(0xFF7FDBB6)

/**
 * IME 侧接线：提供话筒入口 + 在键盘上叠录音浮层。
 * 库不可用（32 位 ABI）时 [AsrMic.available]=false，六个空格键保持普通「空格」键面。
 */
@Composable
internal fun AsrKeyboardHost(content: @Composable () -> Unit) {
    val mic = remember { AsrMic(AsrNative.loaded) { AsrRuntime.start() } }
    CompositionLocalProvider(LocalAsrMic provides mic) {
        Box(Modifier.fillMaxSize()) {
            content()
            AsrOverlay(
                state = AsrRuntime.uiState,
                onStop = { AsrRuntime.stop() },
                onCancel = { AsrRuntime.cancel() },
                onOpenSettings = { AsrRuntime.openSettings() },
                onRequestPermission = { AsrRuntime.requestPermission() },
            )
        }
    }
}

/**
 * 录音浮层：盖住键盘（不盖被输入应用），底部悬浮条。
 * 阶段/文案与按钮全由 [AsrUiState] 决定；[onStop] 只结束收音，[onCancel] 丢弃本次结果。
 */
@Composable
internal fun AsrOverlay(
    state: AsrUiState,
    onStop: () -> Unit,
    onCancel: () -> Unit,
    onOpenSettings: () -> Unit,
    onRequestPermission: () -> Unit,
) {
    if (state.phase == AsrPhase.IDLE) return
    val context = LocalContext.current
    val pack = AsrModels.selected(context, state.whisper)
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomCenter) {
        // 不吃掉手势的话，点浮层空隙会点到下面的键。
        Box(Modifier.fillMaxSize().background(Scrim).consumeAllGestures())
        Surface(
            modifier = Modifier
                .fillMaxWidth()
                .padding(12.dp)
                .widthIn(max = 520.dp),
            shape = RoundedCornerShape(20.dp),
            color = BarColor,
            contentColor = Color.White,
            shadowElevation = 8.dp,
        ) {
            Column(Modifier.padding(16.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    if (state.phase == AsrPhase.TRANSCRIBING) {
                        CircularProgressIndicator(
                            modifier = Modifier.size(22.dp),
                            strokeWidth = 2.dp,
                            color = Color.White,
                        )
                    } else {
                        Icon(
                            OpiIcons.Mic,
                            contentDescription = null,
                            tint = Color.White,
                            modifier = Modifier.size(24.dp),
                        )
                    }
                    Spacer(Modifier.size(10.dp))
                    Column(Modifier.weight(1f)) {
                        Text(titleOf(state), fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
                        Text(
                            subtitleOf(state, pack),
                            fontSize = 13.sp,
                            color = Color.White.copy(alpha = 0.72f),
                        )
                    }
                }
                if (state.phase == AsrPhase.LISTENING) {
                    Spacer(Modifier.height(12.dp))
                    LinearProgressIndicator(
                        progress = { asrLevelFraction(state.levelDb) },
                        modifier = Modifier.fillMaxWidth().height(6.dp),
                        color = LevelColor,
                        trackColor = Color.White.copy(alpha = 0.18f),
                    )
                }
                Spacer(Modifier.height(12.dp))
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.End,
                ) {
                    when (state.phase) {
                        AsrPhase.LISTENING -> {
                            DarkTextButton("取消", onCancel)
                            DarkTextButton("停止", onStop)
                        }
                        AsrPhase.PREPARING, AsrPhase.TRANSCRIBING -> DarkTextButton("取消", onCancel)
                        AsrPhase.MODEL_MISSING -> {
                            DarkTextButton("关闭", onCancel)
                            PrimaryButton("去下载", onOpenSettings)
                        }
                        AsrPhase.PERMISSION_DENIED -> {
                            DarkTextButton("关闭", onCancel)
                            PrimaryButton("去授权", onRequestPermission)
                        }
                        AsrPhase.ERROR -> PrimaryButton("知道了", onCancel)
                        AsrPhase.IDLE -> Unit
                    }
                }
            }
        }
    }
}

private fun titleOf(state: AsrUiState): String = when (state.phase) {
    AsrPhase.PREPARING -> "正在准备本地模型…"
    AsrPhase.LISTENING -> "正在聆听"
    AsrPhase.TRANSCRIBING -> "正在识别…"
    AsrPhase.MODEL_MISSING -> "语音模型未下载"
    AsrPhase.PERMISSION_DENIED -> "需要麦克风权限"
    AsrPhase.ERROR -> "语音输入失败"
    AsrPhase.IDLE -> ""
}

private fun subtitleOf(state: AsrUiState, pack: AsrModelPack): String = when (state.phase) {
    AsrPhase.PREPARING -> state.message ?: "首次使用需加载约 228 MB 本地模型"
    AsrPhase.LISTENING -> "说完点“停止”；取消不会上屏"
    AsrPhase.TRANSCRIBING -> "本地离线识别，请稍候"
    AsrPhase.MODEL_MISSING ->
        "${pack.title} ${AsrModels.mb(pack.bytes + AsrModels.VAD_BYTES)}，" +
            "前往设置下载（建议 Wi-Fi，私有目录，校验 SHA-256）"
    AsrPhase.PERMISSION_DENIED -> state.message ?: "需要麦克风权限才能语音输入"
    AsrPhase.ERROR -> state.message ?: "请重试"
    AsrPhase.IDLE -> ""
}

@Composable
private fun DarkTextButton(text: String, onClick: () -> Unit) {
    TextButton(
        onClick = onClick,
        colors = ButtonDefaults.textButtonColors(contentColor = Color.White),
    ) { Text(text) }
}

@Composable
private fun PrimaryButton(text: String, onClick: () -> Unit) {
    Button(
        onClick = onClick,
        colors = ButtonDefaults.buttonColors(
            containerColor = ActionColor,
            contentColor = ActionContent,
        ),
    ) { Text(text) }
}

private fun Modifier.consumeAllGestures(): Modifier = pointerInput(Unit) {
    awaitEachGesture {
        awaitFirstDown(requireUnconsumed = false).consume()
        var pressed = true
        while (pressed) {
            val event = awaitPointerEvent()
            event.changes.forEach { it.consume() }
            pressed = event.changes.any { it.pressed }
        }
    }
}
