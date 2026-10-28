// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import xyz.erik.opi.keyboard.LocalKeyboardPalette

/**
 * 引擎装载期的占位键面（2026-10-28）：真实键盘要等 [xyz.erik.opi.engine.EngineBoot]
 * 就绪后才组合，见那里的说明。配色取缺省浅色板 —— 皮肤偏好要等装载完才恢复。
 */
@Composable
fun EngineLoadingView() {
    val palette = LocalKeyboardPalette.current
    Box(Modifier.fillMaxSize().background(palette.barBg), contentAlignment = Alignment.Center) {
        Text("正在加载词库…", color = palette.barDimFg)
    }
}
