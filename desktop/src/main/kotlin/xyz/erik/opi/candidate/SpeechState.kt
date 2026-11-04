// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 语音状态机的纯类型与转移表（从 Speech.kt 拆出守 500 行门禁）。

package xyz.erik.opi.candidate

internal enum class SpeechPhase { IDLE, PREPARING, RECORDING, RECOGNIZING, DONE, ERROR }

internal data class SpeechUi(
    val phase: SpeechPhase = SpeechPhase.IDLE,
    val engine: SpeechEngine = SpeechEngine.SENSEVOICE,
    /** 0..1，来自 Rust 的 `level` 事件（rms_db -60..0）。 */
    val level: Float = 0f,
    /** 识别中的临时文本 / 最终文本。 */
    val text: String = "",
    val error: String = "",
)

/** 状态机的纯转移表（单测钉住；浮层与宿主共用）。 */
internal enum class SpeechTransition { START, READY, STOP, FINAL, FAIL, CANCEL, DISMISS }

internal fun nextSpeechPhase(from: SpeechPhase, t: SpeechTransition): SpeechPhase = when (t) {
    SpeechTransition.START -> SpeechPhase.PREPARING
    // READY/STOP/FINAL 只接受「上一步确实在那一档」的转移：迟到的 final 不能把
    // 已经取消的会话重新点亮。
    SpeechTransition.READY -> if (from == SpeechPhase.PREPARING) SpeechPhase.RECORDING else from
    SpeechTransition.STOP -> if (from == SpeechPhase.RECORDING) SpeechPhase.RECOGNIZING else from
    SpeechTransition.FINAL -> if (from == SpeechPhase.RECOGNIZING) SpeechPhase.DONE else from
    SpeechTransition.FAIL -> SpeechPhase.ERROR
    SpeechTransition.CANCEL, SpeechTransition.DISMISS -> SpeechPhase.IDLE
}
