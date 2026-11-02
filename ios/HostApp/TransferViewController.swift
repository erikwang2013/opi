// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 本机（Linux）没有 Swift/UIKit 编译器，未编译；由 apple.yml 真编译。
//
// 宿主 App 的跨设备传输页（iOS）。发现/收发在内核 + OpiTransferService；
// 本文件只做界面与系统文件选择器。键盘扩展点传输入口用 URL scheme 打开这里。
//
// 能力边界如实显示：只在 App 前台运行；后台会被系统挂起。未配置 App Group 时
// 信任表与键盘扩展不共享，需要在两边分别配对。

import UIKit
import UniformTypeIdentifiers

final class TransferViewController: UIViewController, UIDocumentPickerDelegate {

    private enum PickerKind {
        case files
        case folder
        case media
    }
    private let service = OpiTransferService.shared
    private let scroll = UIScrollView()
    private let stack = UIStackView()
    private var selectedPeerId: String?
    private var lastOfferedId: String?
    override func viewDidLoad() {
        super.viewDidLoad()
        title = "跨设备传输"
        view.backgroundColor = .systemGroupedBackground
        navigationItem.rightBarButtonItem = UIBarButtonItem(
            title: "刷新", style: .plain, target: self, action: #selector(refreshTapped))
        buildScroll()
        service.onChange = { [weak self] in
            guard let self, self.isViewLoaded, self.view.window != nil else { return }
            self.reload()
        }
        service.onIncomingOffer = { [weak self] transfer in
            guard let self, self.isViewLoaded, self.view.window != nil else { return }
            self.presentOffer(transfer)
        }
        service.start()
        reload()
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        service.start()
        reload()
    }
    private func buildScroll() {
        stack.axis = .vertical
        stack.spacing = 12
        stack.translatesAutoresizingMaskIntoConstraints = false
        scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.alwaysBounceVertical = true
        scroll.contentInsetAdjustmentBehavior = .never
        scroll.addSubview(stack)
        view.addSubview(scroll)
        NSLayoutConstraint.activate([
            scroll.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor),
            scroll.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            scroll.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor),
            stack.leadingAnchor.constraint(
                equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 16),
            stack.trailingAnchor.constraint(
                equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -16),
            stack.topAnchor.constraint(
                equalTo: scroll.contentLayoutGuide.topAnchor, constant: 16),
            stack.bottomAnchor.constraint(
                equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -24),
            stack.widthAnchor.constraint(
                equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -32),
        ])
    }

    // MARK: - 重画

    private func reload() {
        stack.arrangedSubviews.forEach {
            stack.removeArrangedSubview($0)
            $0.removeFromSuperview()
        }
        addStatusCard()
        addDeviceCard()
        addSendCard()
        addPendingCard()
        addTransferCard()
        addNotes()
    }

    private func addStatusCard() {
        var views: [UIView] = []
        let state = service.isRunning ? "运行中" : "启动中或已停止"
        views.append(makeLabel("本机：\(service.deviceName) · \(state)", style: .body))
        let error = service.lastErrorText
        if !error.isEmpty {
            let label = makeLabel("错误：\(error)", style: .footnote)
            label.textColor = .systemRed
            views.append(label)
        }
        views.append(makeNote("打开本页时启动发现与加密会话；App 退到后台会被系统挂起，传输可能中断。"))
        addCard(views)
    }

    private func addDeviceCard() {
        var views: [UIView] = [makeLabel("附近设备", style: .headline)]
        let peers = service.peers
        if peers.isEmpty {
            views.append(makeNote(
                "还没有发现设备。确认两台设备在同一局域网、都已配对，并允许本地网络访问。"))
        } else {
            for peer in peers {
                let name = peer.name.isEmpty ? String(peer.id.prefix(8)) : peer.name
                let title = makeLabel(name, style: .body)
                let meta = makeLabel(
                    "\(String(peer.id.prefix(8))) · "
                        + (peer.connected ? "在线" : "离线") + " · "
                        + (peer.trusted ? "已信任" : "未配对"), style: .caption1)
                meta.textColor = .secondaryLabel
                let labels = UIStackView(arrangedSubviews: [title, meta])
                labels.axis = .vertical
                labels.spacing = 2
                labels.setContentHuggingPriority(.defaultLow, for: .horizontal)
                let action = makeSmallButton(peer.trusted ? "撤销" : "配对") { [weak self] in
                    guard let self else { return }
                    if peer.trusted {
                        self.service.forget(peer.id)
                    } else {
                        self.service.pair(peer.id)
                    }
                    self.reload()
                }
                let row = UIStackView(arrangedSubviews: [labels, action])
                row.axis = .horizontal
                row.alignment = .center
                row.spacing = 8
                views.append(row)
            }
        }
        addCard(views)
    }

    private func addSendCard() {
        var views: [UIView] = [makeLabel("发送", style: .headline)]
        let sendable = service.peers.filter { $0.trusted && $0.connected }
        if let selected = selectedPeerId, !sendable.contains(where: { $0.id == selected }) {
            selectedPeerId = nil
        }
        let targetName = sendable.first { $0.id == selectedPeerId }?.name ?? ""
        let target = makeButton(
            "发送给：" + (targetName.isEmpty ? "选择设备" : targetName),
            filled: false) { [weak self] in
            self?.showTargetMenu()
        }
        target.isEnabled = !sendable.isEmpty
        views.append(target)

        let fileRow = makeRow([
            makeButton("文件…") { [weak self] in self?.showPicker(.files) },
            makeButton("文件夹…") { [weak self] in self?.showPicker(.folder) },
            makeButton("媒体…") { [weak self] in self?.showPicker(.media) },
        ])
        let textRow = makeRow([
            makeButton("发送文本…") { [weak self] in self?.sendTextTapped() },
            makeButton("发送剪贴板") { [weak self] in self?.sendClipboardTapped() },
        ])
        views.append(fileRow)
        views.append(textRow)
        for button in fileRow.arrangedSubviews.compactMap({ $0 as? UIButton }) {
            button.isEnabled = !sendable.isEmpty
        }
        let trustedCount = service.peers.filter { $0.trusted }.count
        for button in textRow.arrangedSubviews.compactMap({ $0 as? UIButton }) {
            button.isEnabled = trustedCount > 0
        }
        addCard(views)
    }

    private func addPendingCard() {
        let pending = service.pendingPairings
        guard !pending.isEmpty else { return }
        var views: [UIView] = [makeLabel("配对确认", style: .headline)]
        views.append(makeNote("核对两台设备显示相同的 6 位数字，再决定是否信任。"))
        for item in pending {
            let name = item.name.isEmpty ? String(item.id.prefix(8)) : item.name
            let label = makeLabel("\(name) · \(item.sas)", style: .title3)
            label.font = .monospacedDigitSystemFont(ofSize: 17, weight: .semibold)
            let accept = makeSmallButton("信任") { [weak self] in
                self?.service.respondPairing(id: item.id, accept: true)
                self?.reload()
            }
            let reject = makeSmallButton("拒绝") { [weak self] in
                self?.service.respondPairing(id: item.id, accept: false)
                self?.reload()
            }
            let row = makeRow([label, accept, reject], equal: false)
            views.append(row)
        }
        addCard(views)
    }

    private func addTransferCard() {
        var views: [UIView] = [makeLabel("进行中与最近传输", style: .headline)]
        let transfers = service.transfers
        if transfers.isEmpty {
            views.append(makeNote("还没有传输。对方发来请求时会先弹窗，确认后才落盘。"))
        } else {
            for item in transfers {
                views.append(makeTransferRow(item))
            }
            if transfers.contains(where: { $0.state.isFinished }) {
                views.append(makeButton("清除已结束", filled: false) { [weak self] in
                    self?.service.clearFinished()
                    self?.reload()
                })
            }
        }
        addCard(views)
    }

    private func makeTransferRow(_ item: HostTransfer) -> UIView {
        let who = item.peerName.isEmpty ? String(item.peerId.prefix(8)) : item.peerName
        let direction = item.outgoing ? "发送 → " : "接收 ← "
        let title = makeLabel(direction + who + " · " + item.state.title, style: .body)
        if item.state == .failed {
            title.textColor = .systemRed
        } else if item.state == .done {
            title.textColor = .systemGreen
        }
        var rows: [UIView] = [title]

        var detail: [String] = []
        if item.fileCount > 0 { detail.append("\(item.fileCount) 项") }
        if item.totalBytes > 0 {
            if item.state == .active {
                detail.append(OpiTransferLimits.bytes(item.transferredBytes) + " / "
                              + OpiTransferLimits.bytes(item.totalBytes))
            } else {
                detail.append(OpiTransferLimits.bytes(item.totalBytes))
            }
        }
        if !item.message.isEmpty { detail.append(item.message) }
        if !detail.isEmpty {
            let label = makeLabel(detail.joined(separator: " · "), style: .caption1)
            label.textColor = .secondaryLabel
            rows.append(label)
        }

        if item.state == .active || item.state == .waiting {
            let progress = UIProgressView(progressViewStyle: .default)
            progress.progress = Float(item.progress)
            rows.append(progress)
        }

        var actions: [UIButton] = []
        if item.isIncomingOffer {
            actions.append(makeSmallButton("接收") { [weak self] in
                self?.service.respondTransfer(id: item.id, accept: true)
                self?.reload()
            })
            actions.append(makeSmallButton("拒绝") { [weak self] in
                self?.service.respondTransfer(id: item.id, accept: false)
                self?.reload()
            })
        } else if item.state == .active || item.state == .waiting {
            actions.append(makeSmallButton("取消") { [weak self] in
                self?.service.cancelTransfer(id: item.id)
                self?.reload()
            })
        } else if item.state == .done, !item.outgoing,
                  service.savedURL(for: item) != nil {
            actions.append(makeSmallButton("打开") { [weak self] in
                self?.shareSaved(item)
            })
        }
        if !actions.isEmpty { rows.append(makeRow(actions)) }

        let box = UIStackView(arrangedSubviews: rows)
        box.axis = .vertical
        box.spacing = 4
        return box
    }

    private func addNotes() {
        var views: [UIView] = [makeLabel("说明", style: .headline)]
        let notes = [
            "只与已配对/已信任设备通信；未配置 App Group 时，本页与键盘扩展的信任表各自独立。",
            "单次最多 128 个条目（含文件夹）、单文件 2 GiB、总计 8 GiB；选完即前置校验。",
            "文本与剪贴板走内核的剪贴板通道，发给所有已信任设备，不会自动粘贴。",
            "接收保存到 App 的 Documents/opi/<传输 ID 前 8 位>；可在「文件」App 的"
                + "「我的 iPhone/小欧输入法」查看，完成后用系统分享打开。",
            "iOS 会在 App 退到后台后挂起：请保持本页在前台，否则传输可能中断。",
        ]
        for line in notes { views.append(makeNote(line)) }
        addCard(views)
    }

    // MARK: - 动作

    @objc private func refreshTapped() {
        service.refreshPeers()
        reload()
    }

    private func showTargetMenu() {
        let sendable = service.peers.filter { $0.trusted && $0.connected }
        guard !sendable.isEmpty else { return }
        let alert = UIAlertController(title: "发送给", message: nil,
                                      preferredStyle: .actionSheet)
        for peer in sendable {
            let name = peer.name.isEmpty ? String(peer.id.prefix(8)) : peer.name
            alert.addAction(UIAlertAction(title: name, style: .default) { [weak self] _ in
                self?.selectedPeerId = peer.id
                self?.reload()
            })
        }
        alert.addAction(UIAlertAction(title: "取消", style: .cancel))
        if let pop = alert.popoverPresentationController {
            pop.sourceView = view
            pop.sourceRect = CGRect(x: view.bounds.midX, y: view.bounds.midY,
                                    width: 1, height: 1)
        }
        present(alert, animated: true)
    }

    private func showPicker(_ kind: PickerKind) {
        guard selectedPeerId != nil else {
            alert("请先选择一台已配对且在线的设备")
            return
        }
        let types: [UTType]
        switch kind {
        case .files: types = [.item]
        case .folder: types = [.folder]
        case .media: types = [.image, .movie, .audio]
        }
        let picker = UIDocumentPickerViewController(
            forOpeningContentTypes: types, asCopy: false)
        picker.allowsMultipleSelection = true
        picker.delegate = self
        present(picker, animated: true)
    }

    func documentPicker(_ controller: UIDocumentPickerViewController,
                        didPickDocumentsAt urls: [URL]) {
        guard !urls.isEmpty, let peerId = selectedPeerId else { return }
        if let error = service.sendFiles(urls, to: peerId) {
            alert(error)
        }
        reload()
    }

    private func sendTextTapped() {
        let box = UIAlertController(
            title: "发送文本",
            message: "经已信任设备的剪贴板通道发送（最多 256 KiB），不会自动粘贴。长文本请先复制再发送剪贴板。",
            preferredStyle: .alert)
        box.addTextField { field in
            field.placeholder = "输入文本"
            field.autocapitalizationType = .none
        }
        box.addAction(UIAlertAction(title: "取消", style: .cancel))
        box.addAction(UIAlertAction(title: "发送", style: .default) { [weak self, weak box] _ in
            guard let self else { return }
            let text = box?.textFields?.first?.text ?? ""
            if let error = self.service.sendText(text) { self.alert(error) }
            self.reload()
        })
        present(box, animated: true)
    }

    private func sendClipboardTapped() {
        if let error = service.sendClipboard() {
            alert(error)
        } else {
            alert("已发送剪贴板文本")
        }
        reload()
    }

    private func presentOffer(_ transfer: HostTransfer) {
        guard lastOfferedId != transfer.id, presentedViewController == nil else { return }
        lastOfferedId = transfer.id
        let who = transfer.peerName.isEmpty ? String(transfer.peerId.prefix(8)) : transfer.peerName
        let count = transfer.fileCount > 0 ? transfer.fileCount : transfer.files.count
        let size = OpiTransferLimits.bytes(transfer.totalBytes)
        let message = "来自 \(who)\n\(count) 个条目 · \(size)\n"
            + "接收后保存到 App 的 Documents/opi。"
        let box = UIAlertController(title: "收到传输请求", message: message,
                                    preferredStyle: .alert)
        box.addAction(UIAlertAction(title: "接收", style: .default) { [weak self] _ in
            self?.service.respondTransfer(id: transfer.id, accept: true)
            self?.reload()
        })
        box.addAction(UIAlertAction(title: "拒绝", style: .destructive) { [weak self] _ in
            self?.service.respondTransfer(id: transfer.id, accept: false)
            self?.reload()
        })
        present(box, animated: true)
    }

    private func shareSaved(_ transfer: HostTransfer) {
        guard let url = service.savedURL(for: transfer) else { return }
        let share = UIActivityViewController(activityItems: [url], applicationActivities: nil)
        if let pop = share.popoverPresentationController {
            pop.sourceView = view
            pop.sourceRect = CGRect(x: view.bounds.midX, y: view.bounds.midY,
                                    width: 1, height: 1)
        }
        present(share, animated: true)
    }

    private func alert(_ message: String) {
        let box = UIAlertController(title: nil, message: message, preferredStyle: .alert)
        box.addAction(UIAlertAction(title: "好", style: .default))
        present(box, animated: true)
    }

    // MARK: - 控件

    private func addCard(_ views: [UIView]) {
        let inner = UIStackView(arrangedSubviews: views)
        inner.axis = .vertical
        inner.spacing = 8
        inner.translatesAutoresizingMaskIntoConstraints = false
        let card = UIView()
        card.backgroundColor = .secondarySystemGroupedBackground
        card.layer.cornerRadius = 12
        card.translatesAutoresizingMaskIntoConstraints = false
        card.addSubview(inner)
        NSLayoutConstraint.activate([
            inner.leadingAnchor.constraint(equalTo: card.leadingAnchor, constant: 16),
            inner.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -16),
            inner.topAnchor.constraint(equalTo: card.topAnchor, constant: 14),
            inner.bottomAnchor.constraint(equalTo: card.bottomAnchor, constant: -14),
        ])
        stack.addArrangedSubview(card)
    }

    private func makeRow(_ views: [UIView], equal: Bool = true) -> UIStackView {
        let row = UIStackView(arrangedSubviews: views)
        row.axis = .horizontal
        row.alignment = .center
        row.spacing = 8
        if equal { row.distribution = .fillEqually }
        return row
    }

    private func makeButton(_ title: String, filled: Bool = true,
                            action: @escaping () -> Void) -> UIButton {
        let b = UIButton(type: .system)
        b.setTitle(title, for: .normal)
        b.titleLabel?.font = .preferredFont(forTextStyle: .headline)
        b.titleLabel?.adjustsFontForContentSizeCategory = true
        b.titleLabel?.numberOfLines = 0
        b.titleLabel?.textAlignment = .center
        if filled {
            b.backgroundColor = .systemBlue
            b.setTitleColor(.white, for: .normal)
        } else {
            b.backgroundColor = .tertiarySystemFill
            b.setTitleColor(.label, for: .normal)
        }
        b.layer.cornerRadius = 9
        b.contentEdgeInsets = UIEdgeInsets(top: 10, left: 12, bottom: 10, right: 12)
        b.addAction(UIAction { _ in action() }, for: .touchUpInside)
        return b
    }

    private func makeSmallButton(_ title: String,
                                 action: @escaping () -> Void) -> UIButton {
        let b = UIButton(type: .system)
        b.setTitle(title, for: .normal)
        b.titleLabel?.font = .preferredFont(forTextStyle: .callout)
        b.backgroundColor = .tertiarySystemFill
        b.layer.cornerRadius = 7
        b.contentEdgeInsets = UIEdgeInsets(top: 5, left: 10, bottom: 5, right: 10)
        b.setContentHuggingPriority(.required, for: .horizontal)
        b.addAction(UIAction { _ in action() }, for: .touchUpInside)
        return b
    }

    private func makeLabel(_ text: String, style: UIFont.TextStyle) -> UILabel {
        let label = UILabel()
        label.text = text
        label.numberOfLines = 0
        label.font = .preferredFont(forTextStyle: style)
        label.adjustsFontForContentSizeCategory = true
        return label
    }

    private func makeNote(_ text: String) -> UILabel {
        let label = makeLabel(text, style: .footnote)
        label.textColor = .secondaryLabel
        return label
    }
}
