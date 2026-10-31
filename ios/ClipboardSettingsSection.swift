// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 设置面板里的「跨设备剪贴板」节（iOS 降级版）。
//
// 只做四件事：开关、对端列表（撤销/配对）、待确认 SAS（信任/拒绝）、状态/限制说明。
// 真正的内核生命周期在 `KeyboardClipboard.swift`（只在键盘可见时运行）；本视图
// 只读快照、调 `OpiClipboard` 的公开方法，不碰 FFI、不碰句柄。
//
// 未配置 App Group 时，配对记录落在扩展私有目录（见 `OpiClipboard.dataDirectory`），
// 卸载/系统清理后需要重新配对 —— 这条在界面上写清楚，不让用户以为是 bug。

import UIKit

final class ClipboardSettingsSection: UIView {
    private let statusLabel = UILabel()
    private let sourceLabel = UILabel()
    private let errorLabel = UILabel()
    private let toggle = UISwitch()
    private let root = UIStackView()
    private let dynamic = UIStackView()
    private var noteLabels: [UILabel] = []
    private var palette: KeyboardPalette = .light
    private var observer: NSObjectProtocol?

    init() {
        super.init(frame: .zero)

        statusLabel.font = .systemFont(ofSize: 13)
        statusLabel.numberOfLines = 0
        sourceLabel.font = .systemFont(ofSize: 10)
        sourceLabel.numberOfLines = 0
        sourceLabel.isHidden = true
        errorLabel.font = .systemFont(ofSize: 10)
        errorLabel.textColor = .systemRed
        errorLabel.numberOfLines = 0
        errorLabel.isHidden = true

        let labels = UIStackView(arrangedSubviews: [statusLabel, sourceLabel, errorLabel])
        labels.axis = .vertical
        labels.spacing = 1

        toggle.accessibilityLabel = "启用跨设备剪贴板"
        toggle.setContentHuggingPriority(.required, for: .horizontal)
        toggle.addAction(UIAction { [weak self] _ in
            guard let self else { return }
            OpiClipboard.shared.setEnabled(self.toggle.isOn)
            self.refresh()
        }, for: .valueChanged)

        let switchRow = UIStackView(arrangedSubviews: [labels, toggle])
        switchRow.axis = .horizontal
        switchRow.alignment = .center
        switchRow.spacing = 10

        dynamic.axis = .vertical
        dynamic.spacing = 4

        let refreshButton = makeAction("刷新设备") { OpiClipboard.shared.refresh() }
        let stopButton = makeAction("停止共享") {
            OpiClipboard.shared.setEnabled(false)
        }
        let actions = UIStackView(arrangedSubviews: [refreshButton, stopButton])
        actions.axis = .horizontal
        actions.distribution = .fillEqually
        actions.spacing = 8

        root.addArrangedSubview(switchRow)
        root.addArrangedSubview(dynamic)
        root.addArrangedSubview(actions)
        root.axis = .vertical
        root.spacing = 6
        root.translatesAutoresizingMaskIntoConstraints = false
        addSubview(root)
        NSLayoutConstraint.activate([
            root.leadingAnchor.constraint(equalTo: leadingAnchor),
            root.trailingAnchor.constraint(equalTo: trailingAnchor),
            root.topAnchor.constraint(equalTo: topAnchor),
            root.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])

        addNote("只在键盘可见时运行，键盘收起后不同步；iOS 扩展没有后台常驻。")
        addNote("收到的内容只显示顶部提示，点「插入」才写入输入框；不会自动写剪贴板。")
        addNote("发送需点工具条的剪贴板图标；读取本机剪贴板可能触发系统粘贴提示。")
        addNote("联网与读取剪贴板依赖系统设置里的「完全访问」；未授权时功能不可用。")
        addNote("未配置 App Group 时配对记录存在扩展私有目录，重装或系统清理后需重新配对。")

        observer = NotificationCenter.default.addObserver(
            forName: .opiClipChanged, object: nil, queue: .main
        ) { [weak self] _ in self?.refresh() }

        refresh()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    func applyTheme(_ p: KeyboardPalette) {
        palette = p
        statusLabel.textColor = p.panelFg
        sourceLabel.textColor = p.barDimFg
        toggle.onTintColor = p.accent
        noteLabels.forEach { $0.textColor = p.barDimFg }
        refresh() // 动态行里的按钮用色板建，换肤后重建一次
    }

    // MARK: - 快照

    private func refresh() {
        let clip = OpiClipboard.shared
        toggle.isOn = clip.isEnabled
        if !clip.isEnabled {
            statusLabel.text = "已关闭（默认）"
        } else if clip.isRunning {
            statusLabel.text = "共享中 · 仅键盘可见时运行"
        } else {
            statusLabel.text = "已开启 · 键盘可见时启动"
        }
        if let source = clip.lastSourceText, !source.isEmpty {
            sourceLabel.text = "最近来源：\(source)"
            sourceLabel.isHidden = false
        } else {
            sourceLabel.isHidden = true
        }
        let error = clip.lastErrorText
        errorLabel.text = error
        errorLabel.isHidden = error.isEmpty

        dynamic.arrangedSubviews.forEach { $0.removeFromSuperview() }
        addPendingRows(clip.pendingPairings)
        addPeerRows(clip.peers)
    }

    private func addPendingRows(_ pending: [OpiClipPendingInfo]) {
        guard !pending.isEmpty else { return }
        dynamic.addArrangedSubview(heading("待确认配对 · 核对两台设备的 6 位数字"))
        for (index, item) in pending.enumerated() {
            let name = item.name.isEmpty ? String(item.id.prefix(8)) : item.name
            let title = UILabel()
            title.text = "\(name) · \(item.sas)"
            title.font = .monospacedDigitSystemFont(ofSize: 15, weight: .semibold)
            title.numberOfLines = 0
            title.setContentHuggingPriority(.defaultLow, for: .horizontal)
            let accept = makeAction("信任") {
                OpiClipboard.shared.respondPairing(at: index, accept: true)
            }
            let reject = makeAction("拒绝") {
                OpiClipboard.shared.respondPairing(at: index, accept: false)
            }
            accept.accessibilityLabel = "信任 \(name)"
            reject.accessibilityLabel = "拒绝 \(name)"
            let row = UIStackView(arrangedSubviews: [title, accept, reject])
            row.axis = .horizontal
            row.alignment = .center
            row.spacing = 6
            dynamic.addArrangedSubview(row)
        }
    }

    private func addPeerRows(_ peers: [OpiClipPeerInfo]) {
        guard !peers.isEmpty else {
            dynamic.addArrangedSubview(heading("暂未发现设备（需同一局域网且已允许本地网络）"))
            return
        }
        dynamic.addArrangedSubview(heading("附近设备"))
        for peer in peers.prefix(8) {
            let name = peer.name.isEmpty ? String(peer.id.prefix(8)) : peer.name
            let title = UILabel()
            title.text = name
            title.font = .systemFont(ofSize: 12)
            title.lineBreakMode = .byTruncatingTail
            let detail = UILabel()
            detail.text = "\(peer.trusted ? "已信任" : "未配对") · "
                + "\(peer.connected ? "在线" : "离线") · \(peer.address)"
            detail.font = .systemFont(ofSize: 9)
            detail.textColor = palette.barDimFg
            detail.lineBreakMode = .byTruncatingMiddle
            let labels = UIStackView(arrangedSubviews: [title, detail])
            labels.axis = .vertical
            labels.spacing = 1
            labels.setContentHuggingPriority(.defaultLow, for: .horizontal)
            let action = makeAction(peer.trusted ? "撤销" : "配对") {
                if peer.trusted {
                    OpiClipboard.shared.forget(peer.id)
                } else {
                    OpiClipboard.shared.pair(peer.id)
                }
            }
            action.accessibilityLabel = peer.trusted ? "撤销对 \(name) 的信任" : "与 \(name) 配对"
            let row = UIStackView(arrangedSubviews: [labels, action])
            row.axis = .horizontal
            row.alignment = .center
            row.spacing = 8
            dynamic.addArrangedSubview(row)
        }
    }

    // MARK: - 控件

    private func heading(_ text: String) -> UILabel {
        let label = UILabel()
        label.text = text
        label.font = .systemFont(ofSize: 10, weight: .semibold)
        label.textColor = palette.barDimFg
        label.numberOfLines = 0
        return label
    }

    private func makeAction(_ title: String, action: @escaping () -> Void) -> UIButton {
        let b = UIButton(type: .system)
        b.setTitle(title, for: .normal)
        b.titleLabel?.font = .systemFont(ofSize: 12, weight: .medium)
        b.setTitleColor(palette.accent, for: .normal)
        b.backgroundColor = palette.keyBg
        b.layer.cornerRadius = 6
        b.contentEdgeInsets = UIEdgeInsets(top: 3, left: 8, bottom: 3, right: 8)
        b.addAction(UIAction { _ in action() }, for: .touchUpInside)
        return b
    }

    private func addNote(_ text: String) {
        let label = UILabel()
        label.text = text
        label.font = .systemFont(ofSize: 10)
        label.textColor = palette.barDimFg
        label.numberOfLines = 0
        noteLabels.append(label)
        root.addArrangedSubview(label)
    }
}
