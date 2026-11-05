// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 语音 ASR 事件的配置/解析纯函数（从 Speech.kt 拆出守 500 行门禁）。
//! 全部不碰麦克风、FFI 或 UI，SpeechTest 直接钉。

package xyz.erik.opi.candidate

import java.nio.file.Path

/** `AsrConfig::from_json` 的口径；路径全部绝对（模型根由桌面自己定）。 */
internal fun asrConfigJson(root: Path, language: SpeechLanguage): String = buildString {
    append("{\"models_dir\":").append(jsonString(root.toAbsolutePath().toString()))
    append(",\"sense_voice_dir\":\"sensevoice\"")
    append(",\"whisper_dir\":\"whisper-small.en\"")
    append(",\"vad_model\":\"vad/silero_vad.onnx\"")
    append(",\"language\":").append(jsonString(language.value))
    // 连续听写：VAD 每 3 秒强制收一段，segment 事件随时上屏；最后 final 不再重复整段。
    append(",\"max_segment_ms\":3000")
    append(",\"num_threads\":2,\"verify_models\":false}")
}

/**
 * 停止时还要补提交什么：失败的段优先；没有失败段且已有成功段 = 什么都不补；
 * 一段都没成功过（例如极短语音只有 final）= 提交 final 汇总。
 */
internal fun speechRetryText(failedText: String, segmentsCommitted: Int, finalText: String): String =
    failedText.ifEmpty { if (segmentsCommitted > 0) "" else finalText }

private val EVENT_TYPE = Regex("\"type\"\\s*:\\s*\"([a-z_]+)\"")
private val RMS_DB = Regex("\"rms_db\"\\s*:\\s*(-?[0-9]+(?:\\.[0-9]+)?)")
internal fun eventType(json: String): String? =
    EVENT_TYPE.find(json)?.groupValues?.getOrNull(1)

/** rms_db → 0..1；-60 dB 以下按 0 画。 */
internal fun eventLevel(json: String): Float? {
    val db = RMS_DB.find(json)?.groupValues?.getOrNull(1)?.toFloatOrNull() ?: return null
    return ((db + 60f) / 60f).coerceIn(0f, 1f)
}

/** final/segment/error 的 `text`（`message` 也认，兼容 error 事件）。 */
internal fun eventText(json: String): String? {
    val obj = parseLine(json) ?: return null
    return (obj["text"] as? JVal.JStr)?.v ?: (obj["message"] as? JVal.JStr)?.v
}
