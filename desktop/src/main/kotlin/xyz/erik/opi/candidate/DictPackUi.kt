// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「词库」页里的基础包词库包下载卡（2026-11-03 从语音页移来）。
//! 语音引擎/模型只属于语音页；词库包只属于词库页。

package xyz.erik.opi.candidate

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

@Composable
internal fun DictPackSection() {
    val dictState = DictPack.state
    val dictInstalled = remember(dictState) { DictPack.installed() }
    val dictPackaged = remember { DictPack.packaged() }
    val dictRunning = dictState as? DictPack.State.Running
    val dictFailed = dictState as? DictPack.State.Failed

    Column(Modifier.fillMaxWidth().padding(vertical = 6.dp)) {
        SectionTitle("基础包词库包")
        Text(
            when {
                dictInstalled -> "已下载：拼音主词库 / 繁体 / 笔画 / 五笔 / 生僻字索引（重开输入法生效）。"
                dictPackaged -> "当前安装包已带全部词库，无需下载。"
                else -> "基础包只有 app 代码，不随包任何词库。下载约 ${DictPack.mb(DictPack.BYTES)} MB，" +
                    "解压到 ${DictPack.installRoot()}，重开输入法生效。"
            },
            fontSize = 12.sp, color = Muted,
        )
        if (dictRunning != null) {
            Text(
                "下载中 ${dictRunning.received * 100 / dictRunning.total.coerceAtLeast(1)}%" +
                    "（${dictRunning.received / 1_048_576} / ${dictRunning.total / 1_048_576} MB）",
                fontSize = 12.sp, color = Ink,
            )
        }
        if (dictFailed != null) {
            Text("下载失败：${dictFailed.message}", fontSize = 12.sp, color = Color(0xFFB3261E))
        }
        if (!dictInstalled && !dictPackaged) {
            TextButton(enabled = dictRunning == null, onClick = { DictPack.download() }) {
                Text(
                    if (dictRunning != null) "下载中" else "下载词库包（含拼音主词库）",
                    color = Teal, fontWeight = FontWeight.SemiBold,
                )
            }
        }
    }
}
