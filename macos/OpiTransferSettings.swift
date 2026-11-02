// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 本机（Linux）没有 Swift 编译器，未编译；由 apple.yml 真编译。
//
// OpiTransferSettings.swift —— 设置窗「传输」页（macOS）。
// 发现/发送/进度全部来自 OpiClipService 快照；FFI 只在 OpiClipRunner 的线程上调用。
// 会话与信任表与剪贴板共用，但传输开关独立（默认关）。

import AppKit
import UniformTypeIdentifiers

extension OpiSettingsWindow {

    func buildTransfer() {
        let clip = OpiClipService.shared
        clip.onChange = { [weak self] in
            guard let self, self.page == .transfer else { return }
            self.rebuildPage()
        }

        let previousTarget = transferTargetPopup?.selectedItem?.representedObject as? String
        transferTargetPopup = nil
        setPageHeight(760)
        label("跨设备传输", y: 8, size: 18, bold: true)
        note("同一局域网内收发文件、文件夹、媒体、文本和纯文本剪贴板。"
             + "只发给已配对/已信任且在线设备；配对码要两端一致。默认关闭。",
             y: 34, height: 40)

        let state = clip.isTransferEnabled
            ? (clip.isRunning ? OpiText.t("运行中") : OpiText.t("启动中或失败"))
            : OpiText.t("已停止")
        label(OpiText.t("本机设备：") + "\(clip.deviceName) · \(state)", y: 82, size: 12,
              color: .secondaryLabelColor)
        var y: CGFloat = 106
        let error = clip.lastErrorText
        if !error.isEmpty {
            label(OpiText.t("错误：") + OpiText.t(error), y: y, size: 12, color: .systemRed)
            y += 24
        }

        checkbox("启用跨设备传输（默认关）", y: y, on: clip.isTransferEnabled,
                 action: #selector(transferEnableChanged(_:)))
        y += 36

        let pending = clip.pendingPairings
        if !pending.isEmpty {
            label("配对确认", y: y, size: 14, bold: true)
            y += 26
            for (index, item) in pending.enumerated() {
                let who = item.name.isEmpty ? OpiText.t("未命名设备") : item.name
                label(who + (item.incoming ? OpiText.t(" 请求配对") : OpiText.t(" 配对中")),
                      y: y, size: 13)
                label(item.sas, y: y + 18, size: 24, bold: true, color: .systemTeal)
                button("信任", x: 240, y: y + 14, w: 90,
                       action: #selector(clipPairAnswer(_:)), tag: index * 2 + 1)
                button("拒绝", x: 338, y: y + 14, w: 90,
                       action: #selector(clipPairAnswer(_:)), tag: index * 2)
                y += 58
            }
            note("请确认另一台设备显示相同的 6 位数字，再决定是否信任。", y: y, height: 20)
            y += 26
        }

        label("附近设备", y: y, size: 14, bold: true)
        y += 24
        let peers = clip.peers
        if peers.isEmpty {
            note("还没有发现设备。确认两台设备在同一局域网、都已开启传输开关；"
                 + "macOS 15+ 首次监听时会请求本地网络权限。", y: y, height: 34)
            y += 40
        } else {
            for peer in peers {
                let name = peer.name.isEmpty ? OpiText.t("未命名设备") : peer.name
                label(name, y: y, size: 13)
                let meta = "\(String(peer.id.prefix(8))) · "
                    + OpiText.t(peer.connected ? "在线" : "离线")
                    + " · " + OpiText.t(peer.trusted ? "已信任" : "未配对")
                label(meta, y: y + 18, size: 11, color: .secondaryLabelColor)
                let action = button(peer.trusted ? "撤销" : "配对",
                                    x: contentWidth - 110, y: y + 4, w: 100,
                                    action: #selector(clipPeerAction(_:)))
                action.toolTip = peer.id
                action.isEnabled = peer.trusted || peer.connected
                y += 44
            }
        }
        button("刷新设备", y: y + 2, w: 110, action: #selector(clipRefresh(_:)))
        y += 44

        label("发送给", y: y, size: 14, bold: true)
        y += 26
        let sendable = peers.filter { $0.trusted && $0.connected }
        var titles = sendable.map { peer -> String in
            let name = peer.name.isEmpty ? OpiText.t("未命名设备") : peer.name
            return name + " · " + OpiText.t("在线")
        }
        if titles.isEmpty { titles = [OpiText.t("没有已配对且在线的设备")] }
        let target = popup(titles, x: 0, y: y, w: 320, selected: 0,
                           action: #selector(transferTargetChanged(_:)))
        for (index, peer) in sendable.enumerated() {
            target.menu?.item(at: index)?.representedObject = peer.id
        }
        if let previousTarget, let index = sendable.firstIndex(where: { $0.id == previousTarget }) {
            target.selectItem(at: index)
        }
        target.isEnabled = !sendable.isEmpty
        transferTargetPopup = target
        y += 38

        let canSend = !sendable.isEmpty && clip.isTransferEnabled
        sendButton("选择文件…", y: y, x: 0, action: #selector(transferPickFiles(_:)),
                   enabled: canSend)
        sendButton("选择文件夹…", y: y, x: 130, action: #selector(transferPickFolder(_:)),
                   enabled: canSend)
        sendButton("选择媒体…", y: y, x: 260, action: #selector(transferPickMedia(_:)),
                   enabled: canSend)
        y += 34
        sendButton("发送文本…", y: y, x: 0, action: #selector(transferSendText(_:)),
                   enabled: clip.isTransferEnabled)
        sendButton("发送剪贴板", y: y, x: 130, w: 120,
                   action: #selector(transferSendClipboard(_:)), enabled: clip.isTransferEnabled)
        y += 42
        note("文本与剪贴板走内核的剪贴板通道，会发给**所有**已信任设备（没有定向文本接口）；"
             + "文件/文件夹使用传输通道，只发给上面选中的设备。",
             y: y, height: 34)
        y += 42

        label("进行中与最近传输", y: y, size: 14, bold: true)
        y += 24
        let transfers = clip.transfers
        if transfers.isEmpty {
            note("还没有传输。入站请求会先显示在这里，确认后才落盘。", y: y, height: 20)
            y += 28
        } else {
            for item in transfers {
                y = transferRow(item, y: y)
            }
            button("清除已结束", y: y, w: 110, action: #selector(transferClear(_:)))
            y += 38
        }

        label("说明", y: y, size: 14, bold: true)
        y += 24
        let notes = [
            "· 只与已配对/已信任设备通信；信任表在内核里，与剪贴板页共用。",
            "· 单次最多 128 个条目（含文件夹）、单文件 2 GiB、总计 8 GiB；选完即前置校验。",
            "· 接收的传输保存到 ~/Downloads/opi/<传输 ID 前 8 位>，完成后可点「打开」。",
            "· 传输中请保持设置窗所在进程运行；取消会同时通知对端。",
        ]
        for line in notes {
            note(line, y: y, height: 20)
            y += 20
        }
        setPageHeight(y + 16)
    }

    // MARK: - 控件

    @discardableResult
    private func sendButton(_ title: String, y: CGFloat, x: CGFloat, w: CGFloat = 120,
                            action: Selector, enabled: Bool) -> NSButton {
        let b = button(title, x: x, y: y, w: w, action: action)
        b.isEnabled = enabled
        return b
    }

    /// 一条传输：状态 + 进度条 + 取消/接收/拒绝/打开。
    private func transferRow(_ item: OpiTransfer, y: CGFloat) -> CGFloat {
        let who = item.peerName.isEmpty ? OpiText.t("未知设备") : item.peerName
        let direction = item.outgoing ? OpiText.t("发送 → ") : OpiText.t("接收 ← ")
        let color: NSColor = item.state == .failed ? .systemRed
            : item.state == .done ? .systemGreen : .labelColor
        label(direction + who + " · " + OpiText.t(item.state.title),
              y: y, w: contentWidth - 230, size: 13, color: color)

        if item.state == .active || item.state == .waiting {
            let bar = NSProgressIndicator(
                frame: NSRect(x: 0, y: y + 22, width: contentWidth - 230, height: 8))
            bar.style = .bar
            if item.totalBytes == 0 || item.state == .waiting {
                bar.isIndeterminate = true
                bar.startAnimation(nil)
            } else {
                bar.minValue = 0
                bar.maxValue = 1
                bar.doubleValue = item.progress
            }
            host?.addSubview(bar)
        }

        var detail: [String] = []
        if item.fileCount > 0 {
            if item.state == .active {
                detail.append("第 \(min(item.fileIndex + 1, item.fileCount))/\(item.fileCount) 项")
            } else {
                detail.append(OpiText.t("\(item.fileCount) 项"))
            }
        }
        if item.totalBytes > 0 {
            if item.state == .active {
                detail.append(OpiTransferLimits.bytes(item.transferredBytes) + " / "
                              + OpiTransferLimits.bytes(item.totalBytes))
            } else {
                detail.append(OpiTransferLimits.bytes(item.totalBytes))
            }
        }
        if !item.message.isEmpty { detail.append(OpiText.t(item.message)) }
        label(detail.joined(separator: " · "), y: y + 34, w: contentWidth - 230,
              size: 11, color: .secondaryLabelColor)

        if item.isIncomingOffer {
            let accept = button("接收", x: contentWidth - 110, y: y, w: 100,
                                action: #selector(transferAccept(_:)))
            accept.toolTip = item.id
            let reject = button("拒绝", x: contentWidth - 218, y: y, w: 100,
                                action: #selector(transferReject(_:)))
            reject.toolTip = item.id
        } else if item.state == .active || item.state == .waiting {
            let cancel = button("取消", x: contentWidth - 110, y: y, w: 100,
                                action: #selector(transferCancel(_:)))
            cancel.toolTip = item.id
        } else if item.state == .done, !item.outgoing, item.savedDir != nil {
            let open = button("打开", x: contentWidth - 110, y: y, w: 100,
                              action: #selector(transferOpen(_:)))
            open.toolTip = item.id
        }
        return y + 58
    }

    // MARK: - 动作

    @objc func transferEnableChanged(_ sender: NSButton) {
        OpiClipService.shared.setTransferEnabled(sender.state == .on)
        rebuildPage()
    }

    @objc func transferTargetChanged(_ sender: NSPopUpButton) {
        // 选择只影响下一次发送，不需要重画。
    }

    @objc func transferPickFiles(_ sender: Any?) {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = true
        guard panel.runModal() == .OK else { return }
        sendChosen(panel.urls)
    }

    @objc func transferPickFolder(_ sender: Any?) {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = true
        panel.resolvesAliases = true
        guard panel.runModal() == .OK else { return }
        sendChosen(panel.urls)
    }

    @objc func transferPickMedia(_ sender: Any?) {
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = true
        panel.allowedContentTypes = [.image, .movie, .audio]
        guard panel.runModal() == .OK else { return }
        sendChosen(panel.urls)
    }

    private func sendChosen(_ urls: [URL]) {
        guard !urls.isEmpty else { return }
        guard let peerId = transferTargetPopup?.selectedItem?.representedObject as? String else {
            alert("请先选择一台已配对且在线的设备")
            return
        }
        if let error = OpiClipService.shared.sendFiles(urls, to: peerId) {
            alert(error)
        } else {
            OpiStatusBar.shared.showNotice(OpiText.t("已提交传输"))
        }
        rebuildPage()
    }

    @objc func transferSendText(_ sender: Any?) {
        let box = NSAlert()
        box.messageText = OpiText.t("发送文本")
        box.informativeText = OpiText.t(
            "经已信任设备的剪贴板通道发送（最多 256 KiB），不会自动粘贴。")
        box.addButton(withTitle: OpiText.t("发送"))
        box.addButton(withTitle: OpiText.t("取消"))
        let scroll = NSScrollView(frame: NSRect(x: 0, y: 0, width: 420, height: 120))
        scroll.hasVerticalScroller = true
        scroll.borderType = .bezelBorder
        let text = NSTextView(frame: NSRect(x: 0, y: 0, width: 420, height: 120))
        text.isRichText = false
        text.font = .systemFont(ofSize: 13)
        scroll.documentView = text
        box.accessoryView = scroll
        box.window.initialFirstResponder = text
        guard box.runModal() == .alertFirstButtonReturn else { return }
        if let error = OpiClipService.shared.sendText(text.string) {
            alert(error)
        } else {
            OpiStatusBar.shared.showNotice(OpiText.t("已发送文本"))
        }
        rebuildPage()
    }

    @objc func transferSendClipboard(_ sender: Any?) {
        if let error = OpiClipService.shared.sendClipboardText() {
            alert(error)
        } else {
            OpiStatusBar.shared.showNotice(OpiText.t("已发送剪贴板文本"))
        }
        rebuildPage()
    }

    @objc func transferAccept(_ sender: NSButton) {
        guard let id = sender.toolTip else { return }
        OpiClipService.shared.respondTransfer(id: id, accept: true)
        rebuildPage()
    }

    @objc func transferReject(_ sender: NSButton) {
        guard let id = sender.toolTip else { return }
        OpiClipService.shared.respondTransfer(id: id, accept: false)
        rebuildPage()
    }

    @objc func transferCancel(_ sender: NSButton) {
        guard let id = sender.toolTip else { return }
        OpiClipService.shared.cancelTransfer(id: id)
        rebuildPage()
    }

    @objc func transferOpen(_ sender: NSButton) {
        guard let id = sender.toolTip,
              let item = OpiClipService.shared.transfers.first(where: { $0.id == id }) else {
            return
        }
        OpiClipService.shared.openSavedDir(for: item)
    }

    @objc func transferClear(_ sender: Any?) {
        OpiClipService.shared.clearFinishedTransfers()
        rebuildPage()
    }
}
