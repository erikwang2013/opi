// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 本机（Linux）没有 Swift 编译器，未编译；由 apple.yml 真编译。
//
// OpiClipSettings.swift —— 设置窗「剪贴板」页（语义对照桌面 ClipPage.kt）。
// 只显示元数据：设备、在线/信任状态、待确认 SAS、最近来源、错误。
// **不显示剪贴板文本**；状态全部经 OpiClipService 的快照（不碰 FFI 句柄）。

import AppKit

extension OpiSettingsWindow {

    func buildClipboard() {
        let clip = OpiClipService.shared
        // 事件/后台状态变化时重画本页（别的页不动）。
        clip.onChange = { [weak self] in
            guard let self, self.page == .clip else { return }
            self.rebuildPage()
        }

        setPageHeight(720)
        label("跨设备剪贴板", y: 8, size: 18, bold: true)
        note("同一局域网内同步纯文本剪贴板：UDP 发现 + TCP 加密会话。默认关闭；"
             + "配对时两端必须显示同一个 6 位确认码。远端内容只写入剪贴板，不自动粘贴。",
             y: 34, height: 40)

        let state = clip.isEnabled ? (clip.isRunning ? "运行中" : "启动中或失败") : "已停止"
        label("本机设备：\(clip.deviceName) · \(state)", y: 82, size: 12,
              color: .secondaryLabelColor)

        var y: CGFloat = 106
        let error = clip.lastErrorText
        if !error.isEmpty {
            label("错误：\(error)", y: y, size: 12, color: .systemRed)
            y += 24
        }
        if let source = clip.lastSourceText {
            label("最近一次来源：\(source)（内容已写入本机剪贴板，未自动粘贴）",
                  y: y, size: 12, color: .systemTeal)
            y += 26
        }

        checkbox("启用跨设备剪贴板（默认关）", y: y, on: clip.isEnabled,
                 action: #selector(clipEnableChanged(_:)))
        y += 40

        let pending = clip.pendingPairings
        if !pending.isEmpty {
            label("配对确认", y: y, size: 14, bold: true)
            y += 26
            for (index, item) in pending.enumerated() {
                let who = item.name.isEmpty ? "未命名设备" : item.name
                label("\(who)\(item.incoming ? " 请求配对" : " 配对中")", y: y, size: 13)
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

        label("已发现设备", y: y, size: 14, bold: true)
        y += 26
        let peers = clip.peers
        if peers.isEmpty {
            note("还没有发现设备。确认两台设备在同一局域网、都已开启本开关；"
                 + "macOS 15+ 首次监听时会请求本地网络权限。", y: y, height: 34)
            y += 40
        } else {
            for peer in peers {
                let name = peer.name.isEmpty ? "未命名设备" : peer.name
                label(name, y: y, size: 13)
                let meta = "\(String(peer.id.prefix(8))) · \(peer.connected ? "在线" : "离线")"
                    + " · \(peer.trusted ? "已信任" : "未配对")"
                label(meta, y: y + 18, size: 11, color: .secondaryLabelColor)
                let action = button(peer.trusted ? "撤销" : "配对",
                                    x: contentWidth - 110, y: y + 4, w: 100,
                                    action: #selector(clipPeerAction(_:)))
                action.toolTip = peer.id // 列表可能刷新，按 ID 而不是下标操作
                action.isEnabled = peer.trusted || peer.connected
                y += 44
            }
        }
        button("刷新设备", y: y + 2, w: 110, action: #selector(clipRefresh(_:)))
        y += 44

        label("一键停止", y: y, size: 14, bold: true)
        y += 24
        note("关闭开关并释放监听端口。已配对设备仍保留信任，重新开启后可直接使用。",
             y: y, height: 20)
        button("停止共享", y: y + 24, w: 120, action: #selector(clipStopAll(_:)))
        y += 66

        label("安全与网络", y: y, size: 14, bold: true)
        y += 24
        let notes = [
            "· 默认关闭；开启后只在局域网内通信（UDP 发现 + TCP 加密会话）。",
            "· 配对必须由两端核对同一个 6 位数字；远端内容只写剪贴板，不自动粘贴。",
            "· 不记录剪贴板内容，不做剪贴板历史；状态文件只存设备身份与信任表。",
            "· 首次监听时系统会请求「本地网络」权限；拒绝后功能不可用，需到系统设置里放行。",
        ]
        for line in notes {
            note(line, y: y, height: 20)
            y += 20
        }
        setPageHeight(y + 16)
    }

    // MARK: - 动作

    @objc func clipEnableChanged(_ sender: NSButton) {
        OpiClipService.shared.setEnabled(sender.state == .on)
        rebuildPage()
    }

    /// tag = index * 2 + 1（信任）/ index * 2（拒绝）。
    @objc func clipPairAnswer(_ sender: NSButton) {
        OpiClipService.shared.respondPending(at: sender.tag / 2, accept: sender.tag % 2 == 1)
        rebuildPage()
    }

    @objc func clipPeerAction(_ sender: NSButton) {
        guard let id = sender.toolTip else { return }
        if OpiClipService.shared.peers.first(where: { $0.id == id })?.trusted == true {
            OpiClipService.shared.forget(id)
        } else {
            OpiClipService.shared.pair(id)
        }
        rebuildPage()
    }

    @objc func clipRefresh(_ sender: Any?) {
        OpiClipService.shared.refresh()
        rebuildPage()
    }

    @objc func clipStopAll(_ sender: NSButton) {
        OpiClipService.shared.stopAll()
        rebuildPage()
    }
}
