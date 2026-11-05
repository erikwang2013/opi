// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 本机（Linux）没有 Swift/UIKit 编译器，未编译；由 apple.yml 真编译。
//
// iOS 宿主 App 的跨设备传输服务。键盘扩展不能后台跑传输，所以发现/收发/落盘
// 都在这里；扩展只负责用 `opi-transfer://transfer` 打开本 App（TransferKeyboard.swift）。
//
// 内核是与键盘剪贴板相同的 `opi-clip`，但宿主进程有自己的一份节点与信任目录
// （未配置 App Group 时与扩展不共享，需要在本页重新配对）。句柄线程绑定，
// 所有 FFI 调用都在 `OpiClipRunner` 的专用线程上（shared/apple/）。
//
// 生命周期：传输页出现时启动，App 退到后台由系统挂起（iOS 不允许常驻）；
// 本服务不轮询 UIPasteboard（避免反复触发系统粘贴提示），剪贴板发送必须用户点按。

import Foundation
import UIKit

struct HostTransferFile {
    let relativePath: String
    let size: UInt64
    let mime: String
    let isDir: Bool
}

enum HostTransferState {
    case waiting
    case active
    case done
    case failed
    case canceled

    var title: String {
        switch self {
        case .waiting: return "等待中"
        case .active: return "传输中"
        case .done: return "已完成"
        case .failed: return "失败"
        case .canceled: return "已取消"
        }
    }
    var isFinished: Bool {
        self == .done || self == .failed || self == .canceled
    }
}

struct HostTransfer {
    let id: String
    var peerId: String
    var peerName: String
    var outgoing: Bool
    var files: [HostTransferFile]
    var totalBytes: UInt64
    var transferredBytes: UInt64
    var fileIndex: Int
    var fileCount: Int
    var state: HostTransferState
    var savedDir: String?
    var message: String

    init(id: String, peerId: String, peerName: String, outgoing: Bool) {
        self.id = id
        self.peerId = peerId
        self.peerName = peerName
        self.outgoing = outgoing
        files = []
        totalBytes = 0
        transferredBytes = 0
        fileIndex = 0
        fileCount = 0
        state = .waiting
        savedDir = nil
        message = ""
    }

    var progress: Double {
        guard totalBytes > 0 else { return state == .done ? 1 : 0 }
        return min(1, Double(transferredBytes) / Double(totalBytes))
    }
    var isIncomingOffer: Bool {
        !outgoing && state == .waiting
    }
}

struct HostPeerInfo: Equatable {
    let id: String
    let name: String
    let address: String
    let trusted: Bool
    let connected: Bool
}

struct HostPairingInfo: Equatable {
    let id: String
    let name: String
    let sas: String
    let incoming: Bool
}

final class OpiTransferService {
    static let shared = OpiTransferService()

    var onChange: (() -> Void)?
    var onIncomingOffer: ((HostTransfer) -> Void)?

    let runner = OpiClipRunner()
    private let lock = NSLock()
    private var peersCache: [HostPeerInfo] = []
    private var transfersCache: [HostTransfer] = []
    private var pendingCache: [HostPairingInfo] = []
    private var accessCache: [String: [URL]] = [:]
    private var runningCache = false
    private var errorCache = ""

    private init() {
        runner.onEvent = { [weak self] event in self?.handleEvent(event) }
        runner.onPeers = { [weak self] list in self?.handlePeers(list) }
        runner.onState = { [weak self] running, error in
            self?.handleState(running, error)
        }
    }

    // MARK: - 快照
    var isRunning: Bool { withLock { runningCache } }
    var peers: [HostPeerInfo] { withLock { peersCache } }
    var transfers: [HostTransfer] { withLock { transfersCache } }
    var pendingPairings: [HostPairingInfo] { withLock { pendingCache } }
    var lastErrorText: String { withLock { errorCache } }
    var deviceName: String { opiTransferLocalDeviceName() }

    /// 用户填的自定义名（空 = 系统默认）。
    var customDeviceName: String {
        UserDefaults.standard.string(forKey: "DeviceName") ?? ""
    }

    /// 校验并保存设备名；返回 nil = 成功。运行中会重建内核让 beacon 用新名字。
    @discardableResult
    func setDeviceName(_ raw: String) -> String? {
        let name = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if let error = opiTransferDeviceNameError(name) { return error }
        let wasRunning = withLock { runningCache }
        if wasRunning { runner.stop() }
        UserDefaults.standard.set(name, forKey: "DeviceName")
        if wasRunning {
            runner.start(configJSON: Self.configJSON(deviceName: opiTransferLocalDeviceName()))
        }
        notifyMain()
        return nil
    }

    // MARK: - 生命周期

    /// 幂等：传输页每次出现都可以调；内核已在运行时不会重启。
    func start() {
        _ = Self.dataDirectory()
        runner.start(configJSON: Self.configJSON(deviceName: opiTransferLocalDeviceName()))
    }

    func stop() {
        runner.stop()
        withLock {
            peersCache = []
            pendingCache = []
            runningCache = false
        }
        releaseAllAccess()
        notifyMain()
    }

    // MARK: - 配对与设备
    func pair(_ peerId: String) { runner.pair(peerId) }
    func refreshPeers() { runner.refreshPeers() }

    func forget(_ peerId: String) {
        runner.forget(peerId)
        withLock { pendingCache.removeAll { $0.id == peerId } }
        notifyMain()
    }

    func respondPairing(id: String, accept: Bool) {
        runner.confirm(id, accept: accept)
        withLock { pendingCache.removeAll { $0.id == id } }
        notifyMain()
    }

    // MARK: - 发送
    /// nil = 已交给内核；否则是前置拒绝原因（未信任/离线/超限）。
    func sendFiles(_ urls: [URL], to peerId: String) -> String? {
        guard let peer = peers.first(where: { $0.id == peerId }) else { return "目标设备不存在" }
        guard peer.trusted else { return "只能发送给已配对/已信任设备" }
        guard peer.connected else { return "目标设备不在线" }
        guard isRunning else { return "传输服务未启动，请稍候重试" }
        // 文件选择器给的是安全作用域 URL：先取得访问权，前置校验才读得到大小。
        let scoped = urls.filter { $0.startAccessingSecurityScopedResource() }
        if let violation = OpiTransferLimits.violation(urls) {
            scoped.forEach { $0.stopAccessingSecurityScopedResource() }
            return violation
        }
        guard let json = OpiTransferLimits.filesJSON(paths: urls.map { $0.path }) else {
            scoped.forEach { $0.stopAccessingSecurityScopedResource() }
            return "无法生成文件清单"
        }
        withLock { errorCache = "" }
        runner.sendFiles(peerId: peerId, filesJSON: json) { [weak self] id in
            guard let self else { return }
            if let id {
                self.withLock { self.accessCache[id] = scoped }
            } else {
                scoped.forEach { $0.stopAccessingSecurityScopedResource() }
            }
            self.notifyMain()
        }
        return nil
    }

    /// 文本走剪贴板通道：只发给已信任设备；定向由 `setTextRoutes` 决定。
    func sendText(_ text: String) -> String? {
        guard !text.isEmpty else { return "文本为空" }
        guard text.utf8.count <= 256 * 1024 else { return "文本超过 256 KiB 上限" }
        guard isRunning else { return "传输服务未启动，请稍候重试" }
        guard peers.contains(where: { $0.trusted }) else { return "没有已配对/已信任设备" }
        withLock { errorCache = "" }
        runner.publish(text)
        return nil
    }
    func sendClipboard() -> String? {
        guard let text = UIPasteboard.general.string, !text.isEmpty else {
            return "本机剪贴板没有纯文本"
        }
        return sendText(text)
    }

    // MARK: - 接收

    static var saveRoot: URL {
        let docs = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        return docs.appendingPathComponent("opi", isDirectory: true)
    }

    /// 接受时保存到 `Documents/opi/<id 前 8 位>`；拒绝不需要目录。
    func respondTransfer(id: String, accept: Bool) {
        var dir = ""
        if accept {
            let url = Self.saveRoot.appendingPathComponent(
                String(id.prefix(8)), isDirectory: true)
            do {
                try FileManager.default.createDirectory(
                    at: url, withIntermediateDirectories: true)
                dir = url.path
            } catch {
                withLock { errorCache = "无法创建接收目录：\(error.localizedDescription)" }
                notifyMain()
                return
            }
        }
        runner.respondTransfer(id: id, accept: accept, saveDir: dir) { [weak self] ok in
            guard let self else { return }
            if !ok {
                self.withLock { self.errorCache = "传输应答失败（可能已超时）" }
                self.notifyMain()
            }
        }
    }

    func cancelTransfer(id: String) {
        runner.cancelTransfer(id: id) { [weak self] ok in
            guard let self else { return }
            if !ok {
                self.withLock { self.errorCache = "取消失败（可能已结束）" }
                self.notifyMain()
            }
        }
    }
    func clearFinished() {
        withLock {
            transfersCache.removeAll { $0.state.isFinished }
            errorCache = ""
        }
        notifyMain()
    }

    func savedURL(for transfer: HostTransfer) -> URL? {
        guard let dir = transfer.savedDir, !dir.isEmpty else { return nil }
        let url = URL(fileURLWithPath: dir)
        return FileManager.default.fileExists(atPath: url.path) ? url : nil
    }

    // MARK: - Runner 回调（传输线程）

    private func handleEvent(_ event: [String: Any]) {
        guard let type = event["type"] as? String else { return }
        let peerId = event["instance_id"] as? String ?? ""
        switch type {
        case "pairing_prompt":
            let sas = event["sas"] as? String ?? ""
            let incoming = event["incoming"] as? Bool ?? false
            let eventName = event["name"] as? String ?? ""
            let name = eventName.isEmpty ? peerName(peerId) : eventName
            withLock {
                pendingCache.removeAll { $0.id == peerId }
                pendingCache.append(
                    HostPairingInfo(id: peerId, name: name, sas: sas, incoming: incoming))
            }
            notifyMain()
        case "pairing_finished":
            let trusted = event["trusted"] as? Bool ?? false
            let reason = event["reason"] as? String ?? "对端拒绝或超时"
            withLock {
                pendingCache.removeAll { $0.id == peerId }
                if !trusted { errorCache = "配对未完成：\(reason)" }
            }
            notifyMain()
        case "transfer_offer", "transfer_progress", "transfer_completed",
             "transfer_failed", "transfer_canceled":
            handleTransferEvent(type: type, event: event)
        case "error":
            withLock { errorCache = event["message"] as? String ?? "传输内核报错" }
            notifyMain()
        default:
            break
        }
    }
    private func handleTransferEvent(type: String, event: [String: Any]) {
        guard let id = event["transfer_id"] as? String, !id.isEmpty else { return }
        let peerId = event["instance_id"] as? String ?? ""
        let outgoing = event["outgoing"] as? Bool ?? false
        let name = peerName(peerId)
        let eventFiles = Self.parseFiles(event["files"])
        let total = Self.u64(event["total_bytes"])
        let transferred = Self.u64(event["transferred_bytes"])
        let fileIndex = Int(Self.u64(event["file_index"]))
        let fileCount = Int(Self.u64(event["file_count"]))
        let savedDir = event["saved_dir"] as? String
        let reason = event["reason"] as? String ?? "传输失败"
        var terminal = false
        var offer: HostTransfer?

        withLock {
            let known = transfersCache.firstIndex { $0.id == id }
            var item = known.map { transfersCache[$0] }
                ?? HostTransfer(id: id, peerId: peerId, peerName: name, outgoing: outgoing)
            item.peerId = peerId
            item.peerName = name
            item.outgoing = outgoing
            switch type {
            case "transfer_offer":
                if !eventFiles.isEmpty { item.files = eventFiles }
                if total > 0 { item.totalBytes = total }
                if item.fileCount == 0 { item.fileCount = item.files.count }
                item.state = .waiting
                item.message = outgoing ? "等待对方接收" : "待确认接收"
                if !outgoing { offer = item }
            case "transfer_progress":
                item.state = .active
                item.transferredBytes = transferred
                if total > 0 { item.totalBytes = total }
                item.fileIndex = fileIndex
                if fileCount > 0 { item.fileCount = fileCount }
                item.message = ""
            case "transfer_completed":
                item.state = .done
                item.transferredBytes = total > 0 ? total : item.totalBytes
                if let savedDir { item.savedDir = savedDir }
                if !eventFiles.isEmpty { item.files = eventFiles }
                item.message = outgoing ? "已发送" : "已接收"
                terminal = true
            case "transfer_failed":
                item.state = .failed
                item.message = reason
                terminal = true
            case "transfer_canceled":
                item.state = .canceled
                item.message = "已取消"
                terminal = true
            default:
                return
            }
            if let known {
                transfersCache[known] = item
            } else {
                transfersCache.append(item)
            }
            if transfersCache.count > 50 {
                transfersCache.removeFirst(transfersCache.count - 50)
            }
        }

        if terminal {
            let scoped = withLock { accessCache.removeValue(forKey: id) }
            scoped?.forEach { $0.stopAccessingSecurityScopedResource() }
        }
        if let offer {
            DispatchQueue.main.async { [weak self] in self?.onIncomingOffer?(offer) }
        }
        notifyMain()
    }
    private func handlePeers(_ list: [[String: Any]]) {
        let parsed: [HostPeerInfo] = list.compactMap { dict in
            guard let id = dict["instance_id"] as? String else { return nil }
            return HostPeerInfo(
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
                return HostPairingInfo(
                    id: pending.id, name: name, sas: pending.sas, incoming: pending.incoming)
            }
            let pendingChanged = named != pendingCache
            pendingCache = named
            return peersChanged || pendingChanged
        }
        if changed { notifyMain() }
    }

    private func handleState(_ nowRunning: Bool, _ message: String) {
        withLock {
            runningCache = nowRunning
            errorCache = message
        }
        notifyMain()
    }

    // MARK: - 工具
    func withLock<T>(_ body: () -> T) -> T {
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

    private func notifyMain() {
        DispatchQueue.main.async { [weak self] in self?.onChange?() }
    }

    private func releaseAllAccess() {
        let all = withLock { () -> [[URL]] in
            let values = Array(accessCache.values)
            accessCache.removeAll()
            return values
        }
        all.flatMap { $0 }.forEach { $0.stopAccessingSecurityScopedResource() }
    }
    private static func parseFiles(_ any: Any?) -> [HostTransferFile] {
        guard let list = any as? [[String: Any]] else { return [] }
        return list.compactMap { dict in
            guard let path = dict["relative_path"] as? String else { return nil }
            return HostTransferFile(
                relativePath: path,
                size: u64(dict["size"]),
                mime: dict["mime"] as? String ?? "",
                isDir: dict["is_dir"] as? Bool ?? false
            )
        }
    }

    private static func u64(_ any: Any?) -> UInt64 {
        if let number = any as? NSNumber { return number.uint64Value }
        if let text = any as? String { return UInt64(text) ?? 0 }
        return 0
    }
    private static func dataDirectory() -> URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory,
                                            in: .userDomainMask).first
            ?? FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        let dir = base.appendingPathComponent("opi/transfer", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }

    private static func configJSON(deviceName: String) -> String {
        let object: [String: Any] = [
            "data_dir": dataDirectory().path,
            "tcp_port": 0,
            "discovery_port": 45888,
            "device_name": deviceName,
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: object),
              let json = String(data: data, encoding: .utf8) else { return "" }
        return json
    }
}
