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
    /** 已成功写到光标的 VAD 段数（连续听写时让用户看见还在推进）。 */
    val segments: Int = 0,
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

/** 一次连续听写会话的可变状态（宿主单线程持有）。 */
internal class Session(
    val engine: SpeechEngine,
    val commit: (String) -> String?,
    val epoch: Int,
) {
    var deadlineMs: Long = 0L

    /** 成功上屏的 VAD 段数；final 只做收尾，不再重提交整段。 */
    var segmentsCommitted: Int = 0

    /** 上屏失败的段文本；停止/关闭浮层时重试，避免丢字。 */
    var failedText: String = ""
    var segmentText: String = ""
}
