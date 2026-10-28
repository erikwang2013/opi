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
