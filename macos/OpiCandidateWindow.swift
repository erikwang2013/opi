// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 候选窗单例（2026-10-28 从 `InputController.swift` 搬来 —— 那边接括号配对后
// 要留行数；与 `InputControllerAssociation.swift` 同一先例）。
//
// ⚠️ 草案 · 从未编译 · 未验证（同 `InputController.swift` 头注）。

import AppKit
import InputMethodKit

/// 候选窗单例。main.swift 用启动时的 IMKServer 建一次。
/// ⚠️ 未验证：IMKCandidates 的 init 签名、panelType 常量名、以及**定位行为**
/// （用单例而非控制器自带窗口，窗随光标定位是否仍生效未经核对）——见 README #3。
final class OpiCandidateWindow {
    static let shared = OpiCandidateWindow()

    private var window: IMKCandidates?
    /// 当前是否由本层显示着。用于「已显示就只换数据，不再 show()」——
    /// 每键 show() 会白付一次窗口服务器调用与重新定位（面板位置由 IMK 跟着
    /// 插入点走，不需要我们每键提醒）。hide() 与 update(空/不可见) 都回写 false。
    private var shown = false

    var visibleFrame: NSRect? { nil } // IMKCandidates 无公开 frame；拼音条按光标定位

    private init() {}

    func configure(server: IMKServer) {
        // ✅ 首编确认（CI run 37934619550，2026-10-09）：init 也是**可失败**的（`IMKCandidates?`）。
        // 失败条件文档没写明；但候选窗是**显示层** —— 建不起来时输入法其余部分
        // （预编辑、上屏）照常工作，只是没有候选面板。所以这里**不**退进程（对照
        // main.swift 里 IMKServer 为 nil 时的 exit(1)：那是输入法根本没法工作），
        // 只打日志记账。window 保持 nil，`update`/`hide` 里本来就有 guard。
        guard let w = IMKCandidates(server: server,
                                    panelType: kIMKSingleRowSteppingCandidatePanel,
                                    styleType: kIMKMain) else {
            NSLog("opi: 候选窗建不起来（IMKCandidates init 返回 nil）—— 输入法仍可用，仅无候选面板")
            return
        }
        // 可见性由本层驱动（缓冲空就 hide、有候选就 show），不让面板自己关。
        // ⚠️ 默认是 true：那样「回车选中候选」会由**面板**处理并回调
        // candidateSelected → 若此时按键也进了 handle（Rust 侧回车 = 提交首候选），
        // 同一个候选会被提交两次。见 README「最不确定的 API」#3。
        w.setDismissesAutomatically(false)
        window = w
    }

    func update(_ list: [String], visible: Bool) {
        guard let w = window else { return }
        if list.isEmpty || !visible {
            w.hide()
            shown = false
            return
        }
        w.setCandidateData(list)
        // 首编确认（同轮）：ObjC 的 `updateCandidates` 在 Swift 里被改名为 `update()`
        // —— 编译器原文：「'updateCandidates()' has been renamed to 'update()'」。
        w.update()
        // 已显示时 setCandidateData+update 就够；重复 show() 只是每键多一次
        // 窗口服务器往返。首次显示（或我们自己 hide 过）才定位/显示。
        if !shown {
            w.show(kIMKLocateCandidatesBelowHint)
            shown = true
        }
    }

    func hide() {
        window?.hide()
        shown = false
    }
}
