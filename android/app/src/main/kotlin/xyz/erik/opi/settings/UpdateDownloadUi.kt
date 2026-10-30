// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Android 应用内下载控件（设置页「关于」与键盘「关于」共用，2026-10-31）：全量/
//! 热更新两个入口、进度条、取消、校验结果与安装/打开。状态源是 [UpdateDownloader]，
//! 本文件只画；两处宿主传入各自的强调色/次要色。

package xyz.erik.opi.settings

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
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

@Composable
internal fun UpdateDownloadControls(
    update: UpdateState.Newer,
    accent: Color,
    dim: Color,
    onOpenBrowser: (String) -> Unit,
    onNote: (String) -> Unit,
) {
    val context = LocalContext.current
    val state by UpdateDownloader.state.collectAsState()
    LaunchedEffect(Unit) { UpdateDownloader.resume(context) }
    val busy = state is DownloadState.Running || state is DownloadState.Verifying
    val full = update.full
    val hot = update.hot
    if (full == null && hot == null) {
        Text(
            "前往下载页",
            fontSize = 12.sp,
            color = accent,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.clickable { onOpenBrowser(update.download) },
        )
        return
    }
    Column(Modifier.fillMaxWidth()) {
        if (full != null) DownloadLink("下载全量安装包", full, busy, accent, dim) {
            UpdateDownloader.start(context, UpdatePackageKind.FULL, full, update.version)
        }
        if (hot != null) DownloadLink("下载热更新包", hot, busy, accent, dim) {
            UpdateDownloader.start(context, UpdatePackageKind.HOT, hot, update.version)
        }
        when (val s = state) {
            DownloadState.Idle -> Unit
            is DownloadState.Running -> {
                val pct = s.fraction?.let { "（${(it * 100).toInt()}%）" } ?: ""
                Text(
                    "正在下载${s.kind.label}：${formatDownloadSize(s.downloaded)} / ${formatDownloadSize(s.total)}$pct",
                    fontSize = 12.sp, color = dim,
                )
                Spacer(Modifier.height(3.dp))
                Box(Modifier.fillMaxWidth().height(5.dp).background(dim.copy(alpha = 0.25f))) {
                    val fraction = s.fraction
                    if (fraction != null) {
                        Box(Modifier.fillMaxWidth(fraction).fillMaxHeight().background(accent))
                    } else {
                        Box(Modifier.fillMaxWidth(0.25f).fillMaxHeight().background(accent.copy(alpha = 0.6f)))
                    }
                }
                Text(
                    "取消下载",
                    fontSize = 12.sp,
                    color = accent,
                    modifier = Modifier.clickable { UpdateDownloader.cancel(context) },
                )
            }
            is DownloadState.Verifying -> Text(
                "正在校验 ${s.kind.label} 的 SHA-256…", fontSize = 12.sp, color = dim,
            )
            is DownloadState.Done -> {
                Text("已下载${s.kind.label}：${s.file.name}", fontSize = 12.sp, color = dim)
                Text(
                    when (s.shaOk) {
                        true -> "SHA-256 校验通过"
                        null -> "清单未提供 SHA-256（未校验）"
                        false -> "SHA-256 校验失败（文件不会保留）"
                    },
                    fontSize = 11.sp, color = if (s.shaOk == true) accent else dim,
                )
                Row {
                    Text(
                        if (s.file.name.endsWith(".apk", ignoreCase = true)) "安装" else "打开文件",
                        fontSize = 12.sp, color = accent, fontWeight = FontWeight.Bold,
                        modifier = Modifier.clickable {
                            UpdateDownloader.open(context, s)?.let(onNote)
                        }.padding(end = 16.dp),
                    )
                    Text(
                        "取消", fontSize = 12.sp, color = dim,
                        modifier = Modifier.clickable { UpdateDownloader.cancel(context) },
                    )
                }
            }
            is DownloadState.Failed -> {
                Text("下载失败（${s.kind.label}）：${s.reason}", fontSize = 12.sp, color = dim)
                val pkg = when (s.kind) {
                    UpdatePackageKind.FULL -> full
                    UpdatePackageKind.HOT -> hot
                }
                if (pkg != null) {
                    Text(
                        "重试", fontSize = 12.sp, color = accent, fontWeight = FontWeight.Bold,
                        modifier = Modifier.clickable {
                            UpdateDownloader.start(context, s.kind, pkg, update.version)
                        },
                    )
                }
            }
        }
    }
}

@Composable
private fun DownloadLink(
    label: String,
    pkg: UpdatePackage,
    busy: Boolean,
    accent: Color,
    dim: Color,
    onClick: () -> Unit,
) {
    Row(Modifier.fillMaxWidth().padding(top = 2.dp)) {
        Text(
            label,
            fontSize = 12.sp,
            color = if (busy) dim else accent,
            fontWeight = FontWeight.Bold,
            modifier = if (busy) Modifier else Modifier.clickable(onClick = onClick),
        )
        Text("  ${formatDownloadSize(pkg.size)}", fontSize = 11.sp, color = dim)
    }
}
