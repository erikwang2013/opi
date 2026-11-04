// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.asr

import androidx.compose.runtime.compositionLocalOf

/** 键盘上的话筒入口；[available] = 原生库可加载（32 位 ABI 不打包 → 隐藏）。 */
internal data class AsrMic(val available: Boolean, val start: () -> Unit)

/** 六个空格键共用的入口。null（预览/单测/32 位设备）= 不显示话筒。 */
internal val LocalAsrMic = compositionLocalOf<AsrMic?> { null }

