// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 智能联想的 macOS 接线（2026-10-27，Spec §5）：联想态推断、dismiss、文档鼠标点击，
// 以及从 `InputController.swift` 搬来的 `refresh`（那边已贴 500 行上限）。
//
// ⚠️ 本机无 macOS/InputMethodKit，未编译、未上机；编译验证由 apple.yml 的 macos job
// （`scripts/apple/package-macos.sh` 的 `swiftc -typecheck macos/*.swift`）承担。
// `mouseDown` 的形参类型按 Apple 文档抄：`index`/`flags` 是 Int、`keepTracking` 是
// `UnsafeMutablePointer<ObjCBool>!`（不是 Bool）。`override` 的前提是 IMKInputController
// 已完整实现 IMKMouseHandling（Apple 文档原话）—— 若 CI 报「no overridable method」，
// 去掉 `override` 并给类补 `<IMKMouseHandling>` 是等价的另一条路。

import AppKit
import InputMethodKit

extension OpiInputController {

    /// 联想态（Spec §5）：空缓冲 + 空显示串 + 有当前页候选 + 拼音/繁体。
    /// 前缀候选都要求非空缓冲，所以空缓冲下的候选只可能来自联想。
    /// 先判便宜的（缓冲/模式），正常打字时不必取候选；`displayBuffer` 与 `refresh`
    /// 同序 —— 必须在 `candidatesPage()` 之后读（9 键读数缓存绑在候选批上）。
    var associationActive: Bool {
        guard engine.buffer().isEmpty else { return false }
        let mode = engine.mode()
        guard mode == .pinyin || mode == .traditional else { return false }
        guard !engine.candidatesPage().isEmpty else { return false }
        return engine.displayBuffer().isEmpty
    }

    /// 收起联想（引擎 clear 同时清缓冲与联想），并立刻重画。
    func dismissAssociation(_ sender: Any?) {
        engine.clear()
        let fallback: Any? = OpiClientHub.shared.client
        refresh(sender ?? fallback)
    }

    /// 各入口的第一件事：联想态先收起，再执行原本的动作。
    func dismissAssociationIfNeeded(_ sender: Any!) {
        guard associationActive else { return }
        dismissAssociation(sender)
    }

    /// 文档鼠标点击（Spec §5.4）：联想态先收起，再**放行**这次点击（永远返回 false）。
    /// `IMKCandidates` 候选窗自身没有空白区回调，所以候选窗空白点击不做（已知边界）。
    override func mouseDown(onCharacterIndex index: Int, coordinate point: NSPoint,
                            withModifier flags: Int,
                            continueTracking keepTracking: UnsafeMutablePointer<ObjCBool>!,
                            client sender: Any!) -> Bool {
        dismissAssociationIfNeeded(sender)
        return false
    }

    /// 刷新 preedit（marked text）+ 候选窗（2026-10-27 从 `InputController.swift` 搬来）。
    ///
    /// action=1 按 router.rs 的契约就是「请刷新」；它同时覆盖「缓冲/页码变了」与
    /// 「⇧ 状态机等无变化」两种情况（C ABI 只有一个码，不区分）。
    /// 「与上次相同就不动客户端」只是省掉无谓的客户端调用，不是正确性依赖。
    func refresh(_ sender: Any!) {
        guard let client = sender as? IMKTextInput else { return }
        let buffer = engine.buffer()

        if buffer != lastMarked {
            let none = NSRange(location: NSNotFound, length: NSNotFound)
            if buffer.isEmpty {
                client.setMarkedText("",
                                     selectionRange: NSRange(location: 0, length: 0),
                                     replacementRange: none)
            } else {
                client.setMarkedText(buffer,
                                     selectionRange: NSRange(location: buffer.utf16.count, length: 0),
                                     replacementRange: none)
            }
            lastMarked = buffer
        }

        // 显示当前页用引擎的分页出口：页大小/页码/页内选词同源在 router.rs，
        // 客户端不再抄 PAGE_SIZE（`OpiFFI.h` 明写禁止自己按页大小切）。
        let list = engine.candidatesPage()
        // 2026-10-27：可见性 = 有缓冲 **或** 有候选 —— 提交后缓冲为空但联想候选还在，
        // 窗口必须留着显示续接项；可见性变化也要推 update（候选可能同一批）。
        let visible = !buffer.isEmpty || !list.isEmpty
        if list != lastCandidates || visible != lastCandidatesVisible {
            lastCandidates = list
            lastCandidatesVisible = visible
            OpiCandidateWindow.shared.update(list, visible: visible)
        }
        // IMKCandidates 没有标题栏：拼音另画一条；显示串为空回退原文，判定仍读 buffer()。
        OpiPinyinBar.shared.update(display: engine.displayBuffer(), raw: buffer, client: client, candidatesFrame: OpiCandidateWindow.shared.visibleFrame)
    }
}
