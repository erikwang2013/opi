// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 五笔细调（2026-10-21）：Android `WubiSection.kt` 的可用子集。
//
// 五个旋钮在 C ABI 都有读写出口（`OpiEngineSwitches.swift`）：风格、混入方式、
// 候选个数、回车行为、四码唯一自动上屏。iOS 只打包 `wubi86.opid`，所以 Android
// 的 86/98 方案选择在这里**不可用**（`package-ios.sh` 不拷 98，也没有对应出口）——
// 明文写出来，不做假选项。
//
// 本文件只画与发变更；持久化 / 推引擎由 `SettingsPanel` → 控制器做。
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译。

import UIKit

final class WubiSettingsSection: UIView {
    var onChange: ((KeyboardSettings) -> Void)?

    private let stack = UIStackView()
    private var palette: KeyboardPalette = .light
    private var settings = KeyboardSettings()
    private var styleButtons: [UIButton] = []
    private var mixedButtons: [UIButton] = []
    private var limitButtons: [UIButton] = []
    private var returnButtons: [UIButton] = []
    private let autoSwitch = UISwitch()
    private var captions: [UILabel] = []
    private var dimLabels: [UILabel] = []

    init() {
        super.init(frame: .zero)
        stack.axis = .vertical
        stack.spacing = 4
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor),
            stack.topAnchor.constraint(equalTo: topAnchor),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])

        addCaption("五笔风格")
        styleButtons = addChips(WUBI_STYLE_NAMES, perRow: 2) { [weak self] i in
            guard let self else { return }
            self.settings.wubiStyle = i
            self.changed()
        }
        addCaption("非五笔字母混入方式")
        mixedButtons = addChips(WUBI_MIXED_ENTRY_NAMES, perRow: 2) { [weak self] i in
            guard let self else { return }
            self.settings.wubiMixedEntry = i
            self.changed()
        }
        addCaption("候选个数")
        let limits = WUBI_CANDIDATE_LIMITS.map { String($0) }
        limitButtons = addChips(limits, perRow: 4) { [weak self] i in
            guard let self, i < WUBI_CANDIDATE_LIMITS.count else { return }
            self.settings.wubiCandidateLimit = WUBI_CANDIDATE_LIMITS[i]
            self.changed()
        }
        addCaption("回车行为（编码未上屏时）")
        returnButtons = addChips(WUBI_RETURN_NAMES, perRow: 2) { [weak self] i in
            guard let self else { return }
            self.settings.wubiReturn = i
            self.changed()
        }
        addAutoCommitRow()
        addNote("iOS 只打包了五笔 86 码表：Android 的 86/98 方案选择在此不可用。")
        applyTheme(.light)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    /// 面板打开 / 状态回推。只刷新选中态，不重建视图。
    func show(settings s: KeyboardSettings) {
        settings = s
        refresh()
    }

    func applyTheme(_ p: KeyboardPalette) {
        palette = p
        captions.forEach { $0.textColor = p.barDimFg }
        dimLabels.forEach { $0.textColor = p.panelFg }
        autoSwitch.onTintColor = p.accent
        refresh()
    }

    // MARK: - 结构与状态

    private func addCaption(_ text: String) {
        let label = UILabel()
        label.text = text
        label.font = .systemFont(ofSize: 11, weight: .semibold)
        label.numberOfLines = 0
        captions.append(label)
        stack.addArrangedSubview(label)
    }

    private func addAutoCommitRow() {
        let label = UILabel()
        label.text = "四码唯一自动上屏"
        label.font = .systemFont(ofSize: 13)
        label.numberOfLines = 0
        dimLabels.append(label)
        autoSwitch.accessibilityLabel = label.text
        autoSwitch.setContentHuggingPriority(.required, for: .horizontal)
        autoSwitch.setContentCompressionResistancePriority(.required, for: .horizontal)
        autoSwitch.addAction(UIAction { [weak self] _ in
            guard let self else { return }
            self.settings.wubiAutoCommit = self.autoSwitch.isOn
            self.changed()
        }, for: .valueChanged)
        let row = UIStackView(arrangedSubviews: [label, autoSwitch])
        row.axis = .horizontal
        row.alignment = .center
        row.spacing = 10
        stack.addArrangedSubview(row)
    }

    private func addNote(_ text: String) {
        let label = UILabel()
        label.text = text
        label.font = .systemFont(ofSize: 10)
        label.numberOfLines = 0
        captions.append(label)
        stack.addArrangedSubview(label)
    }

    private func addChips(
        _ titles: [String], perRow: Int, action: @escaping (Int) -> Void
    ) -> [UIButton] {
        var result: [UIButton] = []
        var start = 0
        while start < titles.count {
            let end = min(start + perRow, titles.count)
            let row = UIStackView()
            row.axis = .horizontal
            row.spacing = 6
            row.distribution = .fillEqually
            row.heightAnchor.constraint(equalToConstant: 32).isActive = true
            for i in start..<end {
                let b = UIButton(type: .system)
                b.titleLabel?.adjustsFontSizeToFitWidth = true
                b.titleLabel?.minimumScaleFactor = 0.7
                b.contentEdgeInsets = UIEdgeInsets(top: 4, left: 4, bottom: 4, right: 4)
                b.accessibilityLabel = titles[i]
                b.addAction(UIAction { _ in action(i) }, for: .touchUpInside)
                result.append(b)
                row.addArrangedSubview(b)
            }
            // 末行不足用透明占位补齐，避免 chip 被拉宽（同 SettingsPanel）。
            for _ in 0..<(perRow - (end - start)) {
                row.addArrangedSubview(UIView())
            }
            stack.addArrangedSubview(row)
            start = end
        }
        return result
    }

    private func changed() {
        refresh()
        onChange?(settings)
    }

    private func refresh() {
        paint(styleButtons, titles: WUBI_STYLE_NAMES, selected: settings.wubiStyle)
        paint(mixedButtons, titles: WUBI_MIXED_ENTRY_NAMES, selected: settings.wubiMixedEntry)
        paint(
            limitButtons, titles: WUBI_CANDIDATE_LIMITS.map { String($0) },
            selected: WUBI_CANDIDATE_LIMITS.firstIndex(of: settings.wubiCandidateLimit) ?? 2
        )
        paint(returnButtons, titles: WUBI_RETURN_NAMES, selected: settings.wubiReturn)
        autoSwitch.isOn = settings.wubiAutoCommit
    }

    /// 与 Android `PanelChip` / 设置面板同款：键底色 + 强调色描边 + 粗体 + ✓。
    private func paint(_ buttons: [UIButton], titles: [String], selected: Int) {
        for (i, b) in buttons.enumerated() {
            let on = i == selected
            b.setTitle(on ? "\(titles[i]) ✓" : titles[i], for: .normal)
            b.titleLabel?.font = .systemFont(ofSize: 12, weight: on ? .bold : .regular)
            b.setTitleColor(on ? palette.accent : palette.panelFg, for: .normal)
            b.backgroundColor = palette.keyBg
            b.layer.cornerRadius = 7
            b.layer.borderWidth = on ? 2 : 1
            b.layer.borderColor = (on
                ? palette.accent
                : palette.barDimFg.withAlphaComponent(0.35)).cgColor
            b.accessibilityTraits = on ? [.button, .selected] : .button
            b.accessibilityLabel = titles[i] + (on ? "，已选中" : "")
        }
    }
}
