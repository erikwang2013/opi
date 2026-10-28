// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 括号自动配对（macOS/IMK 宿主）。表与行为契约的唯一规范：`shared/brackets/README.md`
// —— Android `ime/BracketPairing.kt`、iOS `BracketPairing.swift`、鸿蒙
// `BracketPairing.ets` 逐项镜像，改表四端一起改。
//
// ⚠️ 本机无 macOS/InputMethodKit，未编译、未上机；编译验证由 apple.yml 的 macos job
// （`scripts/apple/package-macos.sh` 的 `swiftc -typecheck macos/*.swift`）承担。
//
// 与 iOS 的平台差异：
//   * iOS 用 `UITextDocumentProxy.adjustTextPosition` 移光标；`IMKTextInput` **没有**
//     光标移动/删除出口。这里向客户端发标准键绑定 `doCommandBySelector:`（它属于 AppKit
//     的 `NSTextInputClient`，不在 `IMKTextInput` 里）。宿主不应答时靠回读上下文发现，
//     退回普通输入 —— **绝不猜光标位置**。
//   * 上下文用 `length()` + `string(from:actualRange:)` 读（10.7+）；客户端不支持
//     TSMDocumentAccess 时返回 NSNotFound/nil ⇒ 不建跟踪（普通输入照常）。
//   * 单位是 UTF-16 单元（IMK 的 NSRange 就是 UTF-16），不是 Character；快照只做自比对。

import AppKit
import Carbon
import InputMethodKit

/// 一对括号（表里每项都是单个 UTF-16 单元，不会劈开代理对）。
struct BracketPair: Equatable {
    let open: String
    let close: String
}

enum BracketPairs {
    /// 顺序与 `shared/brackets/README.md` 一致；改表四端一起改。
    static let all: [BracketPair] = [
        BracketPair(open: "(", close: ")"),
        BracketPair(open: "[", close: "]"),
        BracketPair(open: "{", close: "}"),
        BracketPair(open: "（", close: "）"),
        BracketPair(open: "【", close: "】"),
        BracketPair(open: "《", close: "》"),
        BracketPair(open: "「", close: "」"),
        BracketPair(open: "『", close: "』"),
        BracketPair(open: "“", close: "”"),
        BracketPair(open: "‘", close: "’"),
    ]

    static func opening(_ text: String) -> BracketPair? {
        all.first { $0.open == text }
    }

    static func closing(_ text: String) -> BracketPair? {
        all.first { $0.close == text }
    }
}

/// `doCommandBySelector:` 属 `NSTextInputClient`，**不在** `IMKTextInput` 里
/// （Xcode 报 "no member 'doCommand'"）。用本地 @objc 协议直接发消息：NSView 与
/// XPC 代理两条路都能到，且不需要辅助功能授权（同 TextVN 的 IMKTextTarget）。
@objc protocol OpiCommandTarget {
    @objc(doCommandBySelector:) func doCommand(by selector: Selector)
}

/// 纯状态机（对齐 iOS/Android）：只跟踪**刚插入**的那一对。
final class BracketPairing {
    static let shared = BracketPairing()

    private struct Pending {
        let pair: BracketPair
        let before: String
        let after: String
    }

    /// 配对符之外两侧各保留的上下文长度（契约第 2 条；与 iOS/Android 同为 8）。
    private let context = 8
    private var pending: Pending?

    private init() {}

    func reset() {
        pending = nil
    }

    /// 返回 true = 已处理（调用方不要再 insertText）。
    func insertTyped(_ text: String, client: IMKTextInput, enabled: Bool) -> Bool {
        guard enabled else {
            pending = nil
            return false
        }
        if let pair = BracketPairs.opening(text) {
            pending = nil
            // 有选区不配对（iOS 同一条）：插入会替换选区，语义交给普通插入。
            // 读不到选区（NSNotFound）也退回普通输入 —— 否则可能静默删掉用户选中的正文。
            guard caret(client) != nil, canCommand(client) else { return false }
            let none = NSRange(location: NSNotFound, length: NSNotFound)
            client.insertText(pair.open + pair.close, replacementRange: none)
            _ = command(#selector(NSResponder.moveLeft(_:)), client: client)
            // 回读确认光标真的到了中间；宿主忽略 moveLeft 时不跟踪（插入的闭符号
            // 留着不删 —— 读不到上下文时删可能删错字符，fail-safe）。
            let before = beforeContext(client, pair.open.utf16.count + context)
            let after = afterContext(client, pair.close.utf16.count + context)
            if let before, let after,
               before.hasSuffix(pair.open), after.hasPrefix(pair.close) {
                pending = Pending(pair: pair, before: before, after: after)
            }
            return true
        }
        if let pair = BracketPairs.closing(text), let p = pending, p.pair == pair,
           stillBetween(p, client) {
            pending = nil
            // 吃掉已存在的闭符号：右移一格，并回读确认真的越过了（同退格分支）。
            // 没越过就把这一键交回正常输入 —— 宿主不响应 moveRight 时吞键比多出
            // 一个闭符号更像「键盘坏了」。
            if command(#selector(NSResponder.moveRight(_:)), client: client),
               let moved = beforeContext(client, p.pair.open.utf16.count + p.pair.close.utf16.count),
               moved.hasSuffix(p.pair.open + p.pair.close) {
                return true
            }
            return false
        }
        pending = nil
        return false
    }

    /// 返回 true = 已删掉一对（调用方不要再退格）。
    func backspace(client: IMKTextInput, enabled: Bool) -> Bool {
        guard enabled, let p = pending else {
            pending = nil
            return false
        }
        pending = nil
        guard stillBetween(p, client) else { return false }
        // 先右移越过闭符号，再退两次（NSTextInputClient 没有 deleteForward 的保证）。
        // ⚠️ 右移没有返回值：回读确认真的越过了，没越过就退回普通退格（最多只删
        // 一个字符，绝不越出配对删到正文）。
        guard command(#selector(NSResponder.moveRight(_:)), client: client),
              let moved = beforeContext(client, p.pair.open.utf16.count + p.pair.close.utf16.count),
              moved.hasSuffix(p.pair.open + p.pair.close) else { return false }
        _ = command(#selector(NSResponder.deleteBackward(_:)), client: client)
        _ = command(#selector(NSResponder.deleteBackward(_:)), client: client)
        return true
    }

    // ---------- 宿主能力 ----------

    private func canCommand(_ client: IMKTextInput) -> Bool {
        (client as AnyObject).responds(to: #selector(OpiCommandTarget.doCommand(by:)))
    }

    @discardableResult
    private func command(_ selector: Selector, client: IMKTextInput) -> Bool {
        let object = client as AnyObject
        guard object.responds(to: #selector(OpiCommandTarget.doCommand(by:))) else { return false }
        unsafeBitCast(object, to: OpiCommandTarget.self).doCommand(by: selector)
        return true
    }

    /// 光标（只认空选区；有选区或读不到 → nil）。
    private func caret(_ client: IMKTextInput) -> Int? {
        let range = client.selectedRange()
        guard range.location != NSNotFound, range.length == 0 else { return nil }
        return range.location
    }

    private func stillBetween(_ p: Pending, _ client: IMKTextInput) -> Bool {
        beforeContext(client, p.pair.open.utf16.count + context) == p.before &&
            afterContext(client, p.pair.close.utf16.count + context) == p.after
    }

    private func beforeContext(_ client: IMKTextInput, _ n: Int) -> String? {
        guard let caret = caret(client) else { return nil }
        let total = client.length()
        guard total != NSNotFound, caret <= total else { return nil }
        let want = min(n, caret)
        guard want > 0 else { return nil }
        return text(client, NSRange(location: caret - want, length: want))
    }

    private func afterContext(_ client: IMKTextInput, _ n: Int) -> String? {
        guard let caret = caret(client) else { return nil }
        let total = client.length()
        guard total != NSNotFound, caret <= total else { return nil }
        let want = min(n, total - caret)
        guard want > 0 else { return nil }
        return text(client, NSRange(location: caret, length: want))
    }

    private func text(_ client: IMKTextInput, _ range: NSRange) -> String? {
        var actual = NSRange(location: 0, length: 0)
        guard let s = client.string(from: range, actualRange: &actual), !s.isEmpty else {
            return nil
        }
        return s
    }
}

extension OpiInputController {

    /// 这个键值算「用户敲出的字符」吗（括号配对的入口条件）：可打印段、不是
    /// 空格、不是数字（拼音/符号模式下数字是**页内选词键**，候选上屏不得触发配对）。
    func isTypingKeyval(_ keyval: UInt32) -> Bool {
        keyval < OpiKeySpace.specialBase && keyval != OpiKeySpace.space
            && !(0x30...0x39).contains(keyval)
    }

    /// 硬件按键版本的同一判定：另加「按下」与「没有 ⌃/⌥/⌘」。
    func isTypingKey(_ keyval: UInt32, _ event: NSEvent) -> Bool {
        guard event.type == .keyDown, isTypingKeyval(keyval) else { return false }
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        return !flags.contains(.control) && !flags.contains(.option) && !flags.contains(.command)
    }

    /// 空缓冲、非英文、非安全输入才配对（契约第 5 条）；设置页可整档关闭。
    /// 缓冲非空 = 有未上屏的拼音/预编辑：这一键该交引擎或普通提交，配对让路。
    /// ⚠️ `IsSecureEventInputEnabled` 是**进程级**标志：任何 App 持有安全输入时全局为真。
    /// 后果只是本功能让路（不配对），不影响正常输入 —— 同 fcitx5-macos 的取舍。
    var bracketPairingEnabled: Bool {
        OpiSettingsStore.bracketPairing && engine.buffer().isEmpty
            && engine.mode() != .english && !IsSecureEventInputEnabled()
    }

    /// 用户敲出的文本 / 符号面板直提。返回 true = 配对已处理（调用方不要再 commit）。
    /// 候选词、空格、回车**不走这里**：它们不是「打了一个括号」（契约第 5 条）。
    @discardableResult
    func bracketInsertTyped(_ text: String, to sender: Any!) -> Bool {
        // 只有表里的括号才读设置/碰客户端；其余输入不归配对管（跟踪交给
        // commit 的重置与「上下文失配」兜底）。
        guard BracketPairs.opening(text) != nil || BracketPairs.closing(text) != nil,
              let client = sender as? IMKTextInput else { return false }
        let enabled = bracketPairingEnabled
        // 配对直接写客户端（不经 commit），先把 preedit 按 commit 的同序清掉。
        if enabled, !lastMarked.isEmpty {
            engine.markUserWordsDirty() // 与 commit 同：preedit = 可能刚选过词
            let none = NSRange(location: NSNotFound, length: NSNotFound)
            client.setMarkedText("", selectionRange: NSRange(location: 0, length: 0),
                                 replacementRange: none)
            lastMarked = ""
        }
        return BracketPairing.shared.insertTyped(text, client: client, enabled: enabled)
    }

    /// 空缓冲退格：光标夹在刚插入的空配对中间时删掉一对。返回 true = 已处理。
    func bracketBackspace(to sender: Any!) -> Bool {
        guard let client = sender as? IMKTextInput else { return false }
        return BracketPairing.shared.backspace(client: client, enabled: bracketPairingEnabled)
    }
}
