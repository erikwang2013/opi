// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 两个新面板（2026-10-20）：皮肤 / 键盘模式（关于已搬到 `AboutPanel.swift`）。
// 参照实现：Android `keyboard/SkinPanel.kt`、`ime/ImeToolbar.kt` 的 `KeyboardModePanel`。
// 面板惯用法沿用 `InputModePanel.swift`（UIView 子类 + 回调解包 + 表头返回键），
// 不发明新框架。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译；从未上机运行。

import UIKit

// MARK: - 面板表头

/// 面板头（输入模式/键盘模式/皮肤/关于/符号共用）：左上返回 + 标题 + 提示。
final class KeyboardPanelHeader: UIView {
    var onBack: (() -> Void)?

    private let backButton = UIButton(type: .system)
    private let titleLabel = UILabel()
    private let hintLabel = UILabel()

    init(title: String, hint: String) {
        super.init(frame: .zero)
        backButton.setTitle("‹ 返回", for: .normal)
        backButton.titleLabel?.font = .systemFont(ofSize: 14)
        backButton.addTarget(self, action: #selector(tapBack), for: .touchUpInside)
        titleLabel.text = title
        titleLabel.font = .boldSystemFont(ofSize: 15)
        hintLabel.text = hint
        hintLabel.font = .systemFont(ofSize: 11)
        hintLabel.numberOfLines = 1
        // 320pt 窄屏上「标题 + 长提示」会撞在一起：标题优先保留，提示可缩字/截尾。
        hintLabel.adjustsFontSizeToFitWidth = true
        hintLabel.minimumScaleFactor = 0.7
        hintLabel.lineBreakMode = .byTruncatingTail
        titleLabel.setContentCompressionResistancePriority(.required, for: .horizontal)
        hintLabel.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        for v in [backButton, titleLabel, hintLabel] {
            v.translatesAutoresizingMaskIntoConstraints = false
            addSubview(v)
        }
        let h = heightAnchor.constraint(equalToConstant: 64)
        NSLayoutConstraint.activate([
            // 2026-10-21 用户裁决：返回箭头**单独一行**，标题/提示在下一行；各面板统一。
            // 返回键占满 44pt 命中高（面板内容都可滚，多出的 12pt 从内容区拿）。
            h,
            backButton.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 6),
            backButton.topAnchor.constraint(equalTo: topAnchor),
            backButton.heightAnchor.constraint(equalToConstant: 44),
            titleLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 10),
            titleLabel.topAnchor.constraint(equalTo: backButton.bottomAnchor),
            hintLabel.leadingAnchor.constraint(equalTo: titleLabel.trailingAnchor, constant: 8),
            hintLabel.centerYAnchor.constraint(equalTo: titleLabel.centerYAnchor),
            hintLabel.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -8),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    func applyTheme(_ p: KeyboardPalette) {
        backButton.setTitleColor(p.accent, for: .normal)
        titleLabel.textColor = p.panelFg
        hintLabel.textColor = p.barDimFg
    }

    /// compact：表头高度保持 64 —— 返回键 44 + 标题行 18 恰好占满，压到 54 会把
    /// 标题裁掉。面板的紧凑化落在列数/行高（Emoji/Symbol/Rare 各自处理）。
    func setCompact(_ on: Bool) {
        _ = on
    }

    @objc private func tapBack() { onBack?() }
}

// MARK: - 键盘模式

/// 键盘模式面板。**点选项不关面板**（Android 同：暗光 + 单手常连开）。
final class KeyboardModePanel: UIView {
    var onToggleTraditional: (() -> Void)?
    var onUpdate: ((KeyboardModes) -> Void)?
    var onClose: (() -> Void)?

    private let header = KeyboardPanelHeader(title: "键盘模式", hint: "可连开多个；再点一下关")
    private let scroll = UIScrollView()
    private let chips = UIStackView()
    private var traditional = false
    private var modes = KeyboardModes()
    private var palette: KeyboardPalette = .light

    init() {
        super.init(frame: .zero)
        header.onBack = { [weak self] in self?.onClose?() }
        chips.axis = .vertical
        chips.spacing = 8
        chips.translatesAutoresizingMaskIntoConstraints = false
        scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.delaysContentTouches = false
        scroll.addSubview(chips)
        for v in [header, scroll] {
            v.translatesAutoresizingMaskIntoConstraints = false
            addSubview(v)
        }
        NSLayoutConstraint.activate([
            header.leadingAnchor.constraint(equalTo: leadingAnchor),
            header.trailingAnchor.constraint(equalTo: trailingAnchor),
            header.topAnchor.constraint(equalTo: topAnchor),
            scroll.leadingAnchor.constraint(equalTo: leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: trailingAnchor),
            scroll.topAnchor.constraint(equalTo: header.bottomAnchor),
            scroll.bottomAnchor.constraint(equalTo: bottomAnchor),
            chips.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 10),
            chips.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -10),
            chips.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 2),
            chips.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -6),
            chips.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -20),
        ])
        show(traditional: false, modes: KeyboardModes())
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    func show(traditional: Bool, modes: KeyboardModes) {
        self.traditional = traditional
        self.modes = modes
        rebuildChips()
    }

    func applyTheme(_ p: KeyboardPalette) {
        // 只有色板变了才重建 chip（7 颗按钮 + 说明）。切皮肤时 `applyTheme`
        // 会被调一次，但此时面板通常不可见 —— 原实现无条件重建，白刷一遍。
        let recolor = p.accent != palette.accent || p.keyBg != palette.keyBg
            || p.panelFg != palette.panelFg || p.barDimFg != palette.barDimFg
        palette = p
        backgroundColor = p.panelBg
        header.applyTheme(p)
        if recolor || chips.arrangedSubviews.isEmpty { rebuildChips() }
    }

    private func rebuildChips() {
        chips.arrangedSubviews.forEach { $0.removeFromSuperview() }
        // 繁体：引擎模式（与中/繁键同一条路）。点它**只切简繁**。
        addRow([chip("繁体模式", selected: traditional, enabled: true) { [weak self] in
            self?.onToggleTraditional?()
        }])
        // 单手：三选一。
        addRow(KeyboardModes.OneHand.allCases.map { hand in
            chip(oneHandLabel(hand), selected: modes.oneHand == hand, enabled: true) { [weak self] in
                guard var m = self?.modes else { return }
                m.oneHand = hand
                self?.onUpdate?(m)
            }
        })
        // 悬浮/游戏：iOS 扩展没有改键盘窗口的 API ⇒ 可见的禁用项 + 说明，不伪造支持。
        addRow([
            chip("悬浮模式（不可用）", selected: false, enabled: false) {},
            chip("游戏模式（不可用）", selected: false, enabled: false) {},
        ])
        addRow([
            chip("暗光模式", selected: modes.dark, enabled: true) { [weak self] in
                guard var m = self?.modes else { return }
                m.dark.toggle()
                self?.onUpdate?(m)
            },
            chip("长辈模式", selected: modes.elder, enabled: true) { [weak self] in
                guard var m = self?.modes else { return }
                m.elder.toggle()
                self?.onUpdate?(m)
            },
        ])
        let note = UILabel()
        note.text = "iOS 键盘扩展不能控制键盘窗口，悬浮/游戏不可用。"
        note.font = .systemFont(ofSize: 11)
        note.textColor = palette.barDimFg
        note.numberOfLines = 0
        chips.addArrangedSubview(note)
    }

    private func addRow(_ rowChips: [UIButton]) {
        let row = UIStackView(arrangedSubviews: rowChips)
        row.axis = .horizontal
        row.distribution = .fillEqually
        row.spacing = 6
        row.heightAnchor.constraint(equalToConstant: 36).isActive = true
        chips.addArrangedSubview(row)
    }

    private func chip(
        _ label: String, selected: Bool, enabled: Bool, action: @escaping () -> Void
    ) -> UIButton {
        let b = UIButton(type: .system)
        b.setTitle(selected ? "\(label) ✓" : label, for: .normal)
        b.titleLabel?.font = selected ? .boldSystemFont(ofSize: 13) : .systemFont(ofSize: 13)
        b.titleLabel?.adjustsFontSizeToFitWidth = true
        b.titleLabel?.minimumScaleFactor = 0.75
        b.setTitleColor(selected ? palette.accent : palette.panelFg, for: .normal)
        b.backgroundColor = palette.keyBg
        b.layer.cornerRadius = 8
        b.layer.borderWidth = selected ? 2 : 1
        b.layer.borderColor = (selected ? palette.accent : palette.barDimFg.withAlphaComponent(0.35)).cgColor
        b.contentEdgeInsets = UIEdgeInsets(top: 7, left: 8, bottom: 7, right: 8)
        b.isEnabled = enabled
        b.alpha = enabled ? 1 : 0.45
        b.accessibilityTraits = selected ? [.button, .selected] : .button
        b.accessibilityLabel = label
        if enabled {
            b.addAction(UIAction { _ in action() }, for: .touchUpInside)
        }
        return b
    }

    private func oneHandLabel(_ hand: KeyboardModes.OneHand) -> String {
        switch hand {
        case .off: return "单手：关"
        case .left: return "单手：左"
        case .right: return "单手：右"
        }
    }
}

// 关于面板已搬到 `AboutPanel.swift`（2026-10-21：本文件贴着 500 行门禁，
// 设置面板的表头/性能改动要在此落笔）。类名与行为不变。
