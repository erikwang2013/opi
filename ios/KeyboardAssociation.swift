// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 智能联想的 iOS 接线（2026-10-27，Spec §5）：联想态推断、收起、选区守卫、
// 候选栏空白点击。⚠️ 本机无 UIKit，只在 Apple CI 真编译；行为未上机验证。
//
// 联想态不另存状态：空缓冲 + 空显示串 + 有候选 + 中文模式即可推断 ——
// 前缀候选/逐字片段都要求非空缓冲，所以空缓冲下的候选只可能来自联想。

import UIKit

/// 自我插入窗口（Spec §5.2）：提交后系统会回调 `selectionDidChange`，
/// 窗口内不能把刚建好的联想收掉。取 200ms（brief 允许 150–250ms）。
private let selfInsertWindow: TimeInterval = 0.2

extension KeyboardViewController {

    /// 联想态（空缓冲 + 空逐字片段 + 有候选 + 拼音/繁体）。
    var associationActive: Bool {
        guard layout != nil else { return false }
        return engine.buffer().isEmpty && engine.displayBuffer().isEmpty &&
            !layout.candidateTexts.isEmpty &&
            (engine.mode() == .pinyin || engine.mode() == .traditional)
    }

    /// 收起联想（引擎侧 `clear` 同时清缓冲与联想）。
    func dismissAssociation() {
        engine.clear()
        refresh()
    }

    /// 各入口的第一件事：联想态先收起，再执行原本的动作。
    func dismissAssociationIfNeeded() {
        guard associationActive else { return }
        dismissAssociation()
    }

    /// 记一次「这是我们自己插入的文本」—— 提交会同步触发选区回调。
    func noteSelfInsert() {
        lastSelfInsertAt = ProcessInfo.processInfo.systemUptime
    }

    func inSelfInsertWindow() -> Bool {
        ProcessInfo.processInfo.systemUptime - lastSelfInsertAt <= selfInsertWindow
    }

    /// 选区变化（`selectionDidChange`）：自我插入窗口外就放弃联想
    /// （用户挪光标/点别处，Spec §5.2）。
    func handleSelectionChange() {
        if !inSelfInsertWindow() && associationActive {
            dismissAssociation()
        }
    }

    /// 引擎说「这键我不管」时，客户端该做什么（平台语义，见 KeyFallback 注释）。
    /// 2026-10-27 从 `KeyboardViewController.swift` 搬来（那边撞 500 行门禁）。
    func applyFallback(_ f: KeyFallback) {
        switch f {
        case .none:
            break
        case .text(let t):
            insertTyped(t)
        case .deleteBackward:
            deleteBackwardTyped()
        }
    }
}
