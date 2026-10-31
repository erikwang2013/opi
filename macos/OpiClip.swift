// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 本机（Linux）没有 Swift 编译器，未编译；由 apple.yml 真编译。
//
// 跨设备剪贴板（macOS 宿主）。内核（opi-clip）挂在 IMK 常驻进程里 ——
// C 句柄线程绑定，所有 FFI 调用都在 `OpiClipRunner` 的专用线程上（shared/apple/）。
// 本文件只做三件平台相关的事：
//   1. NSPasteboard changeCount 轮询（~0.5s，只处理纯文本）；
//   2. 收到远端文本后写入本机剪贴板（**不自动粘贴**、不记录内容）；
//   3. 菜单/设置页用的状态快照（对端、待确认 SAS、最近来源、错误）。
//
// 防回环有两道：内核按内容哈希抑制（PublishOutcome::SuppressedLoop），
// 本层写入后把 changeCount 对齐，watcher 不会再把同一份内容发出去。
//
// 开关默认关（UserDefaults 键 `ClipEnabled`）；`applyPersisted()` 在 main.swift 调。
// 隐私：不打印剪贴板内容，日志只有设备名/ID/长度/错误。

import AppKit
import Foundation

struct OpiClipPeer: Equatable {
    let id: String
    let name: String
    let address: String
    let trusted: Bool
    let connected: Bool
}

struct OpiClipPending: Equatable {
    let id: String
    let name: String
    let sas: String
    let incoming: Bool
}

final class OpiClipService {
    static let shared = OpiClipService()
    /// 与 core 的 DEFAULT_DISCOVERY_PORT 一致（crates/opi-clip/src/types.rs）。
    static let discoveryPort = 45888
    /// 与 core 的文本上限一致（UTF-8 字节）。
    private static let maxTextBytes = 256 * 1024

    /// 状态变化（主线程）。设置页用它就地重画；菜单每次打开现读快照。
    var onChange: (() -> Void)?

    private let runner = OpiClipRunner()
    private let lock = NSLock()
    private var enabled = false
    private var peersCache: [OpiClipPeer] = []
    private var pendingCache: [OpiClipPending] = []
    private var running = false
    private var errorText = ""
    private var sourceName: String?
    private var pasteboardTimer: Timer?
    private var lastChangeCount = NSPasteboard.general.changeCount

    private init() {
        runner.onEvent = { [weak self] event in self?.handleEvent(event) }
        runner.onPeers = { [weak self] list in self?.handlePeers(list) }
        runner.onState = { [weak self] running, error in self?.handleState(running, error) }
    }

    // MARK: - 快照（主线程读）

    var isEnabled: Bool { enabled }
    var isRunning: Bool { withLock { running } }
    var peers: [OpiClipPeer] { withLock { peersCache } }
    var pendingPairings: [OpiClipPending] { withLock { pendingCache } }
    var lastErrorText: String { withLock { errorText } }
    var lastSourceText: String? { withLock { sourceName } }
    var deviceName: String { Self.localDeviceName() }

    // MARK: - 生命周期

    /// main.swift 启动时调用：只按落盘开关决定是否启动，不碰剪贴板内容。
    func applyPersisted() {
        enabled = false
        if OpiSettingsStore.bool("ClipEnabled", default: false) {
            setEnabled(true)
        }
    }

    func setEnabled(_ on: Bool) {
        guard on != enabled else { return }
        enabled = on
        OpiSettingsStore.set("ClipEnabled", on)
        if on {
            // 从当前计数起算：启动前就存在的剪贴板内容不该被广播。
            lastChangeCount = NSPasteboard.general.changeCount
            runner.start(configJSON: Self.configJSON(deviceName: Self.localDeviceName()))
            startPasteboardWatch()
        } else {
            runner.stop()
            pasteboardTimer?.invalidate()
            pasteboardTimer = nil
            withLock {
                peersCache = []
                pendingCache = []
                running = false
                errorText = ""
                sourceName = nil
            }
        }
        onChange?()
    }

    /// 「一键停止」= 关开关并释放端口；信任表保留（核心负责落盘）。
    func stopAll() {
        setEnabled(false)
    }

    func pair(_ peerId: String) {
        runner.pair(peerId)
    }

    func forget(_ peerId: String) {
        runner.forget(peerId)
        withLock { pendingCache.removeAll { $0.id == peerId } }
        onChange?()
    }

    func refresh() {
        runner.refreshPeers()
    }

    /// 接受/拒绝第 index 条待确认配对（菜单与设置页共用；按下标取当时的快照）。
    func respondPending(at index: Int, accept: Bool) {
        let pending = withLock {
            pendingCache.indices.contains(index) ? pendingCache[index] : nil
        }
        guard let pending else { return }
        runner.confirm(pending.id, accept: accept)
        withLock { pendingCache.removeAll { $0.id == pending.id } }
        onChange?()
    }

    // MARK: - 本机 → 远端（主线程轮询）

    private func startPasteboardWatch() {
        pasteboardTimer?.invalidate()
        let timer = Timer(timeInterval: 0.5, repeats: true) { [weak self] _ in
            self?.pollPasteboard()
        }
        // .common：菜单/设置窗跟踪期间也要跑。
        RunLoop.main.add(timer, forMode: .common)
        pasteboardTimer = timer
    }

    private func pollPasteboard() {
        guard enabled else { return }
        let pasteboard = NSPasteboard.general
        let count = pasteboard.changeCount
        guard count != lastChangeCount else { return }
        lastChangeCount = count
        // 只处理纯文本：图片/文件/HTML 等没有 string 表示就跳过。
        guard let text = pasteboard.string(forType: .string), !text.isEmpty else { return }
        guard text.utf8.count <= Self.maxTextBytes else {
            withLock { errorText = "本机剪贴板文本超过 256 KiB，未发送" }
            onChange?()
            return
        }
        runner.publish(text)
    }

    // MARK: - 远端 → 本机

    private func receive(_ text: String, fromId id: String) {
        guard enabled else { return } // 关闭瞬间还在队列里的事件不写剪贴板
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        let ok = pasteboard.setString(text, forType: .string)
        // 自己写入的回声：直接对齐计数，watcher 不会把同一份内容再发布一次。
        lastChangeCount = pasteboard.changeCount
        // peerName 自己上锁（NSLock 不可重入），先算再进临界区。
        let name = peerName(id)
        withLock {
            sourceName = name
            errorText = ok ? "" : "收到远端文本，但写入本机剪贴板失败"
        }
        onChange?()
    }

    // MARK: - Runner 回调（剪贴板线程）

    private func handleEvent(_ event: [String: Any]) {
        guard let type = event["type"] as? String else { return }
        let id = event["instance_id"] as? String ?? ""
        switch type {
        case "clipboard_received":
            guard let text = event["text"] as? String else { return }
            // 写剪贴板必须回主线程（AppKit）。
            DispatchQueue.main.async { [weak self] in self?.receive(text, fromId: id) }
        case "pairing_prompt":
            let sas = event["sas"] as? String ?? ""
            let incoming = event["incoming"] as? Bool ?? false
            let name = peerName(id)
            withLock {
                pendingCache.removeAll { $0.id == id }
                pendingCache.append(OpiClipPending(id: id, name: name, sas: sas, incoming: incoming))
            }
            notifyMain()
        case "pairing_finished":
            let trusted = event["trusted"] as? Bool ?? false
            let reason = event["reason"] as? String ?? "对端拒绝或超时"
            withLock {
                pendingCache.removeAll { $0.id == id }
                errorText = trusted ? "" : "配对未完成：\(reason)"
            }
            notifyMain()
        case "send_failed":
            withLock { errorText = "发送失败：对端离线或会话已断开" }
            notifyMain()
        case "error":
            withLock { errorText = event["message"] as? String ?? "剪贴板内核报错" }
            notifyMain()
        default:
            break
        }
    }

    private func handlePeers(_ list: [[String: Any]]) {
        let parsed: [OpiClipPeer] = list.compactMap { dict in
            guard let id = dict["instance_id"] as? String else { return nil }
            return OpiClipPeer(
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
            // pairing_prompt 可能早于对端快照到达：用快照补上显示名。
            let named = pendingCache.map { pending in
                let name = parsed.first { $0.id == pending.id }?.name ?? ""
                guard !name.isEmpty, name != pending.name else { return pending }
                return OpiClipPending(
                    id: pending.id, name: name, sas: pending.sas, incoming: pending.incoming)
            }
            let pendingChanged = named != pendingCache
            pendingCache = named
            return peersChanged || pendingChanged
        }
        if changed { notifyMain() }
    }

    private func handleState(_ nowRunning: Bool, _ message: String) {
        let changed = withLock { () -> Bool in
            guard running != nowRunning || errorText != message else { return false }
            running = nowRunning
            errorText = message
            return true
        }
        if changed { notifyMain() }
    }

    private func notifyMain() {
        DispatchQueue.main.async { [weak self] in self?.onChange?() }
    }

    // MARK: - 工具

    private func peerName(_ id: String) -> String {
        withLock {
            let name = peersCache.first { $0.id == id }?.name ?? ""
            return name.isEmpty ? String(id.prefix(8)) : name
        }
    }

    private func withLock<T>(_ body: () -> T) -> T {
        lock.lock()
        defer { lock.unlock() }
        return body()
    }

    private static func localDeviceName() -> String {
        // 只取主机名：`Host.localizedName` 在本机无编译器可核对，不赌未验证的 API。
        var name = ProcessInfo.processInfo.hostName
        // core 校验 device_name ≤ 64 字节（UTF-8）。
        while name.utf8.count > 64 { name.removeLast() }
        return name.isEmpty ? "Mac" : name
    }

    private static func configJSON(deviceName: String) -> String {
        let object: [String: Any] = [
            "data_dir": OpiEngine.dataDirectory.appendingPathComponent("clip").path,
            "tcp_port": 0,
            "discovery_port": discoveryPort,
            "device_name": deviceName,
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: object),
              let json = String(data: data, encoding: .utf8) else {
            return ""
        }
        return json
    }
}
