// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 触摸接线（2026-10-23 从 `KeyboardLayout.swift` 机械搬出；那边压 500 行门禁）。
//
// 本文件只做两件事：把按钮的 down/up 翻成委托调用，以及把 ⌫ 的长按接成
// 「0.5s 后按住连续退格」的重复驱动器（`RepeatBackspaceDriver`）。
// 键位语义、层、引擎一概不碰。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译；长按阈值/重复节奏从未上机验证。

import UIKit

extension KeyboardLayout {

    /// 接线：每个键的 down/up，以及两类长按（⇧ Lock / 模式键菜单；⌫ 连续退格）。
    func attach(_ b: KeyButton) {
        b.addTarget(self, action: #selector(onTouchDown(_:)), for: .touchDown)
        b.addTarget(
            self, action: #selector(onTouchUp(_:)),
            for: [.touchUpInside, .touchUpOutside, .touchCancel]
        )
        if b.spec.longPressRepeatsBackspace {
            // ⌫：长按 = 按住连续退格（2026-10-23 用户要求，不再一次性清组合）。
            let lp = UILongPressGestureRecognizer(
                target: self, action: #selector(onBackspaceLongPress(_:))
            )
            lp.minimumPressDuration = 0.5   // ⚠️ 0.5s 是拍的，真机上按手感调
            // **不**拦截按钮的触摸：抬手时 onTouchUp 还要发一次 released
            // （按下/抬平成对，引擎的 ⇧ 状态机要求）。长按已置 longPressFired，
            // 抬手不会补一次短按。
            lp.cancelsTouchesInView = false
            b.addGestureRecognizer(lp)
        } else if b.spec.keyval == OpiKey.shift || b.spec.longPressFunction != nil {
            // ⇧（Lock）/ longPressFunction（模式键）：按下不能立刻上报 ——
            // 否则先干一次短按的事、长按再补一次（⇧ 是「先 tap 再 lock」）。
            let lp = UILongPressGestureRecognizer(target: self, action: #selector(onLongPress(_:)))
            lp.minimumPressDuration = 0.5
            b.addGestureRecognizer(lp)
        }
    }

    @objc func onTouchDown(_ sender: KeyButton) {
        if let fn = sender.spec.function {
            // 带长按替代语义的功能键（模式键）：只起表，等抬手/长按定论。
            if sender.spec.longPressFunction != nil {
                sender.longPressFired = false
                return
            }
            delegate?.keyboardLayout(self, didTapFunction: fn)
            return
        }
        if sender.spec.keyval == OpiKey.shift || sender.spec.longPressRepeatsBackspace {
            sender.longPressFired = false   // 只起表，不上报（抬手/长按再定论）
            return
        }
        delegate?.keyboardLayout(self, didPress: sender.spec, longPress: false)
    }

    /// ⇧ Lock / 模式键菜单的唯一一次长按回调（旧语义，保持）。
    @objc func onLongPress(_ g: UILongPressGestureRecognizer) {
        guard g.state == .began, let b = g.view as? KeyButton else { return }
        b.longPressFired = true
        // 长按的**去向**由键位表给（`KeySpec.longPressFunction`），不在这里按
        // 「这是模式键所以开菜单」硬判 —— 换表即换语义（见 KeySpec 的字段注释）。
        if let fn = b.spec.longPressFunction {
            delegate?.keyboardLayout(self, didTapFunction: fn)
            return
        }
        delegate?.keyboardLayout(self, didPress: b.spec, longPress: true)
    }

    /// ⌫ 长按：`.began` 起表并**立刻退一格**，之后按 interval 重复；
    /// `.ended/.cancelled` 立即停。每拍都是一次普通 `route(.backspace)`
    /// （先退组合、组合空后退宿主正文）。
    @objc func onBackspaceLongPress(_ g: UILongPressGestureRecognizer) {
        guard let b = g.view as? KeyButton else { return }
        switch g.state {
        case .began:
            b.longPressFired = true
            // 弱持有按钮与布局：面板隐藏/视图拆除时闭包不该把整棵树留住。
            backspaceRepeat.onTick = { [weak self, weak b] in
                guard let self, let b else { return }
                self.delegate?.keyboardLayout(self, didPress: b.spec, longPress: true)
            }
            backspaceRepeat.start()
        case .ended, .cancelled, .failed:
            backspaceRepeat.stop()
        default:
            break
        }
    }

    @objc func onTouchUp(_ sender: KeyButton) {
        if let fn = sender.spec.function {
            // 带长按替代语义的功能键：没长按 → 补发一次普通功能键（长按已上报过）。
            if sender.spec.longPressFunction != nil {
                if !sender.longPressFired {
                    delegate?.keyboardLayout(self, didTapFunction: fn)
                }
                sender.longPressFired = false
            }
            return
        }
        if sender.spec.keyval == OpiKey.shift || sender.spec.longPressRepeatsBackspace {
            // ⌫：抬手先停重复，再决定要不要补短按 —— 顺序反了会多删一格。
            if sender.spec.longPressRepeatsBackspace { backspaceRepeat.stop() }
            if !sender.longPressFired {
                delegate?.keyboardLayout(self, didPress: sender.spec, longPress: false)
            }
            sender.longPressFired = false
        }
        delegate?.keyboardLayout(self, didRelease: sender.spec)
    }
}
