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
        set(value) {
            if (field == value) return
            field = value
            onChange?.invoke()
        }

    /**
     * 开关变化时通知**正在运行的 IME** 立即重算候选（2026-10-30 用户报障
     * 「关闭了还会联想，热重启前不生效」）：App 设置页与 IME 同进程，但它只改了
     * gate + 偏好，没有触发服务侧 `refresh`，旧联想候选会一直留在屏幕上。
     * 由 `OpiImeService` 注册/注销（同一开关只服务一个 IME 实例）。
     * ⚠️ 回调里会碰控制器状态，**只在主线程改 [enabled]**（设置页与服务同进程同主线程）。
     */
    @Volatile
    var onChange: (() -> Unit)? = null
}

/** 关闭时把联想候选挡在 UI 之前；判据与 [associationActive] 同一份，不另立一套。 */
internal fun EngineController.gateAssociation() {
    if (AssociationGate.enabled || !associationActive) {
        return
    }
    // 引擎里的联想也要清掉（fcitx5/TSF 同款）：只清 UI 列表的话，隐藏的联想仍能被
    // 数字键/链式选中并上屏（用户报障「联想被直接上屏」）。
    clearEngineOnly()
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
