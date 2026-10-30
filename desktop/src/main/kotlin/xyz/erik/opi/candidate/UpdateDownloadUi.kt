// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「关于」页的应用内下载段（2026-10-31）：全量/热更新两个入口、进度条、取消、
//! SHA-256 结果与打开文件/文件夹。下载器在 UpdateDownload.kt；本文件只管画。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

@Composable
internal fun UpdateDownloadSection(
    downloader: UpdateDownloader,
    available: UpdateCheck.Available,
    onBrowser: (String) -> Unit,
) {
    val state = downloader.state
    val busy = state is DownloadState.Running || state is DownloadState.Verifying
    val full = available.full
    val hot = available.hot
    var note by remember { mutableStateOf("") }
    if (full == null && hot == null) {
        Text(
            "前往下载页：" + available.download,
            fontSize = 12.sp,
            color = SymAccent,
            modifier = Modifier.clickable { onBrowser(available.download) },
        )
        return
    }
    if (full != null) {
        DownloadLink(UpdatePackageKind.FULL, full, available.latest, busy, downloader)
    }
    if (hot != null) {
        DownloadLink(UpdatePackageKind.HOT, hot, available.latest, busy, downloader)
        Text(
            "热更新包：关闭小欧后解压覆盖安装目录的 app 文件夹（Linux 需 sudo）；只下载，不自动替换。",
            fontSize = 11.sp, color = Muted,
        )
    }
    when (state) {
        DownloadState.Idle -> Unit
        is DownloadState.Running -> {
            val total = state.total
            val pct = state.fraction?.let { "（${(it * 100).toInt()}%）" } ?: ""
            Text(
                "正在下载${state.kind.label}：${formatDownloadSize(state.downloaded)} / ${formatDownloadSize(total)}$pct",
                fontSize = 12.sp, color = Ink,
            )
            Spacer(Modifier.height(4.dp))
            Box(Modifier.fillMaxWidth().height(6.dp).background(Hairline)) {
                val fraction = state.fraction
                if (fraction != null) {
                    Box(Modifier.fillMaxWidth(fraction).fillMaxHeight().background(Teal))
                } else {
                    Box(Modifier.fillMaxWidth(0.25f).fillMaxHeight().background(Teal.copy(alpha = 0.55f)))
                }
            }
            Spacer(Modifier.height(3.dp))
            Text(
                "取消下载",
                fontSize = 12.sp, color = Muted,
                modifier = Modifier.clickable { downloader.cancel() },
            )
        }
        is DownloadState.Verifying -> Text(
            "正在校验 ${state.kind.label} 的 SHA-256…",
            fontSize = 12.sp, color = Ink,
        )
        is DownloadState.Done -> {
            Text(
                "已下载${state.kind.label}：${state.file.name}",
                fontSize = 12.sp, color = Ink,
            )
            Text(
                when (state.shaOk) {
                    true -> "SHA-256 校验通过"
                    null -> "清单未提供 SHA-256（未校验）"
                    false -> "SHA-256 校验失败（文件不会保留）"
                },
                fontSize = 11.sp, color = if (state.shaOk == true) Teal else Muted,
            )
            Row {
                Text(
                    "打开文件", fontSize = 12.sp, color = SymAccent,
                    modifier = Modifier.clickable { downloader.openFile(state.file)?.let { note = it } }
                        .padding(end = 14.dp),
                )
                Text(
                    "打开所在文件夹", fontSize = 12.sp, color = SymAccent,
                    modifier = Modifier.clickable { downloader.openFolder(state.file)?.let { note = it } },
                )
            }
            if (note.isNotEmpty()) {
                Text(note, fontSize = 11.sp, color = Muted)
            }
        }
        is DownloadState.Failed -> {
            Text(
                "下载失败（${state.kind.label}）：${state.reason}",
                fontSize = 12.sp, color = Muted,
            )
            val pkg = when (state.kind) {
                UpdatePackageKind.FULL -> full
                UpdatePackageKind.HOT -> hot
            }
            if (pkg != null) {
                Text(
                    "重试", fontSize = 12.sp, color = SymAccent, fontWeight = FontWeight.Bold,
                    modifier = Modifier.clickable { downloader.start(state.kind, pkg, available.latest) },
                )
            }
        }
    }
}

@Composable
private fun DownloadLink(
    kind: UpdatePackageKind,
    pkg: UpdatePackage,
    version: String,
    busy: Boolean,
    downloader: UpdateDownloader,
) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            "下载${kind.label}",
            fontSize = 12.sp,
            fontWeight = FontWeight.Bold,
            color = if (busy) Muted else SymAccent,
            modifier = if (busy) Modifier else Modifier.clickable {
                downloader.start(kind, pkg, version)
            },
        )
        Text("  " + formatDownloadSize(pkg.size), fontSize = 11.sp, color = Muted)
    }
}
