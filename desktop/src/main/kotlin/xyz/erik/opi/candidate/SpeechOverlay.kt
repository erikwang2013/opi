// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 录音/识别浮层（2026-11-02）。独立 Window（与提示窗/状态栏同一条理由：Compose
//! Popup 会被 42dp 的条窗裁掉），**不抢焦点** —— 点话筒时用户正在别的窗口打字。
//! 内容 = 音量 / 阶段 / 临时文本 / 最终文本 / 取消 / 错误 / 模型缺失提示。
//! 音频与正文都不落日志，这里只把 Rust 事件里的文本显示出来。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.Text
import androidx.compose.material.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.rememberWindowState
import xyz.erik.opi.scene.SceneTheme
import java.awt.GraphicsEnvironment
import java.awt.MouseInfo
import java.awt.Rectangle

@Composable
internal fun SpeechOverlay(
    background: SceneTheme?,
    skin: SkinOption,
    onOpenSettings: () -> Unit,
) {
    val ui = SpeechHost.ui
    if (ui.phase == SpeechPhase.IDLE) return
    val palette = skinColors(if (background != null) background.defaultSkin() else skin)
    val screen = remember { activeScreenBounds() }
    // 首次加载 239/358 MB 模型可能几十秒：把等待时间显示出来，别像卡死。
    var loadingSeconds by remember { mutableStateOf(0) }
    LaunchedEffect(ui.phase) {
        loadingSeconds = 0
        if (ui.phase == SpeechPhase.PREPARING) {
            while (true) {
                kotlinx.coroutines.delay(1000L)
                loadingSeconds++
            }
        }
    }
    Window(
        onCloseRequest = { SpeechHost.dismiss() },
        state = rememberWindowState(width = 420.dp, height = 190.dp),
        undecorated = true,
        alwaysOnTop = true,
        resizable = false,
        // 录音时用户正在别的窗口打字：浮层只显示状态，绝不抢焦点。
        focusable = false,
        title = "小欧输入法 语音输入",
    ) {
        LaunchedEffect(window, screen) {
            statusBarGuard("语音浮层 SkipTaskbar") { SkipTaskbar.apply(window) }
            // 多显示器：放到鼠标所在屏幕底部，而不是永远主屏。
            // 80 会让浮层压住底部状态栏（用户看不到话筒/其他按钮）；抬到条上方留出空隙。
            window.setLocation(
                screen.x + (screen.width - window.width) / 2,
                screen.y + screen.height - window.height - 160,
            )
            // 不能 toFront()：AWT 的 toFront 在 Windows 上会 SetWindowPos 激活本窗，
            // 把键盘焦点从输入框抢走 —— 识别文本就再也送不回光标，只能等浮层关闭。
            // alwaysOnTop 已保证可见性。
        }
        Column(
            Modifier.fillMaxSize().background(palette.bg.copy(alpha = 0.97f))
                .border(1.dp, palette.line).padding(horizontal = 16.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("语音输入", fontSize = 16.sp, fontWeight = FontWeight.Bold, color = palette.fg)
                Spacer(Modifier.width(8.dp))
                Text(
                    if (ui.engine == SpeechEngine.WHISPER) "Whisper 英文高精度" else "SenseVoice 本地识别",
                    fontSize = 11.sp, color = palette.accent,
                )
                Spacer(Modifier.weight(1f))
                Box(Modifier.clickable { SpeechHost.dismiss() }.padding(6.dp)) {
                    Text("×", fontSize = 16.sp, color = palette.fg.copy(alpha = 0.7f))
                }
            }
            when (ui.phase) {
                SpeechPhase.PREPARING -> Text(
                    (ui.text.ifEmpty { "正在加载模型…首次加载可能需要几十秒（之后复用）" }) +
                        "（已 ${loadingSeconds} 秒）",
                    fontSize = 13.sp, color = palette.fg,
                )
                SpeechPhase.RECORDING -> {
                    // 识别正文只进光标处的输入框（移动端同款）；浮层只显示状态，
                    // 否则用户会以为识别结果停在弹框里。
                    Text("正在录音…每 3 秒自动上屏一段；点「停止」结束", fontSize = 13.sp, color = palette.fg)
                    Box(
                        Modifier.fillMaxWidth().height(6.dp)
                            .background(palette.line),
                    ) {
                        Box(
                            Modifier.fillMaxWidth(ui.level).fillMaxHeight()
                                .background(palette.accent),
                        )
                    }
                    if (ui.error.isNotEmpty()) {
                        Text(ui.error, fontSize = 12.sp, color = palette.accent)
                    }
                }
                SpeechPhase.RECOGNIZING -> Text("正在识别…", fontSize = 13.sp, color = palette.fg)
                SpeechPhase.DONE -> Text(
                    ui.text,
                    fontSize = 14.sp,
                    color = palette.fg,
                    maxLines = 4,
                )
                SpeechPhase.ERROR -> Text(ui.error, fontSize = 13.sp, color = palette.accent)
                SpeechPhase.IDLE -> {}
            }
            if (ui.phase == SpeechPhase.DONE && ui.error.isNotEmpty()) {
                Text(ui.error, fontSize = 12.sp, color = palette.accent)
            }
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.End,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (ui.phase == SpeechPhase.ERROR) {
                    TextButton(onClick = onOpenSettings) { Text("打开设置", color = palette.accent) }
                }
                when (ui.phase) {
                    SpeechPhase.RECORDING -> {
                        TextButton(onClick = { SpeechHost.stop() }) {
                            Text("停止", color = palette.accent)
                        }
                        TextButton(onClick = { SpeechHost.cancel() }) { Text("取消", color = palette.accent) }
                    }
                    SpeechPhase.PREPARING, SpeechPhase.RECOGNIZING ->
                        TextButton(onClick = { SpeechHost.cancel() }) { Text("取消", color = palette.accent) }
                    SpeechPhase.DONE, SpeechPhase.ERROR ->
                        TextButton(onClick = { SpeechHost.dismiss() }) { Text("关闭", color = palette.accent) }
                    SpeechPhase.IDLE -> {}
                }
            }
        }
    }
}

/** 鼠标所在屏幕的像素边界；取不到鼠标时退回主屏。 */
private fun activeScreenBounds(): Rectangle {
    val screens = GraphicsEnvironment.getLocalGraphicsEnvironment().screenDevices
    val mouse = runCatching { MouseInfo.getPointerInfo()?.location }.getOrNull()
    val device = screens.firstOrNull { it.defaultConfiguration.bounds.contains(mouse) }
        ?: screens.firstOrNull()
    return device?.defaultConfiguration?.bounds ?: Rectangle(0, 0, 1280, 800)
}
