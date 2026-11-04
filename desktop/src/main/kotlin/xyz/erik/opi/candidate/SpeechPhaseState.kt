// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏话筒闪烁用的语音阶段。单独放在这里，避免在按钮组合期初始化 SpeechHost：
//! 首帧所有按钮必须先画出来，语音模块出任何问题都不该让状态栏只剩 logo。

package xyz.erik.opi.candidate

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import kotlinx.coroutines.delay

@Composable
internal fun rememberSpeechPhase(): SpeechPhase {
    var phase by remember { mutableStateOf(SpeechPhase.IDLE) }
    LaunchedEffect(Unit) {
        while (true) {
            phase = SpeechHost.ui.phase
            delay(100)
        }
    }
    return phase
}
