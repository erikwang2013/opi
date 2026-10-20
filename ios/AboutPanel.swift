// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 关于面板（2026-10-20，2026-10-21 从 `KeyboardPanels.swift` 搬出）。
// 搬出的理由与 `KeyboardCandidateBar.swift` 相同：主文件顶着 500 行源码门禁，
// 而设置面板还要往那边加表头/性能改动。
//
// 文案**不自编**：简介/版权/表情数据/三个地址逐字来自
// `android/.../settings/AboutSection.kt` 与 `ime/AboutPanel.kt`；版本读 appex 的
// `CFBundleShortVersionString`（`scripts/apple/package-ios.sh` 写入）。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译；从未上机运行。

import UIKit

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
