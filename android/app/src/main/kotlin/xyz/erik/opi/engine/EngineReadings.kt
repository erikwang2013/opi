// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

/**
 * 候选读数是否值得问引擎：只在「中文模式 + 9/14/双拼」下非空 —— 与
 * `crates/engine-core/src/engine_readings.rs` 的 `candidate_readings()` 开头两道守卫
 * 逐条对应。其余方案/英文模式下读到恒空，那次 JNI（每次击键一次）直接省掉。
 * 两侧集合漂移由 `ReadingsSchemesLockstepTest` 比对钉住。
 */
internal fun needsCandidateReadings(mode: EngineMode, scheme: InputScheme): Boolean =
    (mode == EngineMode.PINYIN || mode == EngineMode.TRADITIONAL) && scheme in READINGS_SCHEMES

private val READINGS_SCHEMES = setOf(InputScheme.NINE, InputScheme.FOURTEEN, InputScheme.SHUANGPIN)

/** 平铺/读数排名窗口（引擎侧一屏全量 + 9/14/双拼读数覆盖深度）。 */
internal const val FETCH_LIMIT_FULL = 512

// 状态快照布局（`crates/opi-ffi/src/jni_snapshot.rs` 模块头逐项对应）：
// 定长前缀 12 槽 + 候选段(计数在前) + 读数段(计数在前)。独立 String 槽位，
// 不用分隔符拼接 —— 候选/读音本身任意，拼接必有歧义。
internal const val SNAP_BUFFER = 0
internal const val SNAP_BUFFER_CARRIED = 1
internal const val SNAP_MODE = 2
internal const val SNAP_SCHEME = 3
internal const val SNAP_LEARNER = 4
internal const val SNAP_FUZZY = 5
internal const val SNAP_JIANPIN = 6
internal const val SNAP_FULLWIDTH = 7
internal const val SNAP_WUBI_HINT = 8
internal const val SNAP_DISPLAY = 9
internal const val SNAP_PENDING = 10
internal const val SNAP_CAND_COUNT = 11
internal const val SNAP_CAND_START = 12

/** 一次 `stateSnapshot` 的解析结果（refresh 的全部读侧状态）。 */
internal class EngineSnapshot(
    val buffer: String,
    val bufferCarried: Boolean,
    val mode: EngineMode,
    val scheme: InputScheme,
    val learnerEnabled: Boolean,
    val fuzzyEnabled: Boolean,
    val jianpinEnabled: Boolean,
    val fullwidth: Boolean,
    val wubiHint: String,
    val displayBuffer: String,
    val pendingText: String,
    val candidates: List<String>,
    val candidateReadings: List<String>,
)

/**
 * 严格解析快照；任何越界/计数不符/非「0/1」标志都返回 null，调用方回退逐出口路径。
 * 旧 so 不认这个出口（调用点抛 UnsatisfiedLinkError），这里只负责新 so 的载荷校验。
 */
internal fun parseSnapshot(raw: Array<String>?): EngineSnapshot? {
    if (raw == null || raw.size < SNAP_CAND_START) return null
    val mode = EngineMode.entries.firstOrNull { it.value.toString() == raw[SNAP_MODE] } ?: return null
    val scheme = InputScheme.entries.firstOrNull { it.value.toString() == raw[SNAP_SCHEME] } ?: return null
    val candCount = raw[SNAP_CAND_COUNT].toIntOrNull()
        ?.takeIf { it in 0..FETCH_LIMIT_FULL } ?: return null
    val readingsAt = SNAP_CAND_START + candCount
    if (readingsAt >= raw.size) return null
    val readingsCount = raw[readingsAt].toIntOrNull()
        ?.takeIf { it in 0..FETCH_LIMIT_FULL } ?: return null
    if (readingsAt + 1 + readingsCount != raw.size) return null
    fun flag(index: Int): Boolean? = when (raw[index]) {
        "1" -> true
        "0" -> false
        else -> null
    }

    return EngineSnapshot(
        buffer = raw[SNAP_BUFFER],
        bufferCarried = flag(SNAP_BUFFER_CARRIED) ?: return null,
        mode = mode,
        scheme = scheme,
        learnerEnabled = flag(SNAP_LEARNER) ?: return null,
        fuzzyEnabled = flag(SNAP_FUZZY) ?: return null,
        jianpinEnabled = flag(SNAP_JIANPIN) ?: return null,
        fullwidth = flag(SNAP_FULLWIDTH) ?: return null,
        wubiHint = raw[SNAP_WUBI_HINT],
        displayBuffer = raw[SNAP_DISPLAY],
        pendingText = raw[SNAP_PENDING],
        candidates = raw.asList().subList(SNAP_CAND_START, readingsAt),
        candidateReadings = raw.asList().subList(readingsAt + 1, readingsAt + 1 + readingsCount),
    )
}
