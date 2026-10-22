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
        NSLayoutConstraint.activate([
            // 2026-10-21 用户裁决：返回箭头**单独一行**，标题/提示在下一行；各面板统一。
            // 返回键占满 44pt 命中高（面板内容都可滚，多出的 12pt 从内容区拿）。
            heightAnchor.constraint(equalToConstant: 64),
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

    @objc private func tapBack() { onBack?() }
}

// MARK: - 皮肤

/// 皮肤面板：12 张卡，点一张换一张，**不自动关**（Android 同：皮肤要连换着比）。
final class SkinPanel: UIView {
    var onSelect: ((SkinOption) -> Void)?
    var onClose: (() -> Void)?

    private let header = KeyboardPanelHeader(
        title: "皮肤", hint: "选中即换；「不改动」= 跟随暗光/长辈这些模式"
    )
    private let scroll = UIScrollView()
    private let grid = UIStackView()
    private var cards: [SkinOption: SkinCard] = [:]
    private var currentSkin: SkinOption = .follow
    private var palette: KeyboardPalette = .light
    /// 卡片样式是否已按当前色板刷过（`show` 只更新选中态变化的两张卡）。
    private var styled = false

    init() {
        super.init(frame: .zero)
        header.onBack = { [weak self] in self?.onClose?() }
        grid.axis = .vertical
        grid.spacing = 2
        grid.translatesAutoresizingMaskIntoConstraints = false
        scroll.translatesAutoresizingMaskIntoConstraints = false
        // 默认值会把单击压到 150ms 的 pan 判定之后，连点皮肤会显得「特别慢/没响应」
        // （2026-10-21 用户报障）。卡片只点不拖，立刻把 touch 交给卡片。
        scroll.delaysContentTouches = false
        scroll.addSubview(grid)
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
            grid.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 4),
            grid.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -4),
            grid.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor),
            grid.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor),
            grid.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -8),
        ])
        buildCards()
        applyTheme(.light)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    func show(current: SkinOption) {
        let previous = currentSkin
        currentSkin = current
        // 第一次（或换肤后）全刷一次；之后一次点选只动「旧选中 + 新选中」两张卡。
        // 原实现每次点选都重设 12 张卡的字体/边框 —— 用户报障的「特别慢」之一。
        guard styled else {
            styled = true
            refreshAll()
            return
        }
        guard current != previous else { return }
        update(previous)
        update(current)
    }

    func applyTheme(_ p: KeyboardPalette) {
        // 点选皮肤会走一次 applyTheme；只有色板真的变了才重刷 12 张卡
        // （预览块用的是各皮肤自己的色，不随当前色板变）。
        let recolor = p.accent != palette.accent || p.keyBg != palette.keyBg
            || p.panelFg != palette.panelFg || p.barDimFg != palette.barDimFg
        palette = p
        backgroundColor = p.panelBg
        header.applyTheme(p)
        if recolor { refreshAll() }
    }

    private func refreshAll() {
        for skin in SkinOption.allCases { update(skin) }
    }

    private func update(_ skin: SkinOption) {
        cards[skin]?.setSelected(
            skin == currentSkin, accent: palette.accent, dim: palette.barDimFg,
            foreground: palette.panelFg
        )
    }

    /// 5 张一行（12 张 = 3 行），窄屏也不裁项；整个网格可竖向滚动。
    private func buildCards() {
        for rowStart in stride(from: 0, to: SkinOption.allCases.count, by: 5) {
            let row = UIStackView()
            row.axis = .horizontal
            row.distribution = .fillEqually
            row.spacing = 4
            for skin in SkinOption.allCases[rowStart..<min(rowStart + 5, SkinOption.allCases.count)] {
                let card = SkinCard(skin: skin)
                card.addTarget(self, action: #selector(tapCard(_:)), for: .touchUpInside)
                cards[skin] = card
                row.addArrangedSubview(card)
            }
            grid.addArrangedSubview(row)
        }
    }

    @objc private func tapCard(_ sender: SkinCard) {
        onSelect?(sender.skin)
    }
}

/// 一张皮肤卡：小键盘样张 + 名字（Android `SkinCard` 的 UIKit 版）。
final class SkinCard: UIControl {
    let skin: SkinOption

    private let preview = UIView()
    private let nameLabel = UILabel()

    init(skin: SkinOption) {
        self.skin = skin
        super.init(frame: .zero)
        // 子视图不接 touch：否则点样张/名字会被它们吃掉，整张卡看起来「没响应」
        // （2026-10-21 用户报障「皮肤选择特别慢，甚至有的没有响应」）。
        // 不要开 isExclusiveTouch：它只挡住同窗口的其它 touch，反而会拖慢连点
        // （皮肤面板真正该关的是 UIScrollView.delaysContentTouches）。
        preview.isUserInteractionEnabled = false
        preview.backgroundColor = skin.palette.barBg
        preview.layer.cornerRadius = 6
        preview.layer.borderWidth = 1
        preview.translatesAutoresizingMaskIntoConstraints = false
        addSubview(preview)

        // 三行 × 四格的小键盘：第二行第三格是强调键。
        let rows = UIStackView()
        rows.axis = .vertical
        rows.spacing = 2
        rows.translatesAutoresizingMaskIntoConstraints = false
        for r in 0..<3 {
            let line = UIStackView()
            line.axis = .horizontal
            line.distribution = .fillEqually
            line.spacing = 2
            for c in 0..<4 {
                let key = UIView()
                key.backgroundColor = (r == 1 && c == 2)
                    ? skin.palette.keyHighlightBg : skin.palette.keyBg
                key.layer.cornerRadius = 1.5
                line.addArrangedSubview(key)
            }
            rows.addArrangedSubview(line)
        }
        preview.addSubview(rows)

        nameLabel.isUserInteractionEnabled = false
        nameLabel.text = skin.label
        nameLabel.font = .systemFont(ofSize: 11)
        nameLabel.textAlignment = .center
        nameLabel.translatesAutoresizingMaskIntoConstraints = false
        addSubview(nameLabel)

        NSLayoutConstraint.activate([
            heightAnchor.constraint(equalToConstant: 76),
            preview.topAnchor.constraint(equalTo: topAnchor, constant: 2),
            preview.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 4),
            preview.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -4),
            preview.heightAnchor.constraint(equalToConstant: 46),
            rows.leadingAnchor.constraint(equalTo: preview.leadingAnchor, constant: 6),
            rows.trailingAnchor.constraint(equalTo: preview.trailingAnchor, constant: -6),
            rows.topAnchor.constraint(equalTo: preview.topAnchor, constant: 5),
            rows.bottomAnchor.constraint(equalTo: preview.bottomAnchor, constant: -5),
            nameLabel.topAnchor.constraint(equalTo: preview.bottomAnchor, constant: 3),
            nameLabel.leadingAnchor.constraint(equalTo: leadingAnchor),
            nameLabel.trailingAnchor.constraint(equalTo: trailingAnchor),
        ])
        isAccessibilityElement = true
        accessibilityTraits = .button
        accessibilityLabel = "皮肤 \(skin.label)"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    func setSelected(_ on: Bool, accent: UIColor, dim: UIColor, foreground: UIColor) {
        preview.layer.borderColor = (on ? accent : dim.withAlphaComponent(0.35)).cgColor
        preview.layer.borderWidth = on ? 2 : 1
        nameLabel.textColor = on ? accent : foreground
        nameLabel.font = on ? .boldSystemFont(ofSize: 11) : .systemFont(ofSize: 11)
        accessibilityTraits = on ? [.button, .selected] : .button
        accessibilityLabel = "皮肤 \(skin.label)\(on ? "，已选中" : "")"
    }
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
