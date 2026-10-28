// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

/**
 * 智能联想开关（2026-10-28 用户裁决「默认关闭」）。
 *
 * 进程级而不是 [EngineController] 字段：引擎是进程单例（[xyz.erik.opi.jni.OpiEngine]），
 * IME 服务与 App 设置页各持一个控制器；没有 JNI 出口，开关只能留在宿主侧共享。
 */
object AssociationGate {
    @Volatile
    var enabled: Boolean = false
}

/** 关闭时把联想候选挡在 UI 之前；判据与 [associationActive] 同一份，不另立一套。 */
internal fun EngineController.gateAssociation() {
    if (AssociationGate.enabled || !associationActive) {
        return
    }
    candidates = emptyList()
    candidateReadings = emptyList()
    reading = ""
}

/**
 * 联想态（Spec §5.1）：空缓冲 + 空逐字片段 + 有候选 + 中文模式。
 *
 * UI 侧**不另存状态**：真值由引擎推导，[EngineController.refresh] 每次按键后重读候选；
 * 缓冲为空时候选只可能来自联想（前缀候选/逐字片段都要求非空缓冲）。
 */
val EngineController.associationActive: Boolean
    get() = buffer.isEmpty() && pendingText.isEmpty() && candidates.isNotEmpty() &&
        (mode == EngineMode.PINYIN || mode == EngineMode.TRADITIONAL)
