// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 皮肤面板（2026-10-20 首版；2026-10-31 用户裁决「动态背景就是皮肤、全端同一套」）：
// 同一张列表里 11 款纯色 + 19 套动态插画。纯色沿用 `SkinCard` 的小键盘样张；
// 动态卡直接显示 `shared/scene/art/scene-<id>.png`。选中不关面板（连换着比）。
//
// 2026-10-31 从 `KeyboardPanels.swift` 拆出（那边 423 行，加动态卡会撞 500 行门禁）。

import UIKit

// MARK: - 皮肤

/// 皮肤面板：纯色在上、动态在下，点一张换一张，**不自动关**。
final class SkinPanel: UIView {
    var onSelect: ((SkinOption) -> Void)?
    var onSelectScene: ((SceneTheme) -> Void)?
    var onClose: (() -> Void)?

    private let header = KeyboardPanelHeader(
        title: "皮肤", hint: "11 款纯色 + 19 套动态；选中即换"
    )
    private let scroll = UIScrollView()
    private let grid = UIStackView()
    private var solidCards: [SkinOption: SkinCard] = [:]
    private var sceneCards: [SceneTheme: SceneSkinCard] = [:]
    private var currentSkin: SkinOption = .fresh
    private var currentScene: SceneTheme?
    private var palette: KeyboardPalette = .light
    private var styled = false

    init() {
        super.init(frame: .zero)
        header.onBack = { [weak self] in self?.onClose?() }
        grid.axis = .vertical
        grid.spacing = 4
        grid.translatesAutoresizingMaskIntoConstraints = false
        scroll.translatesAutoresizingMaskIntoConstraints = false
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
            grid.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 6),
            grid.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -6),
            grid.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 4),
            grid.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -4),
            grid.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -12),
        ])
        buildCards()
        applyTheme(.light)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    func show(current: SkinOption, scene: SceneTheme?) {
        let previousSkin = currentSkin
        let previousScene = currentScene
        currentSkin = current
        currentScene = scene
        guard styled else {
            styled = true
            refreshAll()
            return
        }
        if previousSkin != current { updateSolid(previousSkin) }
        updateSolid(current)
        if previousScene != scene, let old = previousScene { updateScene(old) }
        if let scene { updateScene(scene) }
    }

    func applyTheme(_ p: KeyboardPalette) {
        let recolor = p.accent != palette.accent || p.keyBg != palette.keyBg
            || p.panelFg != palette.panelFg || p.barDimFg != palette.barDimFg
        palette = p
        backgroundColor = p.panelBg
        header.applyTheme(p)
        if recolor { refreshAll() }
    }

    func setCompact(_ on: Bool) { header.setCompact(on) }

    private func refreshAll() {
        for skin in solidCards.keys { updateSolid(skin) }
        for theme in sceneCards.keys { updateScene(theme) }
    }

    private func updateSolid(_ skin: SkinOption) {
        solidCards[skin]?.setSelected(
            currentScene == nil && skin == currentSkin,
            accent: palette.accent, dim: palette.barDimFg, foreground: palette.panelFg
        )
    }

    private func updateScene(_ theme: SceneTheme) {
        sceneCards[theme]?.setSelected(
            theme == currentScene, accent: palette.accent, dim: palette.barDimFg,
            foreground: palette.panelFg
        )
    }

    private func addSection(_ title: String) {
        let label = UILabel()
        label.text = title
        label.font = .boldSystemFont(ofSize: 12)
        label.textColor = palette.barDimFg
        label.translatesAutoresizingMaskIntoConstraints = false
        label.heightAnchor.constraint(equalToConstant: 20).isActive = true
        grid.addArrangedSubview(label)
    }

    /// 纯色 11 张（不含旧「不改动」），动态 19 张；窄屏折行由固定列数承担。
    private func buildCards() {
        addSection("纯色")
        let solids = [SkinOption.fresh]
            + SkinOption.allCases.filter { $0 != .fresh && $0 != .follow }
        for start in stride(from: 0, to: solids.count, by: 5) {
            let row = UIStackView()
            row.axis = .horizontal
            row.distribution = .fillEqually
            row.spacing = 4
            for skin in solids[start..<min(start + 5, solids.count)] {
                let card = SkinCard(skin: skin)
                card.addTarget(self, action: #selector(tapSolid(_:)), for: .touchUpInside)
                solidCards[skin] = card
                row.addArrangedSubview(card)
            }
            grid.addArrangedSubview(row)
        }
        addSection("动态")
        for start in stride(from: 0, to: SceneTheme.allCases.count, by: 4) {
            let row = UIStackView()
            row.axis = .horizontal
            row.distribution = .fillEqually
            row.spacing = 4
            for theme in SceneTheme.allCases[start..<min(start + 4, SceneTheme.allCases.count)] {
                let card = SceneSkinCard(theme: theme)
                card.addTarget(self, action: #selector(tapScene(_:)), for: .touchUpInside)
                sceneCards[theme] = card
                row.addArrangedSubview(card)
            }
            grid.addArrangedSubview(row)
        }
    }

    @objc private func tapSolid(_ sender: SkinCard) { onSelect?(sender.skin) }
    @objc private func tapScene(_ sender: SceneSkinCard) { onSelectScene?(sender.theme) }
}

/// 一张纯色皮肤卡：小键盘样张 + 名字（Android `SkinCard` 的 UIKit 版）。
final class SkinCard: UIControl {
    let skin: SkinOption

    private let preview = UIView()
    private let nameLabel = UILabel()

    init(skin: SkinOption) {
        self.skin = skin
        super.init(frame: .zero)
        preview.isUserInteractionEnabled = false
        preview.backgroundColor = skin.palette.barBg
        preview.layer.cornerRadius = 6
        preview.layer.borderWidth = 1
        preview.translatesAutoresizingMaskIntoConstraints = false
        addSubview(preview)

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

/// 一张动态皮肤卡：直接显示该主题的高清插画缩略图。
final class SceneSkinCard: UIControl {
    let theme: SceneTheme

    private let preview = UIImageView()
    private let nameLabel = UILabel()

    init(theme: SceneTheme) {
        self.theme = theme
        super.init(frame: .zero)
        preview.isUserInteractionEnabled = false
        preview.contentMode = .scaleAspectFill
        preview.clipsToBounds = true
        preview.layer.cornerRadius = 6
        preview.layer.borderWidth = 1
        preview.backgroundColor = theme.defaultSkin.palette.barBg
        preview.image = theme.image
        preview.translatesAutoresizingMaskIntoConstraints = false
        addSubview(preview)

        nameLabel.isUserInteractionEnabled = false
        nameLabel.text = theme.label
        nameLabel.font = .systemFont(ofSize: 11)
        nameLabel.textAlignment = .center
        nameLabel.translatesAutoresizingMaskIntoConstraints = false
        addSubview(nameLabel)

        NSLayoutConstraint.activate([
            heightAnchor.constraint(equalToConstant: 82),
            preview.topAnchor.constraint(equalTo: topAnchor, constant: 2),
            preview.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 4),
            preview.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -4),
            preview.heightAnchor.constraint(equalToConstant: 54),
            nameLabel.topAnchor.constraint(equalTo: preview.bottomAnchor, constant: 3),
            nameLabel.leadingAnchor.constraint(equalTo: leadingAnchor),
            nameLabel.trailingAnchor.constraint(equalTo: trailingAnchor),
        ])
        isAccessibilityElement = true
        accessibilityTraits = .button
        accessibilityLabel = "皮肤 \(theme.label)"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    func setSelected(_ on: Bool, accent: UIColor, dim: UIColor, foreground: UIColor) {
        preview.layer.borderColor = (on ? accent : dim.withAlphaComponent(0.35)).cgColor
        preview.layer.borderWidth = on ? 2 : 1
        nameLabel.textColor = on ? accent : foreground
        nameLabel.font = on ? .boldSystemFont(ofSize: 11) : .systemFont(ofSize: 11)
        accessibilityTraits = on ? [.button, .selected] : .button
        accessibilityLabel = "皮肤 \(theme.label)\(on ? "，已选中" : "")"
    }
}
