// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.asr

/**
 * 语音输入的状态与纯判定。这里不碰 Android/Compose —— JVM 单测直接跑状态机、
 * 权限门与结果校验；Android 侧只负责把事件喂进 [reduce]。
 */
internal enum class AsrPhase {
    IDLE,
    PREPARING,
    LISTENING,
    TRANSCRIBING,
    ERROR,
    MODEL_MISSING,
    PERMISSION_DENIED,
}

internal data class AsrUiState(
    val phase: AsrPhase = AsrPhase.IDLE,
    val message: String? = null,
    /** 最近一次音量（dBFS，负数）；只在 LISTENING 更新。 */
    val levelDb: Float = -60f,
    /** 缺失的是不是 Whisper（true）；浮层文案用。 */
    val whisper: Boolean = false,
)

internal sealed interface AsrEvent {
    data object Start : AsrEvent
    data object Ready : AsrEvent
    data object Stop : AsrEvent
    data class Final(val accepted: Boolean) : AsrEvent
    data class Fail(val message: String) : AsrEvent
    data object Cancel : AsrEvent
    data class Missing(val whisper: Boolean) : AsrEvent
    data object Denied : AsrEvent
    data class Level(val rmsDb: Float) : AsrEvent
}

internal fun reduce(state: AsrUiState, event: AsrEvent): AsrUiState = when (event) {
    AsrEvent.Start -> AsrUiState(AsrPhase.PREPARING)
    AsrEvent.Ready -> AsrUiState(AsrPhase.LISTENING)
    AsrEvent.Stop ->
        if (state.phase == AsrPhase.PREPARING || state.phase == AsrPhase.LISTENING) {
            state.copy(phase = AsrPhase.TRANSCRIBING)
        } else {
            state
        }
    is AsrEvent.Final -> if (event.accepted) AsrUiState() else state
    is AsrEvent.Fail -> AsrUiState(AsrPhase.ERROR, event.message)
    AsrEvent.Cancel -> AsrUiState()
    is AsrEvent.Missing -> AsrUiState(AsrPhase.MODEL_MISSING, whisper = event.whisper)
    AsrEvent.Denied -> AsrUiState(
        AsrPhase.PERMISSION_DENIED,
        "需要麦克风权限才能语音输入",
    )
    is AsrEvent.Level ->
        if (state.phase == AsrPhase.LISTENING) {
            state.copy(levelDb = event.rmsDb)
        } else {
            state
        }
}

internal enum class AsrGate { READY, UNSUPPORTED, NEED_PERMISSION, MODEL_MISSING }

/** 启动前的门：库不可用 > 权限 > 模型。纯函数，单测钉住。 */
internal fun asrGate(
    available: Boolean,
    permissionGranted: Boolean,
    modelReady: Boolean,
): AsrGate = when {
    !available -> AsrGate.UNSUPPORTED
    !permissionGranted -> AsrGate.NEED_PERMISSION
    !modelReady -> AsrGate.MODEL_MISSING
    else -> AsrGate.READY
}

/** 只提交有内容的识别结果；空白/纯空格不是一次有效识别。 */
internal fun asrCommitText(text: String?): String? = text?.trim()?.takeIf { it.isNotEmpty() }

/** 音量条：-60 dBFS..0 dBFS → 0..1。 */
internal fun asrLevelFraction(rmsDb: Float): Float =
    ((rmsDb + 60f) / 60f).coerceIn(0f, 1f)

/**
 * 会话世代。换编辑器/取消/销毁时 [invalidate]，旧会话迟到的 final 事件
 * 因世代不符被丢弃 —— 结果不会写进新的编辑框。
 */
internal class AsrGeneration {
    @Volatile
    private var value = 0

    @Synchronized
    fun begin(): Int = ++value

    @Synchronized
    fun invalidate() {
        value++
    }

    fun isCurrent(generation: Int): Boolean = value == generation
}
