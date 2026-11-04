// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 桌面传输的三个分类拖拽入口（2026-11-04 用户要求：入口框做大、虚线、分类清楚）。

package xyz.erik.opi.candidate

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.draganddrop.dragAndDropTarget
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.draganddrop.DragAndDropEvent
import androidx.compose.ui.draganddrop.DragAndDropTarget
import androidx.compose.ui.draganddrop.awtTransferable
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.awt.datatransfer.DataFlavor
import java.io.File
import java.nio.file.Files
import java.nio.file.Path

private fun dropMatches(mode: TransferPickerMode, path: Path): Boolean = when (mode) {
    TransferPickerMode.FILES -> !Files.isDirectory(path)
    TransferPickerMode.FOLDER -> Files.isDirectory(path)
    TransferPickerMode.MEDIA -> isMediaTransferPath(path)
}

@OptIn(ExperimentalFoundationApi::class, ExperimentalComposeUiApi::class)
@Composable
internal fun TransferDropZone(
    mode: TransferPickerMode,
    title: String,
    hint: String,
    enabled: Boolean,
    onPick: () -> Unit,
    onDrop: (List<Path>) -> Unit,
    report: (String) -> Unit,
) {
    var active by remember { mutableStateOf(false) }
    val latestDrop = rememberUpdatedState(onDrop)
    val latestReport = rememberUpdatedState(report)
    val target = remember {
        object : DragAndDropTarget {
            override fun onEntered(event: DragAndDropEvent) {
                active = true
            }

            override fun onExited(event: DragAndDropEvent) {
                active = false
            }

            override fun onEnded(event: DragAndDropEvent) {
                active = false
            }

            override fun onDrop(event: DragAndDropEvent): Boolean {
                active = false
                @Suppress("UNCHECKED_CAST")
                val paths = runCatching {
                    (event.awtTransferable.getTransferData(DataFlavor.javaFileListFlavor)
                        as List<File>).map { it.toPath() }
                }.getOrDefault(emptyList())
                if (paths.isEmpty()) return false
                val matching = paths.filter { dropMatches(mode, it) }
                if (matching.isEmpty()) {
                    latestReport.value("拖入内容不符合「$title」分类")
                } else {
                    latestDrop.value(matching)
                }
                return true
            }
        }
    }
    val shape = RoundedCornerShape(12.dp)
    val dropModifier = Modifier.dragAndDropTarget(
        shouldStartDragAndDrop = { event ->
            enabled && event.awtTransferable.isDataFlavorSupported(DataFlavor.javaFileListFlavor)
        },
        target = target,
    )
    Row(
        Modifier.fillMaxWidth().height(96.dp)
            .clip(shape)
            .background(if (active) Teal.copy(alpha = 0.12f) else Color(0xFFFCFCFA))
            .drawBehind {
                drawRoundRect(
                    color = if (active) Teal else Teal.copy(alpha = 0.45f),
                    cornerRadius = CornerRadius(12.dp.toPx()),
                    style = Stroke(
                        width = if (active) 2.dp.toPx() else 1.5.dp.toPx(),
                        pathEffect = PathEffect.dashPathEffect(
                            floatArrayOf(10.dp.toPx(), 6.dp.toPx()),
                        ),
                    ),
                )
            }
            .then(dropModifier)
            .clickable(enabled = enabled, onClick = onPick)
            .padding(horizontal = 18.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, fontSize = 15.sp, fontWeight = FontWeight.SemiBold, color = Ink)
            Spacer(Modifier.height(3.dp))
            Text(hint, fontSize = 11.sp, color = Muted)
            Spacer(Modifier.height(6.dp))
            Text(
                if (active) "松开鼠标，发送这一分类" else "拖到这里，或点击选择",
                fontSize = 11.sp,
                color = if (active) Teal else Muted,
                fontWeight = if (active) FontWeight.SemiBold else FontWeight.Normal,
            )
        }
        Spacer(Modifier.width(10.dp))
        Text(
            when (mode) {
                TransferPickerMode.FILES -> "文件"
                TransferPickerMode.FOLDER -> "文件夹"
                TransferPickerMode.MEDIA -> "照片 / 视频"
            },
            fontSize = 12.sp,
            color = Teal,
            fontWeight = FontWeight.Bold,
        )
    }
}
