// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 顶部工具条（2026-10-20）：对齐 Android `ime/ImeToolbar.kt` 的**空闲态**。
//
// Android 的顶部 44dp 槽位是「候选栏 | 工具条」二选一：
// 有候选/有缓冲 → 候选栏；中文空闲、英文空闲、任何面板 → 工具条。
// iOS 此前恒画候选栏，于是空闲时既没有皮肤/输入模式入口，也没有收起键盘键。
// 本类只画图标与回调，状态判断在 `KeyboardLayout.setTopBar(candidatesVisible:english:)`。
//
// ⚠️ 与 Android 的已知差别（如实列出，不伪造）：
//   · iOS 扩展不能控制键盘窗口 ⇒ 没有悬浮/游戏快捷入口（面板里有禁用项）；
//   · 小欧宠物条没有搬（它要先有 Android `petMood` 那套引擎状态，本端无对应出口）。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译；从未上机运行。

import UIKit

final class KeyboardToolbar: UIView {

    var onOpenEmoji: (() -> Void)?
    var onOpenSkin: (() -> Void)?
    var onOpenInputMode: (() -> Void)?
    var onOpenKeyboardMode: (() -> Void)?
    var onOpenAbout: (() -> Void)?
    var onCloseKeyboard: (() -> Void)?
    var onSwitchToChinese: (() -> Void)?

    /// 英文空闲时出现的低存在感「中」键（Android `ChineseHiddenButton` 同款）。
    private let chineseButton = UIButton(type: .system)
    private var iconButtons: [UIButton] = []
    /// 当前色板，供 `applyTheme` 反复调用（切皮肤时控件已存在）。
    private var palette: KeyboardPalette = .light

    init() {
        super.init(frame: .zero)
        // 图标按钮：SF Symbols 名字在运行时可能不存在（系统版本差异），
        // 取不到就退回文字 —— 不静默丢按钮。
        let emoji = makeIcon("face.smiling", fallback: "☺", label: "表情") { [weak self] in
            self?.onOpenEmoji?()
        }
        let skin = makeIcon("paintpalette", fallback: "肤", label: "皮肤") { [weak self] in
            self?.onOpenSkin?()
        }
        let input = makeIcon("keyboard", fallback: "模", label: "输入模式") { [weak self] in
            self?.onOpenInputMode?()
        }
        let modes = makeIcon("slider.horizontal.3", fallback: "盘", label: "键盘模式") { [weak self] in
            self?.onOpenKeyboardMode?()
        }
        let about = makeIcon("info.circle", fallback: "i", label: "关于") { [weak self] in
            self?.onOpenAbout?()
        }
        let close = makeIcon(
            "keyboard.chevron.compact.down", fallback: "⌄", label: "收起键盘"
        ) { [weak self] in
            self?.onCloseKeyboard?()
        }
        iconButtons = [emoji, skin, input, modes, about, close]

        chineseButton.setTitle("中", for: .normal)
        chineseButton.titleLabel?.font = .systemFont(ofSize: 13)
        chineseButton.accessibilityLabel = "切换到中文"
        chineseButton.addTarget(self, action: #selector(tapChinese), for: .touchUpInside)
        chineseButton.isHidden = true

        let spacer = UIView()
        spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)

        let stack = UIStackView(arrangedSubviews: [emoji, skin, input, modes, about, spacer, chineseButton, close])
        stack.axis = .horizontal
        stack.alignment = .fill
        stack.spacing = 0
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 4),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -4),
            stack.topAnchor.constraint(equalTo: topAnchor),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor),
            chineseButton.widthAnchor.constraint(equalToConstant: 30),
        ])
        for b in iconButtons {
            b.widthAnchor.constraint(equalToConstant: 40).isActive = true
        }
        applyTheme(palette)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    /// 英文空闲 = 显示隐藏「中」键（英文有候选时这个键在候选栏，不在这里）。
    func setEnglish(_ english: Bool) {
        chineseButton.isHidden = !english
    }

    func applyTheme(_ p: KeyboardPalette) {
        palette = p
        backgroundColor = p.toolbarBg
        tintColor = p.toolbarFg
        for b in iconButtons {
            b.tintColor = p.toolbarFg
            b.setTitleColor(p.toolbarFg, for: .normal)
        }
        chineseButton.setTitleColor(p.toolbarFg.withAlphaComponent(0.35), for: .normal)
        chineseButton.setTitleColor(p.toolbarFg, for: .highlighted)
    }

    private func makeIcon(
        _ symbol: String, fallback: String, label: String, onTap: @escaping () -> Void
    ) -> UIButton {
        let b = UIButton(type: .system)
        if let image = UIImage(systemName: symbol) {
            b.setImage(image, for: .normal)
            b.imageView?.contentMode = .scaleAspectFit
        } else {
            b.setTitle(fallback, for: .normal)
            b.titleLabel?.font = .systemFont(ofSize: 18)
        }
        b.accessibilityLabel = label
        b.addAction(UIAction { _ in onTap() }, for: .touchUpInside)
        return b
    }

    @objc private func tapChinese() {
        onSwitchToChinese?()
    }
}
