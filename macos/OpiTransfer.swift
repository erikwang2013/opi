// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 本机（Linux）没有 Swift 编译器，未编译；由 apple.yml 真编译。
//
// 跨设备传输（macOS）状态与动作。内核是与剪贴板共用的同一个 `OpiClipService`
// 节点：信任表、发现、加密会话都复用；传输开关单独落盘（`TransferEnabled`）。
// 本文件只做快照与调用编排，FFI 线程在 `OpiClipRunner`（shared/apple/）。

import AppKit
import Foundation

struct OpiTransferFile {
    let relativePath: String
    let size: UInt64
    let mime: String
    let isDir: Bool
}

enum OpiTransferState {
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

struct OpiTransfer {
    let id: String
    var peerId: String
    var peerName: String
    var outgoing: Bool
    var files: [OpiTransferFile]
    var totalBytes: UInt64
    var transferredBytes: UInt64
    var fileIndex: Int
    var fileCount: Int
    var state: OpiTransferState
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

extension OpiClipService {
    /// 接收目录：`~/Downloads/opi/<transfer-id 前 8 位>`（与 iOS 宿主同规则）。
    var transferSaveRoot: URL {
        let downloads = FileManager.default.urls(for: .downloadsDirectory, in: .userDomainMask)
            .first ?? FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Downloads", isDirectory: true)
        return downloads.appendingPathComponent("opi", isDirectory: true)
    }

    // MARK: - 发送

    /// nil = 已交给内核；否则是前置拒绝原因（未信任/离线/超限）。
    func sendFiles(_ urls: [URL], to peerId: String) -> String? {
        guard let peer = peers.first(where: { $0.id == peerId }) else { return "目标设备不存在" }
        guard peer.trusted else { return "只能发送给已配对/已信任设备" }
        guard peer.connected else { return "目标设备不在线" }
        guard isRunning else { return "传输服务未启动，请先打开传输开关" }
        // 安全作用域 URL 要先取得访问权再读大小（macOS 非沙盒通常返回 false，无需释放）。
        let scoped = urls.filter { $0.startAccessingSecurityScopedResource() }
        if let violation = OpiTransferLimits.violation(urls) {
            scoped.forEach { $0.stopAccessingSecurityScopedResource() }
            return violation
        }
        guard let json = OpiTransferLimits.filesJSON(paths: urls.map { $0.path }) else {
            scoped.forEach { $0.stopAccessingSecurityScopedResource() }
            return "无法生成文件清单"
        }
        withLock { errorText = "" }
        runner.sendFiles(peerId: peerId, filesJSON: json) { [weak self] id in
            guard let self else { return }
            if let id {
                self.withLock { self.transferAccessCache[id] = scoped }
            } else {
                scoped.forEach { $0.stopAccessingSecurityScopedResource() }
            }
            self.notifyMain()
        }
        return nil
    }

    /// 文本走内核的剪贴板通道：只发给已信任设备，没有定向文本接口。
    func sendText(_ text: String) -> String? {
        guard !text.isEmpty else { return "文本为空" }
        guard text.utf8.count <= 256 * 1024 else { return "文本超过 256 KiB 上限" }
        guard isRunning else { return "传输服务未启动，请先打开传输开关" }
        guard peers.contains(where: { $0.trusted }) else { return "没有已配对/已信任设备" }
        withLock { errorText = "" }
        runner.publish(text)
        return nil
    }

    func sendClipboardText() -> String? {
        guard let text = NSPasteboard.general.string(forType: .string), !text.isEmpty else {
            return "本机剪贴板没有纯文本"
        }
        return sendText(text)
    }

    // MARK: - 接收

    /// 接受时保存到 `~/Downloads/opi/<id 前 8 位>`；拒绝不需要目录。
    func respondTransfer(id: String, accept: Bool) {
        guard isRunning else { return }
        var dir = ""
        if accept {
            let url = transferSaveRoot.appendingPathComponent(
                String(id.prefix(8)), isDirectory: true)
            do {
                try FileManager.default.createDirectory(
                    at: url, withIntermediateDirectories: true)
                dir = url.path
            } catch {
                withLock { errorText = "无法创建接收目录：\(error.localizedDescription)" }
                notifyMain()
                return
            }
        }
        runner.respondTransfer(id: id, accept: accept, saveDir: dir) { [weak self] ok in
            guard let self else { return }
            if !ok {
                self.withLock { self.errorText = "传输应答失败（可能已超时）" }
                self.notifyMain()
            }
        }
    }

    func cancelTransfer(id: String) {
        guard isRunning else { return }
        runner.cancelTransfer(id: id) { [weak self] ok in
            guard let self else { return }
            if !ok {
                self.withLock { self.errorText = "取消失败（可能已结束）" }
                self.notifyMain()
            }
        }
    }

    func clearFinishedTransfers() {
        withLock {
            transferCache.removeAll { $0.state.isFinished }
            errorText = ""
        }
        onChange?()
    }

    func openSavedDir(for transfer: OpiTransfer) {
        guard let dir = transfer.savedDir, !dir.isEmpty else { return }
        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: dir)])
    }

    // MARK: - 事件（剪贴板线程）

    func handleTransferEvent(type: String, event: [String: Any]) {
        guard let id = event["transfer_id"] as? String, !id.isEmpty else { return }
        let peerId = event["instance_id"] as? String ?? ""
        let outgoing = event["outgoing"] as? Bool ?? false
        let name = peerName(peerId)
        let eventFiles = Self.parseTransferFiles(event["files"])
        let total = Self.u64(event["total_bytes"])
        let transferred = Self.u64(event["transferred_bytes"])
        let fileIndex = Int(Self.u64(event["file_index"]))
        let fileCount = Int(Self.u64(event["file_count"]))
        let savedDir = event["saved_dir"] as? String
        let reason = event["reason"] as? String ?? "传输失败"
        var terminal = false
        var incomingOffer = false

        withLock {
            let known = transferCache.firstIndex { $0.id == id }
            var item = known.map { transferCache[$0] }
                ?? OpiTransfer(id: id, peerId: peerId, peerName: name, outgoing: outgoing)
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
                incomingOffer = !outgoing
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
                transferCache[known] = item
            } else {
                transferCache.append(item)
            }
            // 只留最近 50 条，避免长时间开着设置窗时无限增长。
            if transferCache.count > 50 {
                transferCache.removeFirst(transferCache.count - 50)
            }
        }

        if terminal {
            let scoped = withLock { transferAccessCache.removeValue(forKey: id) }
            scoped?.forEach { $0.stopAccessingSecurityScopedResource() }
        }
        if incomingOffer || terminal {
            DispatchQueue.main.async { NSApp.requestUserAttention(.informationalRequest) }
        }
        notifyMain()
    }

    // MARK: - 解析

    private static func parseTransferFiles(_ any: Any?) -> [OpiTransferFile] {
        guard let list = any as? [[String: Any]] else { return [] }
        return list.compactMap { dict in
            guard let path = dict["relative_path"] as? String else { return nil }
            return OpiTransferFile(
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
}
