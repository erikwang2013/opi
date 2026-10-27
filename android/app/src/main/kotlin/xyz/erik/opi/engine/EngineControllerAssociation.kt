// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

/**
 * 联想态（Spec §5.1）：空缓冲 + 空逐字片段 + 有候选 + 中文模式。
 *
 * UI 侧**不另存状态**：真值由引擎推导，[EngineController.refresh] 每次按键后重读候选；
 * 缓冲为空时候选只可能来自联想（前缀候选/逐字片段都要求非空缓冲）。
 */
val EngineController.associationActive: Boolean
    get() = buffer.isEmpty() && pendingText.isEmpty() && candidates.isNotEmpty() &&
        (mode == EngineMode.PINYIN || mode == EngineMode.TRADITIONAL)
