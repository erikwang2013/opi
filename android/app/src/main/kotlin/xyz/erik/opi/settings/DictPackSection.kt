// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * 基础包的可选词库下载入口（2026-11-01 三档打包）。普通/完整包已随安装包提供，
 * 这一节显示「随包提供」；基础包下载后解压到 filesDir，重开键盘即可用。
 */
@Composable
internal fun DictPackSection() {
    val context = LocalContext.current
    val installed = remember(DictPack.state) { DictPack.installed(context) }
    val packaged = remember {
        runCatching { context.assets.open("trad.opid").use { true } }.getOrDefault(false)
    }
    val state = DictPack.state
    val running = state as? DictPack.State.Running
    val failed = state as? DictPack.State.Failed

    ListItem(
        headlineContent = { Text("扩展词库") },
        supportingContent = {
            Column {
                Text(
                    when {
                        installed -> "已安装 · 繁体 / 笔画 / 五笔 / 生僻字索引"
                        packaged -> "随安装包提供 · 繁体 / 笔画 / 五笔 / 生僻字索引"
                        else -> "未安装 · 约 ${DictPack.mb(DictPack.BYTES)} · 含繁体、笔画、五笔与生僻字索引"
                    },
                )
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
                installed -> Text("已安装")
                packaged -> Text("已内置")
                else -> TextButton(
                    onClick = { DictPack.download(context) },
                    enabled = running == null,
                ) { Text(if (running != null) "下载中" else "下载") }
            }
        },
    )
    HorizontalDivider()
}
