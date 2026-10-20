// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 键盘内设置面板（2026-10-21 用户要求：iOS 安装后要有可用的设置界面）。
//
// 参照实现是 Android `settings/SettingsScreen.kt` + `settings/ImePreferences.kt`；
// iOS 没有可共享的宿主设置（扩展与宿主 App 未配置 App Group），所以**优先**把
// 设置做在键盘里，工具栏齿轮进入。分区按 Android 的可用子集：
//   常用（学习 / 模糊音逐组 / 纠错 / 简拼）
//   按键（中文标点 / 双拼布局；键盘模式有独立面板，这里给跳转）
//   外观（皮肤跳转 / 暗光 / 长辈）
//   符号与表情（表情页「自有」贴纸）
//   关于（版本 / 关于面板跳转 / 存储与不可用项说明）
//
// **只放 C ABI 有读写出口的开关**（见 `OpiEngineSwitches.swift`）：
//   · 学习        opi_learner_enabled / opi_set_learner
//   · 模糊音      opi_fuzzy_enabled / opi_set_fuzzy / opi_fuzzy_pair_names /
//                 opi_set_fuzzy_pairs_names
//   · 简拼        opi_jianpin_enabled / opi_set_jianpin
//   · 纠错        opi_correction_enabled / opi_set_correction
//   · 中文标点    opi_chinese_punct / opi_set_chinese_punct
//   · 双拼布局    opi_shuangpin_layout / opi_set_shuangpin_layout
// 悬浮/游戏（iOS 扩展不能控制键盘窗口）、英文专业模式、五笔 98（码表未打包）
// 一律**明文标注不可用**，不做点了没反应的假开关。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译；从未上机运行。

import UIKit

/// 设置面板。改动**立即生效**（Android 设置页同姿态），不设「保存」按钮。
final class SettingsPanel: UIView {

    var onClose: (() -> Void)?
    var onOpenSkin: (() -> Void)?
    var onOpenKeyboardModes: (() -> Void)?
    var onOpenAbout: (() -> Void)?
    /// 引擎开关变了：控制器负责持久化 + 推给引擎。
    var onChange: ((KeyboardSettings) -> Void)?
    /// 暗光/长辈变了：控制器负责持久化 + 换肤。
    var onUpdateModes: ((KeyboardModes) -> Void)?

    private enum Toggle: Hashable {
        case learn, fuzzy, jianpin, correction, chinesePunct, showOwn, dark, elder
    }

    private let header = KeyboardPanelHeader(title: "设置", hint: "改动立即生效")
    private let scroll = UIScrollView()
    private let stack = UIStackView()
    private let wubiSection = WubiSettingsSection()
    private var palette: KeyboardPalette = .light

    private var settings = KeyboardSettings()
    private var modes = KeyboardModes()
    private var fuzzyNames: [String] = []
    private var selectedPairs: Set<String> = []

    private var switches: [Toggle: UISwitch] = [:]
    private var pairButtons: [String: UIButton] = [:]
    private var layoutButtons: [String: UIButton] = [:]
    private var headingLabels: [UILabel] = []
    private var detailLabels: [UILabel] = []
    private var navButtons: [UIButton] = []
    private var contentBuilt = false
    private var builtPairNames: [String] = []

    init() {
        super.init(frame: .zero)
        backgroundColor = .light.panelBg
        header.onBack = { [weak self] in self?.onClose?() }
        // 五笔节自己持一份副本；这里只挑五个五笔字段回写，别的开关不受影响。
        wubiSection.onChange = { [weak self] s in
            guard let self else { return }
            self.settings.wubiStyle = s.wubiStyle
            self.settings.wubiMixedEntry = s.wubiMixedEntry
            self.settings.wubiCandidateLimit = s.wubiCandidateLimit
            self.settings.wubiReturn = s.wubiReturn
            self.settings.wubiAutoCommit = s.wubiAutoCommit
            self.onChange?(self.settings)
        }

        stack.axis = .vertical
        stack.spacing = 6
        stack.translatesAutoresizingMaskIntoConstraints = false
        scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.delaysContentTouches = false
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
            stack.leadingAnchor.constraint(
                equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 12
            ),
            stack.trailingAnchor.constraint(
                equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -12
            ),
            stack.topAnchor.constraint(
                equalTo: scroll.contentLayoutGuide.topAnchor, constant: 2
            ),
            stack.bottomAnchor.constraint(
                equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -10
            ),
            stack.widthAnchor.constraint(
                equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -24
            ),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    // MARK: - 数据入口

    /// 打开面板/状态回推时调用。`fuzzyNames` 变了才重建结构（引擎装载前后
    /// 组名会从空变全 —— 那是唯一下次打开需要重排的情况）。
    func show(settings s: KeyboardSettings, modes m: KeyboardModes, fuzzyNames names: [String]) {
        settings = s
        modes = m
        fuzzyNames = names
        wubiSection.show(settings: s)
        if !contentBuilt || names != builtPairNames {
            rebuildContent()
        } else {
            resolvePairs()
            refreshControls()
        }
    }

    /// 键盘模式面板改了暗光/长辈后同步开关（设置面板可见时）。
    func updateModes(_ m: KeyboardModes) {
        modes = m
        guard contentBuilt else { return }
        switches[.dark]?.isOn = m.dark
        switches[.elder]?.isOn = m.elder
    }

    // MARK: - 结构

    private func rebuildContent() {
        stack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        switches.removeAll()
        pairButtons.removeAll()
        layoutButtons.removeAll()
        headingLabels.removeAll()
        detailLabels.removeAll()
        navButtons.removeAll()
        builtPairNames = fuzzyNames
        resolvePairs()

        addSection("常用")
        addSwitchRow(
            "学习", detail: "根据选词学习用户词频", tag: .learn, isOn: settings.learn
        )
        addSwitchRow(
            "模糊音", detail: "总开关；开 = 全选，关 = 逐组不生效",
            tag: .fuzzy, isOn: settings.fuzzy
        )
        addPairChips()
        addSwitchRow(
            "拼音纠错", detail: "允许常见误拼自动纠正", tag: .correction, isOn: settings.correction
        )
        addSwitchRow(
            "简拼", detail: "只用声母也能出候选", tag: .jianpin, isOn: settings.jianpin
        )

        addSection("按键")
        addSwitchRow(
            "中文标点", detail: "关闭后标点键输出英文标点",
            tag: .chinesePunct, isOn: settings.chinesePunct
        )
        addLayoutChips()
        addNote("双拼布局只在输入模式选「拼音双拼」时生效。")
        addNav("键盘模式（单手 / 暗光 / 长辈）") { [weak self] in
            self?.onOpenKeyboardModes?()
        }

        addSection("五笔（86 码表）")
        stack.addArrangedSubview(wubiSection)

        addSection("外观")
        addNav("皮肤（12 款色板）") { [weak self] in self?.onOpenSkin?() }
        addSwitchRow("暗光模式", detail: nil, tag: .dark, isOn: modes.dark)
        addSwitchRow("长辈模式", detail: "更大字号与高对比配色", tag: .elder, isOn: modes.elder)

        addSection("符号与表情")
        addSwitchRow(
            "表情页显示「自有」贴纸", detail: "Android/桌面同款开关，缺省隐藏",
            tag: .showOwn, isOn: settings.showOwn
        )
        addNote("自有贴纸点按会复制图片到剪贴板；iOS 键盘扩展不能直接往宿主文本框插图片。")

        addSection("关于")
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString")
            as? String ?? "未知"
        addNote("小欧输入法 v\(version) · MIT · © 2026 erik.xyz")
        addNav("关于与链接（主页 / 项目 / 博客）") { [weak self] in self?.onOpenAbout?() }
        addNote(
            "设置只保存在键盘扩展自己的容器里。宿主 App 与键盘扩展没有配置 App Group，"
                + "因此宿主 App 不会读取或修改这些设置 —— 真实情况如此，不假装同步。"
        )
        addNote(
            "不可用：悬浮 / 游戏模式 —— iOS 键盘扩展不能控制键盘窗口；英文专业模式 —— "
                + "C ABI 没有出口；五笔 98 方案 —— 码表未打包进 iOS（只能用 86）。"
        )
        contentBuilt = true
        refreshControls()
        // 内容可能在建好之后才第一次构建（面板打开时才 rebuild）：此刻补刷一次
        // 色板，否则深色皮肤下标题/说明是系统黑字，直接看不见。
        applyTheme(palette)
    }

    private func addSection(_ title: String) {
        let label = UILabel()
        label.text = title
        label.font = .boldSystemFont(ofSize: 12)
        label.numberOfLines = 0
        headingLabels.append(label)
        stack.addArrangedSubview(label)
    }

    private func addSwitchRow(
        _ title: String, detail: String?, tag: Toggle, isOn: Bool
    ) {
        let titleLabel = UILabel()
        titleLabel.text = title
        titleLabel.font = .systemFont(ofSize: 13)
        titleLabel.numberOfLines = 0
        let labels = UIStackView(arrangedSubviews: [titleLabel])
        labels.axis = .vertical
        labels.spacing = 1
        if let detail {
            let d = UILabel()
            d.text = detail
            d.font = .systemFont(ofSize: 10)
            d.numberOfLines = 0
            detailLabels.append(d)
            labels.addArrangedSubview(d)
        }
        let sw = UISwitch()
        sw.isOn = isOn
        sw.onTintColor = palette.accent
        sw.setContentHuggingPriority(.required, for: .horizontal)
        sw.setContentCompressionResistancePriority(.required, for: .horizontal)
        sw.accessibilityLabel = title
        sw.addAction(UIAction { [weak self] _ in
            guard let self, let on = self.switches[tag]?.isOn else { return }
            self.toggle(tag, on: on)
        }, for: .valueChanged)
        switches[tag] = sw
        let row = UIStackView(arrangedSubviews: [labels, sw])
        row.axis = .horizontal
        row.alignment = .center
        row.spacing = 10
        stack.addArrangedSubview(row)
    }

    private func addPairChips() {
        guard !fuzzyNames.isEmpty else {
            addNote("引擎未装载，逐组模糊音暂不可用（总开关仍会保存，装载后生效）。")
            return
        }
        addChipRows(fuzzyNames, perRow: 4) { [weak self] name in
            guard let self else { return UIButton() }
            let b = self.makeChip(Self.pairTitle(name)) { [weak self] in self?.togglePair(name) }
            self.pairButtons[name] = b
            return b
        }
        stylePairChips()
    }

    private func addLayoutChips() {
        addChipRows(SHUANGPIN_LAYOUT_NAMES, perRow: 3) { [weak self] name in
            guard let self else { return UIButton() }
            let b = self.makeChip(name) { [weak self] in self?.selectLayout(name) }
            self.layoutButtons[name] = b
            return b
        }
        styleLayoutChips()
    }

    /// 固定每行 `perRow` 格的 chip 行；末行不足用透明占位补齐，避免 chip 被拉宽。
    private func addChipRows(
        _ titles: [String], perRow: Int, make: @escaping (String) -> UIButton
    ) {
        var start = 0
        while start < titles.count {
            let end = min(start + perRow, titles.count)
            let row = UIStackView()
            row.axis = .horizontal
            row.spacing = 6
            row.distribution = .fillEqually
            row.heightAnchor.constraint(equalToConstant: 34).isActive = true
            for title in titles[start..<end] {
                row.addArrangedSubview(make(title))
            }
            if end - start < perRow {
                for _ in 0..<(perRow - (end - start)) {
                    row.addArrangedSubview(UIView())
                }
            }
            stack.addArrangedSubview(row)
            start = end
        }
    }

    private func makeChip(_ title: String, action: @escaping () -> Void) -> UIButton {
        let b = UIButton(type: .system)
        b.setTitle(title, for: .normal)
        b.titleLabel?.font = .systemFont(ofSize: 12)
        b.titleLabel?.adjustsFontSizeToFitWidth = true
        b.titleLabel?.minimumScaleFactor = 0.7
        b.contentEdgeInsets = UIEdgeInsets(top: 4, left: 4, bottom: 4, right: 4)
        b.backgroundColor = palette.keyBg
        b.layer.cornerRadius = 7
        b.addAction(UIAction { _ in action() }, for: .touchUpInside)
        return b
    }

    private func addNav(_ title: String, action: @escaping () -> Void) {
        let b = UIButton(type: .system)
        b.setTitle("\(title)  ›", for: .normal)
        b.titleLabel?.font = .systemFont(ofSize: 13)
        b.contentHorizontalAlignment = .left
        b.contentEdgeInsets = UIEdgeInsets(top: 6, left: 0, bottom: 6, right: 0)
        b.setTitleColor(palette.accent, for: .normal)
        b.accessibilityLabel = title
        b.addAction(UIAction { _ in action() }, for: .touchUpInside)
        navButtons.append(b)
        stack.addArrangedSubview(b)
    }

    private func addNote(_ text: String) {
        let label = UILabel()
        label.text = text
        label.font = .systemFont(ofSize: 10)
        label.numberOfLines = 0
        detailLabels.append(label)
        stack.addArrangedSubview(label)
    }

    // MARK: - 状态

    private func resolvePairs() {
        let raw = settings.fuzzyPairs.split(separator: ",").map {
            String($0).trimmingCharacters(in: .whitespaces)
        }.filter { !$0.isEmpty }
        if raw.isEmpty {
            // 空串 = 回落布尔（Android 同）：总开关开 ⇒ 全部选中。
            selectedPairs = settings.fuzzy ? Set(fuzzyNames) : []
        } else {
            selectedPairs = Set(raw)
        }
    }

    private func refreshControls() {
        switches[.learn]?.isOn = settings.learn
        switches[.fuzzy]?.isOn = settings.fuzzy
        switches[.jianpin]?.isOn = settings.jianpin
        switches[.correction]?.isOn = settings.correction
        switches[.chinesePunct]?.isOn = settings.chinesePunct
        switches[.showOwn]?.isOn = settings.showOwn
        switches[.dark]?.isOn = modes.dark
        switches[.elder]?.isOn = modes.elder
        stylePairChips()
        styleLayoutChips()
    }

    private func toggle(_ tag: Toggle, on: Bool) {
        switch tag {
        case .learn:
            settings.learn = on
        case .fuzzy:
            settings.fuzzy = on
            // 总开关一按就清掉逐组串（Android 合同：空串 ⇒ 回落布尔）。
            settings.fuzzyPairs = ""
            resolvePairs()
            stylePairChips()
        case .jianpin:
            settings.jianpin = on
        case .correction:
            settings.correction = on
        case .chinesePunct:
            settings.chinesePunct = on
        case .showOwn:
            settings.showOwn = on
        case .dark:
            modes.dark = on
            onUpdateModes?(modes)
            return
        case .elder:
            modes.elder = on
            onUpdateModes?(modes)
            return
        }
        onChange?(settings)
    }

    private func togglePair(_ name: String) {
        if selectedPairs.contains(name) {
            selectedPairs.remove(name)
        } else {
            selectedPairs.insert(name)
        }
        // 按引擎给的规范次序拼串，保证与 `opi_fuzzy_pair_names()` 同序；
        // 选择集为空 ⇒ 空串（回落布尔 false）。
        let ordered = fuzzyNames.filter { selectedPairs.contains($0) }
        settings.fuzzyPairs = ordered.joined(separator: ",")
        settings.fuzzy = !ordered.isEmpty
        switches[.fuzzy]?.isOn = settings.fuzzy
        stylePairChips()
        onChange?(settings)
    }

    private func selectLayout(_ name: String) {
        settings.shuangpinLayout = name
        styleLayoutChips()
        onChange?(settings)
    }

    // MARK: - 换肤

    func applyTheme(_ p: KeyboardPalette) {
        palette = p
        backgroundColor = p.panelBg
        header.applyTheme(p)
        wubiSection.applyTheme(p)
        guard contentBuilt else { return }
        headingLabels.forEach { $0.textColor = p.barDimFg }
        detailLabels.forEach { $0.textColor = p.barDimFg }
        navButtons.forEach { $0.setTitleColor(p.accent, for: .normal) }
        switches.values.forEach { $0.onTintColor = p.accent }
        stylePairChips()
        styleLayoutChips()
    }

    private func stylePairChips() {
        for (name, button) in pairButtons {
            styleChip(button, title: Self.pairTitle(name), selected: selectedPairs.contains(name))
        }
    }

    /// 引擎组名是 `zh/z` 这种契约串；界面按 Android 的 `a ↔ b` 展示，存储仍用原名。
    private static func pairTitle(_ name: String) -> String {
        name.replacingOccurrences(of: "/", with: " ↔ ")
    }

    private func styleLayoutChips() {
        for (name, button) in layoutButtons {
            styleChip(
                button, title: name,
                selected: name == settings.shuangpinLayout
            )
        }
    }

    /// 与 Android `PanelChip` 同款：键底色 + 强调色描边 + 粗体 + 行尾 ✓。
    private func styleChip(_ b: UIButton, title: String, selected: Bool) {
        b.setTitle(selected ? "\(title) ✓" : title, for: .normal)
        b.titleLabel?.font = .systemFont(ofSize: 12, weight: selected ? .bold : .regular)
        b.setTitleColor(selected ? palette.accent : palette.panelFg, for: .normal)
        b.backgroundColor = palette.keyBg
        b.layer.borderWidth = selected ? 2 : 1
        b.layer.borderColor = (selected
            ? palette.accent
            : palette.barDimFg.withAlphaComponent(0.35)).cgColor
        b.accessibilityTraits = selected ? [.button, .selected] : .button
        b.accessibilityLabel = title + (selected ? "，已选中" : "")
    }
}
