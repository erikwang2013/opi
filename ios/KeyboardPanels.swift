// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 三个新面板（2026-10-20）：皮肤 / 键盘模式 / 关于。
// 参照实现：Android `keyboard/SkinPanel.kt`、`ime/ImeToolbar.kt` 的 `KeyboardModePanel`、
// `ime/AboutPanel.kt`。面板惯用法沿用 `InputModePanel.swift`（UIView 子类 + 回调解包
// + 表头返回键），不发明新框架。
//
// 文案**不自编**：关于页的简介/版权/表情数据/三个地址逐字来自
// `android/.../settings/AboutSection.kt` 与 `ime/AboutPanel.kt`；版本读 appex 的
// `CFBundleShortVersionString`（`scripts/apple/package-ios.sh` 写入）。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译；从未上机运行。

import UIKit

// MARK: - 面板表头

/// 面板头（输入模式/键盘模式/皮肤/关于共用）：左上返回 + 标题 + 提示。
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
        for v in [backButton, titleLabel, hintLabel] {
            v.translatesAutoresizingMaskIntoConstraints = false
            addSubview(v)
        }
        NSLayoutConstraint.activate([
            heightAnchor.constraint(equalToConstant: 44),
            backButton.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 4),
            backButton.centerYAnchor.constraint(equalTo: centerYAnchor),
            titleLabel.leadingAnchor.constraint(equalTo: backButton.trailingAnchor, constant: 6),
            titleLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
            hintLabel.leadingAnchor.constraint(equalTo: titleLabel.trailingAnchor, constant: 8),
            hintLabel.centerYAnchor.constraint(equalTo: centerYAnchor),
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

    init() {
        super.init(frame: .zero)
        header.onBack = { [weak self] in self?.onClose?() }
        grid.axis = .vertical
        grid.spacing = 2
        grid.translatesAutoresizingMaskIntoConstraints = false
        scroll.translatesAutoresizingMaskIntoConstraints = false
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
        currentSkin = current
        for (skin, card) in cards {
            card.setSelected(
                skin == current, accent: palette.accent, dim: palette.barDimFg,
                foreground: palette.panelFg
            )
        }
    }

    func applyTheme(_ p: KeyboardPalette) {
        palette = p
        backgroundColor = p.panelBg
        header.applyTheme(p)
        show(current: currentSkin)
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
        palette = p
        backgroundColor = p.panelBg
        header.applyTheme(p)
        rebuildChips()
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
        case .off: return "单手模式：关"
        case .left: return "单手模式：左手"
        case .right: return "单手模式：右手"
        }
    }
}

// MARK: - 关于

/// 关于面板。文案与 Android `AboutPanel.kt` / `settings/AboutSection.kt` **同一份**。
///
/// 链接处理：键盘扩展**可能不被允许**直接打开 URL（`extensionContext.open` 的
/// 行为系统版本相关，且没有宿主 App 的 URL scheme 可兜底）。所以点地址时
/// **先复制到剪贴板**再尝试打开 —— 打不开也不会变成「点了没反应」。
final class AboutPanel: UIView {
    var onClose: (() -> Void)?
    var onOpenURL: ((String) -> Void)?

    private static let intro = "小欧输入法（opi）是一款回归本质、人人可用的多端输入法。"
        + "Open People Input 开放、纯粹、跨端，"
        + "致力于为每一位用户提供不被干扰、不被窥探、不被绑架的输入体验。"
    private static let homeURL = "https://opi.erik.xyz"
    private static let projectURL = "https://github.com/erikwang2013/opi"
    private static let blogURL = "https://erik.xyz"

    private let header = KeyboardPanelHeader(title: "关于", hint: "")
    private let scroll = UIScrollView()
    private let stack = UIStackView()
    private var paragraphLabels: [UILabel] = []
    private var headingLabels: [UILabel] = []
    private var linkButtons: [UIButton] = []
    private var noteLabel: UILabel?

    init() {
        super.init(frame: .zero)
        header.onBack = { [weak self] in self?.onClose?() }
        stack.axis = .vertical
        stack.spacing = 4
        stack.translatesAutoresizingMaskIntoConstraints = false
        scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.addSubview(stack)
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
            stack.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 14),
            stack.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -14),
            stack.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor),
            stack.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -10),
            stack.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -28),
        ])
        buildContent()
        applyTheme(.light)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    func applyTheme(_ p: KeyboardPalette) {
        backgroundColor = p.panelBg
        header.applyTheme(p)
        paragraphLabels.forEach { $0.textColor = p.panelFg }
        headingLabels.forEach { $0.textColor = p.panelFg }
        linkButtons.forEach { $0.setTitleColor(p.accent, for: .normal) }
        noteLabel?.textColor = p.barDimFg
    }

    private func buildContent() {
        addParagraph(Self.intro)
        addHeading("版本")
        // 引擎没有版本出口（C ABI 与 JNI 都没有）—— 只报 App 版本，不编一个。
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString")
            as? String ?? "未知"
        addParagraph("v\(version)（引擎随 App 打包，没有独立的版本出口）")
        addHeading("版权")
        addParagraph("© 2026 erik.xyz · 代码按 MIT 许可（全文见仓库 LICENSE）；词库数据与代码分开授权")
        addHeading("表情数据")
        addParagraph(
            "Emoji 取自 OpenMoji（CC BY-SA 4.0）；颜文字取自 rime_kaomoji_dict（MIT）；"
                + "小欧贴纸/图标为项目专有资产，商业使用需授权"
        )
        addHeading("主页地址")
        addLink(Self.homeURL)
        addHeading("项目地址")
        addLink(Self.projectURL)
        addHeading("博客地址")
        addLink(Self.blogURL)
        let note = UILabel()
        note.text = "点地址会先复制到剪贴板，再尝试用浏览器打开；"
            + "键盘扩展不一定会被允许打开链接，复制后可粘贴到浏览器。"
        note.font = .systemFont(ofSize: 11)
        note.numberOfLines = 0
        noteLabel = note
        stack.addArrangedSubview(note)
    }

    private func addHeading(_ text: String) {
        let label = UILabel()
        label.text = text
        label.font = .boldSystemFont(ofSize: 12)
        label.numberOfLines = 0
        headingLabels.append(label)
        stack.addArrangedSubview(label)
    }

    private func addParagraph(_ text: String) {
        let label = UILabel()
        label.text = text
        label.font = .systemFont(ofSize: 12)
        label.numberOfLines = 0
        paragraphLabels.append(label)
        stack.addArrangedSubview(label)
    }

    private func addLink(_ url: String) {
        let b = UIButton(type: .system)
        b.setTitle(url, for: .normal)
        b.titleLabel?.font = .systemFont(ofSize: 12)
        b.contentHorizontalAlignment = .left
        b.contentEdgeInsets = UIEdgeInsets(top: 2, left: 0, bottom: 2, right: 0)
        b.accessibilityLabel = "打开 \(url)"
        b.addAction(UIAction { [weak self] _ in self?.onOpenURL?(url) }, for: .touchUpInside)
        linkButtons.append(b)
        stack.addArrangedSubview(b)
    }
}
