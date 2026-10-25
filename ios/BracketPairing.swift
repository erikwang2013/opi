// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 括号自动配对（iOS 宿主）。表与行为契约的唯一规范：`shared/brackets/README.md`
// —— Android `ime/BracketPairing.kt`、鸿蒙 `BracketPairing.ets` 逐项镜像，改表三端一起改。
//
// ⚠️ 本机没有 swiftc（Linux），本文件**未过编译器**；CI 的 iOS job 会真编。
// ⚠️ 键盘扩展**收不到光标移动通知**（UITextDocumentProxy 没有选区位置 API），
// 所以「还是我们插入的那一对吗」靠插入时两侧各 8 个字符的快照逐字比对；用户
// 把光标移到别处时上下文对不上，不会误删/误跳过。

import UIKit

/// 一对括号（开 + 闭都是单个 UTF-16 单元，`adjustTextPosition(-1)` 不会劈开代理对）。
struct BracketPair: Equatable {
    let open: String
    let close: String
}

enum BracketPairs {
    /// 顺序与 `shared/brackets/README.md` 一致。
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

/// 纯状态机（对齐 Android `BracketPairing`）：只跟踪**刚插入**的那一对。
final class BracketPairing {
    static let shared = BracketPairing()

    private struct Pending {
        let pair: BracketPair
        let before: String
        let after: String
    }

    /// 配对符之外，两侧各保留的上下文长度（契约第 2 条）。
    private let context = 8
    private var pending: Pending?

    func reset() {
        pending = nil
    }

    /// 返回 true = 已处理（调用方不要再 insert）。
    func insertTyped(_ text: String, proxy: UITextDocumentProxy, enabled: Bool) -> Bool {
        guard enabled else {
            pending = nil
            return false
        }
        if let pair = BracketPairs.opening(text) {
            pending = nil
            proxy.unmarkText()
            proxy.insertText(pair.open + pair.close)
            proxy.adjustTextPosition(byCharacterOffset: -1)
            let before = beforeContext(proxy, pair.open.count + context)
            let after = afterContext(proxy, pair.close.count + context)
            if let before = before, let after = after,
               before.hasSuffix(pair.open), after.hasPrefix(pair.close) {
                pending = Pending(pair: pair, before: before, after: after)
            }
            return true
        }
        if let pair = BracketPairs.closing(text), let p = pending,
           p.pair == pair, stillBetween(p, proxy) {
            pending = nil
            proxy.adjustTextPosition(byCharacterOffset: 1)
            return true
        }
        pending = nil
        return false
    }

    /// 返回 true = 已删掉一对（调用方不要再 deleteBackward）。
    func backspace(proxy: UITextDocumentProxy, enabled: Bool) -> Bool {
        guard enabled, let p = pending else {
            pending = nil
            return false
        }
        pending = nil
        guard stillBetween(p, proxy) else { return false }
        // proxy 没有 deleteForward：先右移越过闭符号，再退两次（Apple 官方建议的替代写法）。
        // ⚠️ adjustTextPosition 没有返回值/失败回调：右移若被宿主忽略，两次退格就会
        // 退到配对之外的正文。用光标前上下文回读确认真的越过了闭符号；没越过就退回
        // 普通退格（最多只删一个字符，绝不误删配对之外的正文）。
        proxy.adjustTextPosition(byCharacterOffset: 1)
        guard let moved = proxy.documentContextBeforeInput,
              moved.hasSuffix(p.pair.open + p.pair.close) else { return false }
        proxy.deleteBackward()
        proxy.deleteBackward()
        return true
    }

    private func stillBetween(_ p: Pending, _ proxy: UITextDocumentProxy) -> Bool {
        beforeContext(proxy, p.pair.open.count + context) == p.before &&
            afterContext(proxy, p.pair.close.count + context) == p.after
    }

    private func beforeContext(_ proxy: UITextDocumentProxy, _ n: Int) -> String? {
        guard let s = proxy.documentContextBeforeInput, !s.isEmpty else { return nil }
        return String(s.suffix(n))
    }

    private func afterContext(_ proxy: UITextDocumentProxy, _ n: Int) -> String? {
        guard let s = proxy.documentContextAfterInput, !s.isEmpty else { return nil }
        return String(s.prefix(n))
    }
}

extension KeyboardViewController {
    /// 英文模式 / 密码安全框不配对（契约第 5 条）。
    var bracketPairingEnabled: Bool {
        engine.mode() != .english && !textDocumentProxy.isSecureTextEntry
    }

    /// 用户**敲出来**的文本：软键盘 route/fallback、外接键盘提交、符号面板直提。
    /// 候选词 / 表情 / 贴纸仍走 `insert` —— 它们不是「打了一个括号」。
    func insertTyped(_ text: String) {
        guard !text.isEmpty else { return }
        if BracketPairing.shared.insertTyped(text, proxy: textDocumentProxy, enabled: bracketPairingEnabled) {
            return
        }
        insert(text)
    }

    /// 空缓冲退格：光标仍夹在我们插入的空配对中间时删一对，否则走系统退格。
    func deleteBackwardTyped() {
        if !BracketPairing.shared.backspace(proxy: textDocumentProxy, enabled: bracketPairingEnabled) {
            textDocumentProxy.deleteBackward()
        }
    }

    /// 新会话（切 App / 重新弹出键盘）不再跟踪上一处的配对。
    /// UITextDocumentProxy 没有会话/光标通知，只能在这个生命周期点清一次。
    func resetBracketPairing() {
        BracketPairing.shared.reset()
    }
}
