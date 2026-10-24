// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 触摸接线（2026-10-23 从 `KeyboardLayout.swift` 机械搬出；那边压 500 行门禁）。
//
// 本文件只做三件事：把按钮的 down/up 翻成委托调用；把 ⌫ 的长按接成
// 「0.5s 后按住连续退格」的重复驱动器（`RepeatBackspaceDriver`）；以及
// 长按/上滑浮层（`KeyPopup`）的手势与提交。键位语义、层、引擎一概不碰。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译；长按阈值/重复节奏从未上机验证。

import UIKit

extension KeyboardLayout {

    /// 接线：每个键的 down/up，两类长按（⇧ Lock / 模式键菜单；⌫ 连续退格），
    /// 以及带浮层的键的长按/上滑手势。
    func attach(_ b: KeyButton) {
        b.addTarget(self, action: #selector(onTouchDown(_:)), for: .touchDown)
        if b.spec.defersPress {
            // 可滚动列：拖动滚动时 UIScrollView 取消内容触摸 —— 取消/滑出都不算点击。
            b.addTarget(self, action: #selector(onTouchUp(_:)), for: .touchUpInside)
        } else {
            b.addTarget(
                self, action: #selector(onTouchUp(_:)),
                for: [.touchUpInside, .touchUpOutside, .touchCancel]
            )
        }
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
        if !b.spec.popupItems.isEmpty {
            // 长按出浮层（2026-10-24）。0.35s 与 iOS 原生键盘的字符浮层同档；
            // 上滑由 pan 立即触发（不必等长按计时）。
            let lp = UILongPressGestureRecognizer(
                target: self, action: #selector(onKeyPopupLongPress(_:))
            )
            lp.minimumPressDuration = 0.35
            lp.cancelsTouchesInView = false
            b.addGestureRecognizer(lp)
            let pan = UIPanGestureRecognizer(
                target: self, action: #selector(onKeyPopupPan(_:))
            )
            pan.cancelsTouchesInView = false
            b.addGestureRecognizer(pan)
        }
    }

    @objc func onTouchDown(_ sender: KeyButton) {
        if sender.spec.defersPress {
            // 可滚动列（2026-10-24）：按下先不上报，等抬手；拖动滚动就什么都不发。
            sender.longPressFired = false
            return
        }
        if !sender.spec.popupItems.isEmpty {
            // 有浮层的键（含分词这种功能键）**不能按下即上报**：否则长按浮层选完，
            // 短按的动作已经发出去了。普通点击在 `onTouchUp` 里补发。
            sender.popupUsed = false
            return
        }
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
        if !sender.spec.popupItems.isEmpty {
            // 浮层接管过的这次触摸：一个键事件都不发（浮层提交在 `commitKeyPopup`）。
            // 没被接管的点击：功能键补发一次功能，其余补发按下 + 抬起一对（都推迟到了抬手）。
            if !sender.popupUsed {
                if let fn = sender.spec.function {
                    delegate?.keyboardLayout(self, didTapFunction: fn)
                } else {
                    delegate?.keyboardLayout(self, didPress: sender.spec, longPress: false)
                    delegate?.keyboardLayout(self, didRelease: sender.spec)
                }
            }
            sender.popupUsed = false
            return
        }
        if sender.spec.defersPress {
            // 可滚动列（2026-10-24）：点击在抬手才成立（按下被 `onTouchDown` 推迟）。
            // 有 `longPressFunction` 的（已选符号的自定义键）长按已上报，别再补一次。
            if sender.spec.longPressFunction != nil {
                if !sender.longPressFired, let fn = sender.spec.function {
                    delegate?.keyboardLayout(self, didTapFunction: fn)
                }
                sender.longPressFired = false
            } else if let fn = sender.spec.function {
                delegate?.keyboardLayout(self, didTapFunction: fn)
            } else {
                delegate?.keyboardLayout(self, didPress: sender.spec, longPress: false)
                delegate?.keyboardLayout(self, didRelease: sender.spec)
            }
            return
        }
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

    // MARK: - 长按 / 上滑浮层（2026-10-24）

    /// 显示浮层：贴在来源键上方（顶部不够就翻到下方），宽度按条目数、夹在键区内。
    /// 浮层 `isUserInteractionEnabled = false` —— 选词靠键上的手势算横向位移。
    func openKeyPopup(for b: KeyButton, anchorX: CGFloat) {
        guard !b.spec.popupItems.isEmpty else { return }
        hideKeyPopup()
        let popup = KeyPopup(
            items: b.spec.popupItems,
            defaultIndex: b.spec.popupDefaultIndex,
            palette: palette,
            fontScale: fontScale
        )
        let keyRect = convert(b.bounds, from: b)
        let height: CGFloat = 58
        let width = min(
            bounds.width - 8,
            max(keyRect.width, 44 * CGFloat(popup.itemCount))
        )
        var x = keyRect.midX - width / 2
        x = min(max(4, x), bounds.width - width - 4)
        var y = keyRect.minY - height - 4
        if y < 4 { y = keyRect.maxY + 4 }
        popup.frame = CGRect(x: x, y: y, width: width, height: height)
        popup.source = b
        popup.anchorX = anchorX
        addSubview(popup)
        activePopup = popup
        b.popupUsed = true
    }

    /// 收起浮层（不提交）。
    func hideKeyPopup() {
        activePopup?.removeFromSuperview()
        activePopup = nil
    }

    /// 提交当前选中项：小写字母与 9 键 T9 键**进引擎**（匹配词库）；大写、数字、
    /// 符号**直接原样上屏**（2026-10-25 用户裁决，`KeyPopupItem.direct`）。
    func commitKeyPopup() {
        guard let popup = activePopup else { return }
        let text = popup.selectedValue
        let direct = popup.selectedDirect
        hideKeyPopup()
        if direct {
            insert(text)
            return
        }
        guard let scalar = text.unicodeScalars.first else { return }
        let spec = KeySpec(text, keyval: OpiKey.printable(scalar), fallback: .text(text))
        delegate?.keyboardLayout(self, didPress: spec, longPress: false)
        delegate?.keyboardLayout(self, didRelease: spec)
    }

    /// 手指相对开浮层位置的横向位移 → 选中项（从**默认项**数格子，浮层一开不会跳项）。
    private func updateKeyPopupSelection(for b: KeyButton, fingerX: CGFloat) {
        guard let popup = activePopup, popup.source === b else { return }
        let cell = popup.bounds.width / CGFloat(max(1, popup.itemCount))
        guard cell > 0 else { return }
        let steps = Int(((fingerX - popup.anchorX) / cell).rounded())
        popup.setSelected(popup.defaultIndex + steps)
    }

    @objc func onKeyPopupLongPress(_ g: UILongPressGestureRecognizer) {
        guard let b = g.view as? KeyButton else { return }
        switch g.state {
        case .began:
            openKeyPopup(for: b, anchorX: g.location(in: b).x)
        case .changed:
            updateKeyPopupSelection(for: b, fingerX: g.location(in: b).x)
        case .ended:
            if activePopup?.source === b { commitKeyPopup() }
        case .cancelled:
            if activePopup?.source === b { hideKeyPopup() }
        default:
            break
        }
    }

    /// 上滑打开浮层；打开后横向拖动换选中项。
    /// `.failed` 不收浮层：上滑路径由 pan 开、长按路径由 longPress 收，别互相拆台。
    @objc func onKeyPopupPan(_ g: UIPanGestureRecognizer) {
        guard let b = g.view as? KeyButton else { return }
        switch g.state {
        case .changed:
            let translation = g.translation(in: b)
            let fingerX = g.location(in: b).x
            if activePopup == nil, translation.y < -12 {
                // 上滑开浮层：锚点 = 拖动起点（当前点 − 累计位移）。
                openKeyPopup(for: b, anchorX: fingerX - translation.x)
            }
            updateKeyPopupSelection(for: b, fingerX: fingerX)
        case .ended:
            if activePopup?.source === b { commitKeyPopup() }
        case .cancelled:
            if activePopup?.source === b { hideKeyPopup() }
        default:
            break
        }
    }
}
