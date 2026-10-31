// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 本机（Linux）没有 UIKit/Swift 编译器，未编译未上机；由 apple.yml 真编译。
//
// 跨设备剪贴板（iOS **降级版**）。能力边界如实写在这里，不假装后台：
//   · 只在键盘扩展**可见**时运行：KeyboardViewController.viewWillAppear 起、
//     viewWillDisappear 停；没有后台模式，键盘收起后不同步。
//   · 接收**不自动插入、不自动写剪贴板**：事件只变成键盘顶部一枚 chip，
//     用户点它才 `insertText`（UIPasteboard 写入不提供 —— 用户要的是打进当前输入框）。
//   · 发送必须用户点工具条的剪贴板图标；读取 `UIPasteboard.general` 在 iOS 16+
//     可能触发系统粘贴提示（RequestsOpenAccess=true；**未上机验证**）。
//   · UDP 发现 / TCP 会话在键盘扩展内是否可用**未验证**（扩展网络权限没有单独的
//     开关，靠宿主 App 的本地网络授权；见 NSLocalNetworkUsageDescription）。
//
// 身份/信任目录：优先 App Group `group.xyz.erik.opi`（需要在宿主与扩展的
// entitlements 里同时配置，本仓库当前**没有**签名配置）；取不到时回退到扩展私有
// 目录 —— 那份不会与宿主共享，重装/系统清理会丢，配对需重做。
//
// 句柄线程绑定，全部 FFI 走 `OpiClipRunner`（shared/apple/）。剪贴板内容不落盘、
// 不打日志，错误只留设备名/ID/长度。

import UIKit

extension Notification.Name {
    /// 状态快照变化（主线程）；设置页的剪贴板节用它刷新。
    static let opiClipChanged = Notification.Name("opi.clip.changed")
}

struct OpiClipPeerInfo: Equatable {
    let id: String
    let name: String
    let address: String
    let trusted: Bool
    let connected: Bool
}

struct OpiClipPendingInfo: Equatable {
    let id: String
    let name: String
    let sas: String
}

final class OpiClipboard {
    static let shared = OpiClipboard()
    /// 需要与宿主/扩展的 App Group entitlement 一致；未配置时下面的 fallback 生效。
    static let appGroupID = "group.xyz.erik.opi"
    private static let maxTextBytes = 256 * 1024

    private let runner = OpiClipRunner()
    private let lock = NSLock()
    private weak var controller: KeyboardViewController?
    private var visible = false
    private var peersCache: [OpiClipPeerInfo] = []
    private var pendingCache: [OpiClipPendingInfo] = []
    private var errorText = ""
    private var sourceName: String?
    private var pendingText: String?
    private var chip: UIView?
    private var flashView: UILabel?
    private var flashWork: DispatchWorkItem?

    private init() {
        runner.onEvent = { [weak self] event in self?.handleEvent(event) }
        runner.onPeers = { [weak self] list in self?.handlePeers(list) }
        runner.onState = { [weak self] running, error in self?.handleState(running, error) }
    }

    // MARK: - 快照（主线程读）

    var isEnabled: Bool { KeyboardPreferences.clipEnabled }
    var isRunning: Bool { runner.isRunning }
    var peers: [OpiClipPeerInfo] { withLock { peersCache } }
    var pendingPairings: [OpiClipPendingInfo] { withLock { pendingCache } }
    var lastErrorText: String { withLock { errorText } }
    var lastSourceText: String? { withLock { sourceName } }

    // MARK: - 生命周期（KeyboardViewController 调）

    func keyboardDidAppear(_ controller: KeyboardViewController) {
        self.controller = controller
        visible = true
        if isEnabled { start() }
    }

    func keyboardDidDisappear() {
        visible = false
        stop()
    }

    func setEnabled(_ on: Bool) {
        KeyboardPreferences.clipEnabled = on
        if on, visible { start() } else { stop() }
        postChanged()
    }

    private func start() {
        guard visible, isEnabled, controller != nil else { return }
        runner.start(configJSON: Self.configJSON(deviceName: Self.localDeviceName()))
    }

    private func stop() {
        runner.stop()
        pendingText = nil
        hideChip()
        withLock {
            peersCache = []
            pendingCache = []
            sourceName = nil
            errorText = ""
        }
        postChanged()
    }

    // MARK: - 手动发送（工具条图标）

    func sendCurrentClipboard() {
        guard isEnabled else {
            showFlash("请先在设置里开启跨设备剪贴板")
            return
        }
        if controller?.hasFullAccess == false {
            showFlash("请先在系统设置里为本输入法开启完全访问")
            return
        }
        // ⚠️ 读取共享剪贴板：完整访问已开时键盘扩展可直接读；是否弹系统粘贴提示
        // 随 iOS 版本与授权状态而异，未上机验证。
        guard let text = UIPasteboard.general.string, !text.isEmpty else {
            showFlash("本机剪贴板没有文本")
            return
        }
        guard text.utf8.count <= Self.maxTextBytes else {
            showFlash("文本超过 256 KiB 上限，未发送")
            return
        }
        runner.publish(text)
        showFlash("已发送剪贴板")
    }

    // MARK: - 对端操作（设置页）

    func pair(_ peerId: String) { runner.pair(peerId) }

    func forget(_ peerId: String) {
        runner.forget(peerId)
        withLock { pendingCache.removeAll { $0.id == peerId } }
        postChanged()
    }

    func refresh() { runner.refreshPeers() }

    func respondPairing(at index: Int, accept: Bool) {
        let pending = withLock {
            pendingCache.indices.contains(index) ? pendingCache[index] : nil
        }
        guard let pending else { return }
        runner.confirm(pending.id, accept: accept)
        withLock { pendingCache.removeAll { $0.id == pending.id } }
        postChanged()
    }

    // MARK: - Runner 回调（剪贴板线程）

    private func handleEvent(_ event: [String: Any]) {
        guard let type = event["type"] as? String else { return }
        let id = event["instance_id"] as? String ?? ""
        switch type {
        case "clipboard_received":
            guard let text = event["text"] as? String else { return }
            DispatchQueue.main.async { [weak self] in
                self?.showIncoming(text, fromId: id)
            }
        case "pairing_prompt":
            let sas = event["sas"] as? String ?? ""
            let incoming = event["incoming"] as? Bool ?? false
            let name = peerName(id)
            withLock {
                pendingCache.removeAll { $0.id == id }
                pendingCache.append(OpiClipPendingInfo(id: id, name: name, sas: sas))
            }
            postChanged()
            DispatchQueue.main.async { [weak self] in
                self?.presentPairing(id: id, sas: sas, incoming: incoming, attempt: 0)
            }
        case "pairing_finished":
            let trusted = event["trusted"] as? Bool ?? false
            withLock {
                pendingCache.removeAll { $0.id == id }
                errorText = trusted ? "" : "配对未完成（对端拒绝或超时）"
            }
            postChanged()
            DispatchQueue.main.async { [weak self] in
                self?.showFlash(trusted ? "配对成功" : "配对未完成")
            }
        case "send_failed":
            withLock { errorText = "发送失败：对端离线或会话已断开" }
            postChanged()
            DispatchQueue.main.async { [weak self] in self?.showFlash("发送失败") }
        case "error":
            let message = event["message"] as? String ?? "剪贴板内核报错"
            withLock { errorText = message }
            postChanged()
            DispatchQueue.main.async { [weak self] in self?.showFlash(message) }
        default:
            break
        }
    }

    private func handlePeers(_ list: [[String: Any]]) {
        let parsed: [OpiClipPeerInfo] = list.compactMap { dict in
            guard let id = dict["instance_id"] as? String else { return nil }
            return OpiClipPeerInfo(
                id: id,
                name: dict["name"] as? String ?? "",
                address: dict["address"] as? String ?? "",
                trusted: dict["trusted"] as? Bool ?? false,
                connected: dict["connected"] as? Bool ?? false
            )
        }.sorted {
            if $0.connected != $1.connected { return $0.connected }
            if $0.trusted != $1.trusted { return $0.trusted }
            return $0.name < $1.name
        }
        let changed = withLock { () -> Bool in
            let peersChanged = parsed != peersCache
            if peersChanged { peersCache = parsed }
            let named = pendingCache.map { pending in
                let name = parsed.first { $0.id == pending.id }?.name ?? ""
                guard !name.isEmpty, name != pending.name else { return pending }
                return OpiClipPendingInfo(id: pending.id, name: name, sas: pending.sas)
            }
            let pendingChanged = named != pendingCache
            pendingCache = named
            return peersChanged || pendingChanged
        }
        if changed { postChanged() }
    }

    private func handleState(_ nowRunning: Bool, _ message: String) {
        _ = nowRunning // running 由 runner 暴露；这里只关心错误文本
        let changed = withLock { () -> Bool in
            guard errorText != message else { return false }
            errorText = message
            return true
        }
        guard changed else { return }
        postChanged()
        if !message.isEmpty {
            DispatchQueue.main.async { [weak self] in self?.showFlash(message) }
        }
    }

    // MARK: - 接收 chip（点击才插入）

    private func showIncoming(_ text: String, fromId id: String) {
        guard visible, isEnabled else { return }
        pendingText = text
        let name = peerName(id)
        withLock { sourceName = name }
        showChip()
        postChanged()
        // 不自动 insertText、不自动写 UIPasteboard、不展示内容。
    }

    private func insertPending() {
        guard let text = pendingText else { return }
        pendingText = nil
        hideChip()
        // unmark + insertText 都在 KeyboardViewController.insert 里（同一入口）。
        controller?.insert(text)
        postChanged()
    }

    private func dismissPending() {
        pendingText = nil
        hideChip()
        postChanged()
    }

    private func showChip() {
        guard let view = controller?.view else { return }
        let chip = self.chip ?? makeChip()
        if chip.superview == nil {
            view.addSubview(chip)
            NSLayoutConstraint.activate([
                chip.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor,
                                               constant: -6),
                chip.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor,
                                          constant: 3),
                chip.heightAnchor.constraint(equalToConstant: 28),
                chip.widthAnchor.constraint(lessThanOrEqualToConstant: 260),
            ])
        }
        chip.isHidden = false
        view.bringSubviewToFront(chip)
    }

    /// 顶部工具条右侧的一枚临时 chip。会遮住工具条右端（关于/收起），
    /// 但只在有待接收内容时出现，点「插入」或「×」即消失 —— 属于可接受的取舍。
    private func makeChip() -> UIView {
        let container = UIView()
        container.translatesAutoresizingMaskIntoConstraints = false
        container.backgroundColor = .secondarySystemBackground
        container.layer.cornerRadius = 14
        container.layer.borderWidth = 1
        container.layer.borderColor = UIColor.separator.cgColor
        container.isHidden = true

        let insert = UIButton(type: .system)
        insert.setTitle("📋 收到剪贴板 · 插入", for: .normal)
        insert.titleLabel?.font = .systemFont(ofSize: 12)
        insert.accessibilityLabel = "插入收到的剪贴板内容"
        insert.addAction(UIAction { [weak self] _ in self?.insertPending() }, for: .touchUpInside)

        let close = UIButton(type: .system)
        close.setTitle("×", for: .normal)
        close.titleLabel?.font = .systemFont(ofSize: 15)
        close.accessibilityLabel = "忽略这条剪贴板内容"
        close.addAction(UIAction { [weak self] _ in self?.dismissPending() }, for: .touchUpInside)
        close.setContentHuggingPriority(.required, for: .horizontal)

        let row = UIStackView(arrangedSubviews: [insert, close])
        row.axis = .horizontal
        row.alignment = .center
        row.spacing = 6
        row.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(row)
        NSLayoutConstraint.activate([
            row.leadingAnchor.constraint(equalTo: container.leadingAnchor, constant: 10),
            row.trailingAnchor.constraint(equalTo: container.trailingAnchor, constant: -8),
            row.topAnchor.constraint(equalTo: container.topAnchor),
            row.bottomAnchor.constraint(equalTo: container.bottomAnchor),
        ])
        chip = container
        return container
    }

    private func hideChip() {
        chip?.isHidden = true
    }

    // MARK: - 配对确认（SAS）

    /// 键盘里唯一能弹窗的时机就是扩展可见时；设置面板开着就先等它关掉（最多 15s）。
    private func presentPairing(id: String, sas: String, incoming: Bool, attempt: Int) {
        guard visible, let controller else { return }
        if controller.presentedViewController != nil {
            guard attempt < 15 else { return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 1) { [weak self] in
                self?.presentPairing(id: id, sas: sas, incoming: incoming, attempt: attempt + 1)
            }
            return
        }
        let name = peerName(id)
        let alert = UIAlertController(
            title: incoming ? "\(name) 请求配对" : "与 \(name) 配对",
            message: "确认码：\(sas)\n请确认另一台设备显示相同的 6 位数字，再决定是否信任。",
            preferredStyle: .alert
        )
        alert.addAction(UIAlertAction(title: "信任", style: .default) { [weak self] _ in
            self?.runner.confirm(id, accept: true)
        })
        alert.addAction(UIAlertAction(title: "拒绝", style: .cancel) { [weak self] _ in
            self?.runner.confirm(id, accept: false)
        })
        controller.present(alert, animated: true)
    }

    // MARK: - 工具

    private func postChanged() {
        DispatchQueue.main.async {
            NotificationCenter.default.post(name: .opiClipChanged, object: nil)
        }
    }

    private func withLock<T>(_ body: () -> T) -> T {
        lock.lock()
        defer { lock.unlock() }
        return body()
    }

    private func peerName(_ id: String) -> String {
        withLock {
            let name = peersCache.first { $0.id == id }?.name ?? ""
            return name.isEmpty ? String(id.prefix(8)) : name
        }
    }

    /// 短暂提示（发送结果/错误）；1.6s 后自消失。不显示剪贴板内容。
    private func showFlash(_ message: String) {
        guard let view = controller?.view, !message.isEmpty else { return }
        flashWork?.cancel()
        flashView?.removeFromSuperview()
        let label = UILabel()
        label.text = message
        label.font = .systemFont(ofSize: 12)
        label.textColor = .label
        label.backgroundColor = .secondarySystemBackground
        label.textAlignment = .center
        label.layer.cornerRadius = 12
        label.layer.masksToBounds = true
        label.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(label)
        NSLayoutConstraint.activate([
            label.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            label.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 4),
            label.heightAnchor.constraint(equalToConstant: 24),
            label.widthAnchor.constraint(lessThanOrEqualTo: view.widthAnchor, constant: -16),
        ])
        flashView = label
        let work = DispatchWorkItem { [weak self, weak label] in
            label?.removeFromSuperview()
            if self?.flashView === label { self?.flashView = nil }
        }
        flashWork = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.6, execute: work)
    }

    // MARK: - 配置

    private static func localDeviceName() -> String {
        var name = UIDevice.current.name
        while name.utf8.count > 64 { name.removeLast() }
        return name.isEmpty ? "iPhone" : name
    }

    private static func dataDirectory() -> URL {
        // App Group 优先：宿主与扩展在同一容器里共享身份/信任表。
        if let group = FileManager.default.containerURL(
            forSecurityApplicationGroupIdentifier: appGroupID
        ) {
            return group.appendingPathComponent("clip")
        }
        // fallback：扩展自己的 Application Support。与宿主不共享，重装即丢。
        let base = FileManager.default.urls(
            for: .applicationSupportDirectory, in: .userDomainMask
        ).first ?? URL(fileURLWithPath: NSTemporaryDirectory())
        return base.appendingPathComponent("opi-clip")
    }

    private static func configJSON(deviceName: String) -> String {
        let object: [String: Any] = [
            "data_dir": dataDirectory().path,
            "tcp_port": 0,
            "discovery_port": 45888,
            "device_name": deviceName,
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: object),
              let json = String(data: data, encoding: .utf8) else {
            return ""
        }
        return json
    }
}
