// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.clip

/**
 * 文件传输的宿主侧清单构造与前置限额（纯 JVM，无 android.*）。
 *
 * 真源上限 = `crates/opi-clip/src/types.rs`；内核还会在发送/接收两侧各校验一次，
 * 这里提前拒绝只是为了让用户在选完文件后立刻看到原因，而不是等复制完 cache 才失败。
 * 目录条目只给 `path`（内核递归展开，相对名取目录名）。
 */

internal const val MAX_TRANSFER_FILES = 128
internal const val MAX_TRANSFER_FILE_BYTES = 2L * 1024 * 1024 * 1024
internal const val MAX_TRANSFER_TOTAL_BYTES = 8L * 1024 * 1024 * 1024
internal const val MAX_TRANSFER_PATH_BYTES = 256

/** `clipSendFiles` 的条目：`name`/`mime` 只对文件有效，目录只能给 `path`。 */
internal data class TransferSpec(
    val path: String,
    val name: String? = null,
    val mime: String? = null,
)

/** 扫描阶段得到的一项（复制前用于限额判定；目录 size 恒为 0）。 */
internal data class TransferEntry(
    val relativePath: String,
    val size: Long,
    val mime: String,
    val isDir: Boolean,
)

internal enum class TransferPhase { OFFERED, ACTIVE, COMPLETED, FAILED, CANCELED }

/** 传输面板的一行（由 transfer_* 事件驱动）。 */
internal data class ClipTransfer(
    val id: String,
    val instanceId: String,
    val peerName: String,
    val outgoing: Boolean,
    val files: List<TransferFileInfo>,
    val totalBytes: Long,
    val transferredBytes: Long = 0,
    val fileIndex: Int = 0,
    val fileCount: Int = 0,
    val phase: TransferPhase = TransferPhase.ACTIVE,
    val savedDir: String? = null,
    val reason: String = "",
)

/** 文件清单 JSON：`[{"path":...,"name":...,"mime":...}]`，顺序与字段名同 Rust 侧。 */
internal fun transferFilesJson(specs: List<TransferSpec>): String = buildString {
    append('[')
    specs.forEachIndexed { index, spec ->
        if (index > 0) append(',')
        append("{\"path\":").append(clipJsonString(spec.path))
        spec.name?.let { append(",\"name\":").append(clipJsonString(it)) }
        spec.mime?.let { append(",\"mime\":").append(clipJsonString(it)) }
        append('}')
    }
    append(']')
}

/** 超限原因；null = 可发送。与内核 `validate_offer` 的判据同序（数量→路径→单文件→总量）。 */
internal fun transferLimitError(entries: List<TransferEntry>): String? {
    if (entries.isEmpty()) return "没有可发送的文件"
    if (entries.size > MAX_TRANSFER_FILES) {
        return "条目超过 $MAX_TRANSFER_FILES 个（当前 ${entries.size} 个），请减少文件"
    }
    val seen = HashSet<String>(entries.size)
    var total = 0L
    for (entry in entries) {
        if (entry.relativePath.toByteArray(Charsets.UTF_8).size > MAX_TRANSFER_PATH_BYTES) {
            return "路径过长（超过 $MAX_TRANSFER_PATH_BYTES 字节）：${entry.relativePath.take(48)}…"
        }
        if (!seen.add(entry.relativePath)) return "存在重名路径：${entry.relativePath}"
        if (!entry.isDir) {
            if (entry.size > MAX_TRANSFER_FILE_BYTES) {
                return "单个文件超过 2 GiB：${entry.relativePath}"
            }
            total += entry.size
            if (total > MAX_TRANSFER_TOTAL_BYTES) return "总大小超过 8 GiB，请分批发送"
        }
    }
    return null
}

/**
 * SAF 显示名 → 安全的相对名。内核拒绝 `/`、`\`、`:`、`.`/`..` 组件与空串；
 * 这里取路径末段并替换非法字符，空结果用「文件」兜底。
 */
internal fun safeTransferName(raw: String): String {
    val last = raw.substringAfterLast('/').substringAfterLast('\\')
    val cleaned = last.map { c ->
        when {
            c == ':' || c == '\\' || c.isISOControl() -> '_'
            else -> c
        }
    }.joinToString("").trim().trim('.')
    return cleaned.ifEmpty { "文件" }
}

/** 同一批选择里的重名文件：保留扩展名，在主干后加 ` (n)`（内核拒绝重复相对路径）。 */
internal fun uniqueTransferName(name: String, used: MutableSet<String>): String {
    if (used.add(name)) return name
    val dot = name.lastIndexOf('.')
    val stem = if (dot > 0) name.substring(0, dot) else name
    val ext = if (dot > 0) name.substring(dot) else ""
    var index = 2
    while (true) {
        val candidate = "$stem ($index)$ext"
        if (used.add(candidate)) return candidate
        index++
    }
}

/** 扩展名兜底 MIME（ContentResolver.getType 返回 null 时用；常见类型即可）。 */
internal fun mimeForName(name: String): String {
    val ext = name.substringAfterLast('.', "").lowercase()
    return when (ext) {
        "txt", "md", "log" -> "text/plain"
        "json" -> "application/json"
        "html", "htm" -> "text/html"
        "css" -> "text/css"
        "xml" -> "application/xml"
        "pdf" -> "application/pdf"
        "zip" -> "application/zip"
        "png" -> "image/png"
        "jpg", "jpeg" -> "image/jpeg"
        "gif" -> "image/gif"
        "webp" -> "image/webp"
        "mp4" -> "video/mp4"
        "mp3" -> "audio/mpeg"
        "wav" -> "audio/wav"
        "ogg" -> "audio/ogg"
        else -> "application/octet-stream"
    }
}

/** 进度/大小的短文案（不引入 java.text 的本地化依赖，JVM 单测可读）。 */
internal fun formatTransferBytes(bytes: Long): String {
    if (bytes < 1024) return "$bytes B"
    val units = listOf("KiB", "MiB", "GiB", "TiB")
    var value = bytes.toDouble()
    var unit = -1
    while (value >= 1024 && unit < units.lastIndex) {
        value /= 1024
        unit++
    }
    return if (value >= 100) "${value.toInt()} ${units[unit]}"
    else "${(value * 10).toInt() / 10.0} ${units[unit]}"
}
