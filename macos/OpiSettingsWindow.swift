// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// OpiSettingsWindow.swift —— 设置窗外壳（Linux SettingsWindow.kt 的 macOS 等价物）。
//
// 页签顺序对齐 Linux：常用 / 按键 / 外观 / 状态栏 / 符号与表情 / 词库 / 五笔 / 关于。
// 页面构建在 OpiSettingsPages.swift 与 OpiSettingsDicts.swift（每文件 <500 行）。
//
// 开关的**默认值不在这里**：初值一律读引擎（Rust 侧 `RankOptions::default()` 是
// 真源），只有用户改过才写 UserDefaults（键名沿用 Linux 配置键）。
// 皮肤/暗光/长辈只作用于 OPI 自绘面板（状态栏、虚拟键盘、表情/贴纸）；
// 系统画的 IMKCandidates 候选窗不能套皮肤 —— 差异记在 README。
//
// 内容区可滚动：Linux 页更长，macOS 窗口固定 620pt，页面高度按需设置。

import AppKit
import UniformTypeIdentifiers

final class OpiSettingsWindow: NSObject {
    enum Page: Int, CaseIterable {
        case common, keys, appearance, statusbar, symbols, dicts, wubi, about

        var title: String {
            switch self {
            case .common: return "常用"
            case .keys: return "按键"
            case .appearance: return "外观"
            case .statusbar: return "状态栏"
            case .symbols: return "符号与表情"
            case .dicts: return "词库"
            case .wubi: return "五笔"
            case .about: return "关于"
            }
        }
    }

    static let shared = OpiSettingsWindow()
    static let width: CGFloat = 880
    static let height: CGFloat = 620
    static let navW: CGFloat = 144

    private(set) var panel: NSPanel?
    var host: OpiFlippedView?
    private var scroll: NSScrollView?
    private var navButtons: [NSButton] = []
    var page: Page = .common
    var dictEntries: [OpiDictEntry] = []
    var userWordField: NSTextField?
    // 「关于」页的更新检测控件（异步结果回来时就地改字，不重建整页）。
    var aboutUpdateLabel: NSTextField?
    var aboutDownloadButton: NSButton?

    var contentWidth: CGFloat { Self.width - Self.navW - 28 }

    private override init() { super.init() }

    func show(page: Page = .common) {
        let p = ensurePanel()
        self.page = page
        rebuildNav()
        rebuildPage()
        // 打开设置自动检测一次；自动失败保持上次结论（关于页的「检测更新」才显示失败）。
        OpiUpdateChecker.shared.onChange = { [weak self] in self?.refreshAboutUpdate() }
        OpiUpdateChecker.shared.check(manual: false)
        NSApp.activate(ignoringOtherApps: true)
        p.makeKeyAndOrderFront(nil)
    }

    // ---------- 建窗 ----------

    private func ensurePanel() -> NSPanel {
        if let panel { return panel }
        let p = NSPanel(contentRect: NSRect(x: 0, y: 0, width: Self.width, height: Self.height),
                        styleMask: [.titled, .closable, .miniaturizable, .resizable],
                        backing: .buffered, defer: false)
        p.title = "OPI 设置"
        p.isReleasedWhenClosed = false
        p.minSize = NSSize(width: 760, height: 480)
        p.center()

        let root = OpiFlippedView(frame: NSRect(x: 0, y: 0, width: Self.width, height: Self.height))
        root.autoresizingMask = [.width, .height]
        p.contentView = root

        for page in Page.allCases {
            let b = NSButton(title: page.title, target: self, action: #selector(selectPage(_:)))
            b.frame = NSRect(x: 12, y: 16 + CGFloat(page.rawValue) * 38,
                             width: Self.navW - 24, height: 30)
            b.bezelStyle = .rounded
            b.setButtonType(.pushOnPushOff)
            b.alignment = .left
            b.tag = page.rawValue
            root.addSubview(b)
            navButtons.append(b)
        }

        let scroll = NSScrollView(frame: NSRect(x: Self.navW, y: 12,
                                                width: Self.width - Self.navW - 12,
                                                height: Self.height - 24))
        scroll.autoresizingMask = [.width, .height]
        scroll.hasVerticalScroller = true
        scroll.autohidesScrollers = true
        scroll.drawsBackground = false
        scroll.borderType = .noBorder
        root.addSubview(scroll)
        self.scroll = scroll

        let content = OpiFlippedView(frame: NSRect(x: 0, y: 0,
                                                   width: Self.width - Self.navW - 28,
                                                   height: Self.height - 24))
        scroll.documentView = content
        host = content
        panel = p
        return p
    }

    private func rebuildNav() {
        for b in navButtons { b.state = (b.tag == page.rawValue) ? .on : .off }
    }

    func rebuildPage() {
        host?.subviews.forEach { $0.removeFromSuperview() }
        setPageHeight(Self.height)
        switch page {
        case .common: buildCommon()
        case .keys: buildKeys()
        case .appearance: buildAppearance()
        case .statusbar: buildStatusBar()
        case .symbols: buildSymbols()
        case .dicts: buildDicts()
        case .wubi: buildWubi()
        case .about: buildAbout()
        }
        if let scroll, let host {
            scroll.documentView = host
            scroll.contentView.scroll(to: NSPoint(x: 0, y: 0))
            scroll.reflectScrolledClipView(scroll.contentView)
        }
    }

    @objc private func selectPage(_ sender: NSButton) {
        guard let p = Page(rawValue: sender.tag) else { return }
        page = p
        rebuildNav()
        rebuildPage()
    }

    /// 页面构建器在开头调用：内容比窗口高时滚动，不裁字。
    func setPageHeight(_ height: CGFloat) {
        guard let host else { return }
        host.frame = NSRect(x: 0, y: 0, width: contentWidth, height: max(height, 200))
    }

    func refreshPageHeight(_ height: CGFloat) {
        host?.frame.size.height = max(height, 200)
    }

    // ---------- 通用控件 ----------

    @discardableResult
    func label(_ text: String, x: CGFloat = 0, y: CGFloat, w: CGFloat? = nil,
               size: CGFloat = 13, bold: Bool = false,
               color: NSColor = .labelColor) -> NSTextField {
        let l = NSTextField(labelWithString: text)
        l.frame = NSRect(x: x, y: y, width: w ?? contentWidth - x, height: 20)
        l.font = bold ? .boldSystemFont(ofSize: size) : .systemFont(ofSize: size)
        l.textColor = color
        l.lineBreakMode = .byTruncatingTail
        host?.addSubview(l)
        return l
    }

    /// 多行说明：按宽度自动换行（高度给足，超长仍会截断；本页文案都短）。
    @discardableResult
    func note(_ text: String, x: CGFloat = 0, y: CGFloat, w: CGFloat? = nil,
              size: CGFloat = 11, color: NSColor = .secondaryLabelColor,
              height: CGFloat = 34) -> NSTextField {
        let l = NSTextField(wrappingLabelWithString: text)
        l.frame = NSRect(x: x, y: y, width: w ?? contentWidth - x, height: height)
        l.font = .systemFont(ofSize: size)
        l.textColor = color
        host?.addSubview(l)
        return l
    }

    @discardableResult
    func checkbox(_ title: String, x: CGFloat = 0, y: CGFloat, on: Bool,
                  action: Selector, tag: Int = 0) -> NSButton {
        let b = NSButton(checkboxWithTitle: title, target: self, action: action)
        b.frame = NSRect(x: x, y: y, width: 380, height: 22)
        b.state = on ? .on : .off
        b.tag = tag
        host?.addSubview(b)
        return b
    }

    @discardableResult
    func button(_ title: String, x: CGFloat = 0, y: CGFloat, w: CGFloat = 130,
                action: Selector, tag: Int = 0) -> NSButton {
        let b = NSButton(title: title, target: self, action: action)
        b.frame = NSRect(x: x, y: y, width: w, height: 28)
        b.bezelStyle = .rounded
        b.tag = tag
        host?.addSubview(b)
        return b
    }

    @discardableResult
    func popup(_ titles: [String], x: CGFloat, y: CGFloat, w: CGFloat,
               selected: Int, action: Selector) -> NSPopUpButton {
        let p = NSPopUpButton(frame: NSRect(x: x, y: y, width: w, height: 26),
                              pullsDown: false)
        p.addItems(withTitles: titles)
        p.selectItem(at: min(max(0, selected), max(0, titles.count - 1)))
        p.target = self
        p.action = action
        host?.addSubview(p)
        return p
    }

    @discardableResult
    func linkLabel(_ title: String, url: String, y: CGFloat) -> NSButton {
        let b = NSButton(title: title, target: self, action: #selector(openAboutLink(_:)))
        b.frame = NSRect(x: 0, y: y, width: 420, height: 18)
        b.isBordered = false
        b.alignment = .left
        b.contentTintColor = .linkColor
        b.toolTip = url
        host?.addSubview(b)
        return b
    }

    @objc private func openAboutLink(_ sender: NSButton) {
        guard let raw = sender.toolTip, let url = URL(string: raw) else { return }
        NSWorkspace.shared.open(url)
    }

    func notifySettingsChanged() {
        NotificationCenter.default.post(name: .opiSettingsChanged, object: nil)
        OpiStatusBar.shared.refresh()
    }

    func alert(_ message: String, info: String = "") {
        let a = NSAlert()
        a.messageText = message
        a.informativeText = info
        a.addButton(withTitle: "好")
        a.runModal()
    }
}
