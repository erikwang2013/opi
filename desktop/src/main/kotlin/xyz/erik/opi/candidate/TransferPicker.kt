// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 桌面传输的自绘文件选择器（2026-11-04 用户报障：系统 JFileChooser 太丑）。
//! 与设置页同一套配色（Ink/Muted/Teal/Hairline）；支持文件/照片视频多选、文件夹选择、
//! 常用位置、路径输入、上一级。所有目录读取都在 IO 线程，绝不在组合线程扫盘。

package xyz.erik.opi.candidate

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.OutlinedTextField
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path as DrawPath
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.DialogWindow
import androidx.compose.ui.window.rememberDialogState
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import xyz.erik.opi.pet.OpiPetWindowIcon
import java.io.File
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths

internal enum class TransferPickerMode { FILES, MEDIA, FOLDER }

private data class PickerEntry(val path: Path, val name: String, val isDir: Boolean, val size: Long)

private val IMAGE_EXT = setOf("jpg", "jpeg", "png", "gif", "webp", "bmp", "heic", "heif", "avif", "tif", "tiff")
private val VIDEO_EXT = setOf("mp4", "mov", "m4v", "avi", "mkv", "webm", "wmv", "flv", "3gp", "mpg", "mpeg")
private val MEDIA_EXT = IMAGE_EXT + VIDEO_EXT

private val PickerBg = Color(0xFFF6F5F1)
private val PickerCard = Color.White

private var lastPickerDir: Path? = null

private fun extOf(name: String): String = name.substringAfterLast('.', "").lowercase()

/** 拖拽分类用：照片/视频按扩展名判定，目录不算媒体。 */
internal fun isMediaTransferPath(path: Path): Boolean =
    !Files.isDirectory(path) && extOf(path.fileName?.toString().orEmpty()) in MEDIA_EXT

private fun startDir(): Path {
    lastPickerDir?.toAbsolutePath()?.normalize()
        ?.takeIf { Files.isDirectory(it) }?.let { return it }
    val home = Paths.get(System.getProperty("user.home") ?: ".").toAbsolutePath().normalize()
    return listOf("Downloads", "Desktop", "Documents", "Pictures", "Videos")
        .map { home.resolve(it) }
        .firstOrNull { Files.isDirectory(it) } ?: home
}

private fun quickPlaces(): List<Pair<String, Path>> {
    val home = Paths.get(System.getProperty("user.home") ?: ".").toAbsolutePath().normalize()
    val places = mutableListOf(
        "主页" to home,
        "下载" to home.resolve("Downloads"),
        "桌面" to home.resolve("Desktop"),
        "文档" to home.resolve("Documents"),
        "图片" to home.resolve("Pictures"),
        "视频" to home.resolve("Videos"),
        "音乐" to home.resolve("Music"),
    )
    if (isWindows()) {
        File.listRoots()?.forEach { root -> places += ("磁盘 ${root.path}" to root.toPath()) }
    }
    return places.filter { Files.isDirectory(it.second) }.distinctBy { it.second }
}

private fun loadEntries(dir: Path, mode: TransferPickerMode): List<PickerEntry> =
    Files.list(dir).use { stream ->
        stream.iterator().asSequence()
            .filter { !it.fileName.toString().startsWith(".") }
            .mapNotNull { p ->
                val isDir = runCatching { Files.isDirectory(p) }.getOrDefault(false)
                val name = p.fileName?.toString() ?: return@mapNotNull null
                if (!isDir && mode == TransferPickerMode.FOLDER) return@mapNotNull null
                if (!isDir && mode == TransferPickerMode.MEDIA && extOf(name) !in MEDIA_EXT) return@mapNotNull null
                val size = if (isDir) 0L else runCatching { Files.size(p) }.getOrDefault(0L)
                PickerEntry(p, name, isDir, size)
            }
            .sortedWith(compareByDescending<PickerEntry> { it.isDir }.thenBy { it.name.lowercase() })
            .toList()
    }

// Path implements Iterable<Path>，`selected + path` 会命中 plus(Iterable) 重载，
// 把路径拆成每个名字组件（tmp、目录、文件名）逐个加入；用 setOf 包一层避免。
internal fun togglePicked(selected: Set<Path>, path: Path): Set<Path> =
    if (path in selected) selected - setOf(path) else selected + setOf(path)

@Composable
internal fun TransferPickerWindow(
    mode: TransferPickerMode,
    initialDir: Path? = null,
    onCancel: () -> Unit,
    onPick: (List<Path>) -> Unit,
) {
    DialogWindow(
        onCloseRequest = onCancel,
        title = when (mode) {
            TransferPickerMode.FILES -> "选择文件"
            TransferPickerMode.MEDIA -> "选择照片 / 视频"
            TransferPickerMode.FOLDER -> "选择文件夹"
        },
        state = rememberDialogState(width = 800.dp, height = 580.dp),
        icon = OpiPetWindowIcon,
        resizable = false,
    ) {
        TransferPickerBody(mode, initialDir, onCancel, onPick)
    }
}

@Composable
private fun TransferPickerBody(
    mode: TransferPickerMode,
    initialDir: Path?,
    onCancel: () -> Unit,
    onPick: (List<Path>) -> Unit,
) {
    var current by remember {
        mutableStateOf(
            initialDir?.toAbsolutePath()?.normalize()?.takeIf { Files.isDirectory(it) } ?: startDir(),
        )
    }
    var entries by remember { mutableStateOf<List<PickerEntry>>(emptyList()) }
    var selected by remember { mutableStateOf<Set<Path>>(emptySet()) }
    var loading by remember { mutableStateOf(true) }
    var error by remember { mutableStateOf<String?>(null) }
    var pathText by remember { mutableStateOf(current.toString()) }

    LaunchedEffect(current, mode) {
        loading = true
        error = null
        val result = withContext(Dispatchers.IO) {
            runCatching { loadEntries(current, mode) }
        }
        entries = result.getOrElse {
            error = it.message ?: it.javaClass.simpleName
            emptyList()
        }
        pathText = current.toString()
        lastPickerDir = current
        loading = false
    }

    fun go(dir: Path) {
        val target = dir.toAbsolutePath().normalize()
        if (Files.isDirectory(target)) {
            selected = emptySet()
            current = target
        } else {
            error = "目录不存在：$target"
        }
    }

    Column(Modifier.fillMaxSize().background(PickerBg).padding(14.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                when (mode) {
                    TransferPickerMode.FILES -> "选择文件"
                    TransferPickerMode.MEDIA -> "选择照片 / 视频"
                    TransferPickerMode.FOLDER -> "选择文件夹"
                },
                fontSize = 18.sp, color = Ink, fontWeight = FontWeight.Bold,
            )
            Spacer(Modifier.width(10.dp))
            Text(
                when (mode) {
                    TransferPickerMode.FILES -> "可多选；单击文件夹进入"
                    TransferPickerMode.MEDIA -> "只显示图片/视频；可多选"
                    TransferPickerMode.FOLDER -> "进入目标文件夹后点右下角选择"
                },
                fontSize = 11.sp, color = Muted,
            )
        }
        Spacer(Modifier.height(10.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            PickerGhost("↑ 上一级", enabled = current.parent != null) {
                current.parent?.let(::go)
            }
            Spacer(Modifier.width(8.dp))
            OutlinedTextField(
                value = pathText,
                onValueChange = { pathText = it },
                singleLine = true,
                label = { Text("路径", fontSize = 11.sp) },
                modifier = Modifier.weight(1f),
            )
            Spacer(Modifier.width(8.dp))
            PickerGhost("前往") {
                runCatching { Paths.get(pathText) }.getOrNull()?.let(::go) ?: run {
                    error = "路径无效"
                }
            }
        }
        Spacer(Modifier.height(10.dp))
        Row(Modifier.fillMaxWidth().weight(1f)) {
            Column(Modifier.width(150.dp).fillMaxHeight().verticalScroll(rememberScrollState())) {
                Text("常用位置", fontSize = 11.sp, color = Muted)
                Spacer(Modifier.height(4.dp))
                for ((label, place) in quickPlaces()) {
                    val active = current == place
                    Text(
                        label,
                        fontSize = 12.sp,
                        color = if (active) Teal else Ink,
                        modifier = Modifier.fillMaxWidth()
                            .clip(RoundedCornerShape(6.dp))
                            .background(if (active) Teal.copy(alpha = 0.10f) else Color.Transparent)
                            .clickable { go(place) }
                            .padding(horizontal = 8.dp, vertical = 7.dp),
                    )
                }
            }
            Spacer(Modifier.width(10.dp))
            Column(
                Modifier.weight(1f).fillMaxHeight()
                    .clip(RoundedCornerShape(10.dp))
                    .border(1.dp, Hairline, RoundedCornerShape(10.dp))
                    .background(PickerCard),
            ) {
                when {
                    loading -> CenterHint("正在读取目录…")
                    error != null -> CenterHint("读取失败：$error")
                    entries.isEmpty() -> CenterHint("这个文件夹是空的")
                    else -> LazyColumn(Modifier.fillMaxSize()) {
                        items(entries, key = { it.path.toString() }) { entry ->
                            PickerRow(
                                entry = entry,
                                selected = entry.path in selected,
                                onClick = {
                                    if (entry.isDir) {
                                        go(entry.path)
                                    } else {
                                        selected = togglePicked(selected, entry.path)
                                    }
                                },
                            )
                        }
                    }
                }
            }
        }
        Spacer(Modifier.height(10.dp))
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                if (mode == TransferPickerMode.FOLDER) {
                    "当前：${current.fileName ?: current}"
                } else {
                    "已选 ${selected.size} 项"
                },
                fontSize = 12.sp,
                color = Muted,
                modifier = Modifier.weight(1f),
            )
            PickerGhost("取消", onClick = onCancel)
            Spacer(Modifier.width(8.dp))
            PickerPrimary(
                label = when (mode) {
                    TransferPickerMode.FOLDER -> "选择此文件夹"
                    TransferPickerMode.FILES, TransferPickerMode.MEDIA -> "发送所选 ${selected.size} 项"
                },
                enabled = if (mode == TransferPickerMode.FOLDER) {
                    Files.isDirectory(current)
                } else {
                    selected.isNotEmpty()
                },
            ) {
                val picked = if (mode == TransferPickerMode.FOLDER) listOf(current) else selected.toList()
                onPick(picked.map { it.toAbsolutePath().normalize() })
            }
        }
    }
}

@Composable
private fun PickerRow(entry: PickerEntry, selected: Boolean, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth()
            .background(if (selected) Teal.copy(alpha = 0.10f) else Color.Transparent)
            .clickable(onClick = onClick)
            .padding(horizontal = 10.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        PickerGlyph(entry)
        Spacer(Modifier.width(10.dp))
        Text(
            entry.name,
            fontSize = 13.sp,
            color = Ink,
            maxLines = 1,
            modifier = Modifier.weight(1f),
        )
        Text(
            if (entry.isDir) "文件夹" else formatTransferBytes(entry.size),
            fontSize = 11.sp,
            color = Muted,
        )
        if (!entry.isDir && selected) {
            Spacer(Modifier.width(8.dp))
            Text("✓", fontSize = 14.sp, color = Teal, fontWeight = FontWeight.Bold)
        }
    }
}

@Composable
private fun PickerGlyph(entry: PickerEntry) {
    val accent = when {
        entry.isDir -> Teal
        extOf(entry.name) in IMAGE_EXT -> Color(0xFFB7791F)
        extOf(entry.name) in VIDEO_EXT -> Color(0xFF8A5A9B)
        else -> Muted
    }
    Canvas(Modifier.size(26.dp)) {
        val w = size.width
        val h = size.height
        if (entry.isDir) {
            drawRoundRect(
                accent.copy(alpha = 0.18f),
                Offset(0f, h * 0.28f),
                Size(w * 0.92f, h * 0.62f),
                CornerRadius(w * 0.10f),
            )
            drawRoundRect(
                accent.copy(alpha = 0.45f),
                Offset(0f, h * 0.16f),
                Size(w * 0.46f, h * 0.24f),
                CornerRadius(w * 0.06f),
            )
        } else if (extOf(entry.name) in VIDEO_EXT) {
            drawRoundRect(
                accent.copy(alpha = 0.18f),
                Offset(w * 0.08f, h * 0.14f),
                Size(w * 0.84f, h * 0.72f),
                CornerRadius(w * 0.10f),
            )
            val play = DrawPath().apply {
                moveTo(w * 0.40f, h * 0.32f)
                lineTo(w * 0.40f, h * 0.68f)
                lineTo(w * 0.68f, h * 0.50f)
                close()
            }
            drawPath(play, accent)
        } else {
            drawRoundRect(
                accent.copy(alpha = 0.15f),
                Offset(w * 0.12f, h * 0.10f),
                Size(w * 0.76f, h * 0.80f),
                CornerRadius(w * 0.08f),
            )
            if (extOf(entry.name) in IMAGE_EXT) {
                drawCircle(accent.copy(alpha = 0.7f), w * 0.08f, Offset(w * 0.34f, h * 0.34f))
                val hill = DrawPath().apply {
                    moveTo(w * 0.20f, h * 0.74f)
                    lineTo(w * 0.42f, h * 0.50f)
                    lineTo(w * 0.58f, h * 0.66f)
                    lineTo(w * 0.70f, h * 0.56f)
                    lineTo(w * 0.80f, h * 0.74f)
                    close()
                }
                drawPath(hill, accent.copy(alpha = 0.55f))
            } else {
                for (i in 0..2) {
                    val y = h * (0.38f + i * 0.16f)
                    drawLine(accent.copy(alpha = 0.55f), Offset(w * 0.26f, y), Offset(w * 0.74f, y), 1.5f)
                }
            }
        }
    }
}

@Composable
private fun CenterHint(text: String) {
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Text(text, fontSize = 12.sp, color = Muted)
    }
}

@Composable
private fun PickerGhost(label: String, enabled: Boolean = true, onClick: () -> Unit) {
    Box(
        Modifier.clip(RoundedCornerShape(7.dp))
            .border(1.dp, if (enabled) Teal.copy(alpha = 0.55f) else Hairline, RoundedCornerShape(7.dp))
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 7.dp),
    ) {
        Text(label, fontSize = 12.sp, color = if (enabled) Teal else Muted)
    }
}

@Composable
private fun PickerPrimary(label: String, enabled: Boolean, onClick: () -> Unit) {
    Box(
        Modifier.clip(RoundedCornerShape(7.dp))
            .background(if (enabled) Teal else Hairline)
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 14.dp, vertical = 8.dp),
    ) {
        Text(label, fontSize = 12.sp, color = if (enabled) Color.White else Muted)
    }
}
