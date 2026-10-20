// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// OpiEmojiPanel.swift —— 表情 / 颜文字 / 小欧贴纸面板。
//
// 数据与 Linux/Android 同一份仓库文件（打包脚本拷进 Resources）：
//   data/raw/emoji.tsv      表情<TAB>英文名<TAB>中文分组（分组按连续同名段聚合，12×6/页）
//   data/raw/kaomoji.tsv    颜文字<TAB>分类（6×6/页）
//   shared/pet/emoji-font/map.tsv  贴纸标签（mood/codepoint/file/label_zh/shortcode）
//   shared/pet/stickers/*.png、shared/pet/stickers-gif/*.gif
//
// 「贴纸」页由 ShowOwn 控制（Linux/Android 同名设置，默认关）。
// 贴纸**不直接上屏**：macOS 侧把 PNG + TIFF（以及 GIF 原文件）一起写进
// NSPasteboard，用户按 ⌘V 粘贴。IMK 只能插文本，插图片需要辅助功能注入，
// 本实现不走那条路（与 README 的「贴纸=剪贴板」说明一致）。
//
// 表情/颜文字点击**绕过引擎**直提（同符号面板：用户点的逗号就是要逗号）。

import AppKit

// 数据类型与 TSV 装载在 OpiEmojiData.swift。

final class OpiEmojiPanel: NSObject, NSSearchFieldDelegate {
    static let shared = OpiEmojiPanel()

    private enum Tab: Int {
        case emoji = 0
        case kaomoji
        case stickers
    }

    private static let panelW: CGFloat = 800
    private static let panelH: CGFloat = 440
    private static let gridY: CGFloat = 100
    private static let gridH: CGFloat = 276
    private static let emojiColumns = 12
    private static let kaomojiColumns = 6
    private static let stickerColumns = 4

    private var panel: NSPanel?
    private var root: OpiFlippedView?
    private var titleLabel: NSTextField?
    private var tabsHost: OpiFlippedView?
    private var categoriesHost: OpiFlippedView?
    private var gridHost: OpiFlippedView?
    private var searchField: NSSearchField?
    private var pageLabel: NSTextField?
    private var noticeLabel: NSTextField?
    private var prevButton: NSButton?
    private var nextButton: NSButton?
    private var tabButtons: [NSButton] = []
    private var categoryButtons: [NSButton] = []

    private var emojiGroups: [OpiEmojiCategory] = []
    private var kaomojiGroups: [OpiEmojiCategory] = []
    private var stickers: [OpiStickerItem] = []
    private var tab: Tab = .emoji
    private var category = 0
    private var page = 0
    private var query = ""
    private var searchItems: [OpiEmojiItem] = []
    private var searchStickers: [OpiStickerItem] = []
    private var pageTexts: [OpiEmojiItem] = []
    private var pageStickers: [OpiStickerItem] = []
    private var noticeTimer: Timer?

    private override init() {
        super.init()
        NotificationCenter.default.addObserver(
            self, selector: #selector(externalChanged(_:)),
            name: .opiThemeChanged, object: nil)
        NotificationCenter.default.addObserver(
            self, selector: #selector(externalChanged(_:)),
            name: .opiSettingsChanged, object: nil)
    }

    // ---------- 显示 ----------

    func show() {
        ensurePanel()
        reloadData()
        if tab == .stickers && !stickersVisible { tab = .emoji }
        query = ""
        searchField?.stringValue = ""
        searchItems = []
        searchStickers = []
        category = 0
        page = 0
        rebuildAll()
        panel?.orderFrontRegardless()
    }

    @objc private func externalChanged(_ note: Notification) {
        guard panel?.isVisible == true else { return }
        rebuildAll()
    }

    private var stickersVisible: Bool {
        OpiSettingsStore.showOwn && !stickers.isEmpty
    }

    private func ensurePanel() -> NSPanel {
        if let panel { return panel }
        let p = NSPanel(contentRect: NSRect(x: 0, y: 0, width: Self.panelW, height: Self.panelH),
                        styleMask: [.titled, .closable, .nonactivatingPanel],
                        backing: .buffered, defer: false)
        p.title = "表情与贴纸"
        p.isFloatingPanel = true
        p.level = .floating
        p.hidesOnDeactivate = false
        p.isReleasedWhenClosed = false
        p.becomesKeyOnlyIfNeeded = true
        p.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        p.center()

        let root = OpiFlippedView(frame: NSRect(x: 0, y: 0,
                                                width: Self.panelW, height: Self.panelH))
        root.autoresizingMask = [.width, .height]
        p.contentView = root
        self.root = root

        let title = NSTextField(labelWithString: "表情与贴纸")
        title.frame = NSRect(x: 14, y: 10, width: 170, height: 22)
        title.font = .boldSystemFont(ofSize: 16)
        root.addSubview(title)
        titleLabel = title

        let search = NSSearchField(frame: NSRect(x: Self.panelW - 234, y: 8,
                                                 width: 220, height: 26))
        search.placeholderString = "搜索表情 / 颜文字 / 贴纸"
        search.delegate = self
        search.autoresizingMask = [.minXMargin]
        root.addSubview(search)
        searchField = search

        let tabs = OpiFlippedView(frame: NSRect(x: 12, y: 38, width: 500, height: 26))
        root.addSubview(tabs)
        tabsHost = tabs

        let categories = OpiFlippedView(frame: NSRect(x: 12, y: 68, width: 776, height: 26))
        categories.autoresizingMask = [.width]
        root.addSubview(categories)
        categoriesHost = categories

        let grid = OpiFlippedView(frame: NSRect(x: 12, y: Self.gridY,
                                                width: 776, height: Self.gridH))
        grid.autoresizingMask = [.width, .height]
        root.addSubview(grid)
        gridHost = grid

        let notice = NSTextField(labelWithString: "")
        notice.frame = NSRect(x: 12, y: 382, width: 776, height: 16)
        notice.alignment = .center
        notice.font = .systemFont(ofSize: 11)
        notice.autoresizingMask = [.width, .minYMargin]
        root.addSubview(notice)
        noticeLabel = notice

        let prev = NSButton(title: "‹ 上一页", target: self, action: #selector(prevPage))
        prev.frame = NSRect(x: 12, y: 402, width: 90, height: 26)
        prev.bezelStyle = .rounded
        prev.autoresizingMask = [.minYMargin]
        root.addSubview(prev)
        prevButton = prev

        let label = NSTextField(labelWithString: "")
        label.frame = NSRect(x: 112, y: 406, width: 300, height: 20)
        label.font = .systemFont(ofSize: 12)
        label.autoresizingMask = [.minYMargin]
        root.addSubview(label)
        pageLabel = label

        let next = NSButton(title: "下一页 ›", target: self, action: #selector(nextPage))
        next.frame = NSRect(x: 422, y: 402, width: 90, height: 26)
        next.bezelStyle = .rounded
        next.autoresizingMask = [.minYMargin]
        root.addSubview(next)
        nextButton = next

        panel = p
        return p
    }

    // ---------- 数据 ----------

    private func reloadData() {
        emojiGroups = Self.loadEmoji()
        kaomojiGroups = Self.loadKaomoji()
        stickers = Self.loadStickers()
        if emojiGroups.isEmpty { NSLog("opi: 包里没有可用的 emoji.tsv —— 表情页为空") }
        if kaomojiGroups.isEmpty { NSLog("opi: 包里没有可用的 kaomoji.tsv —— 颜文字页为空") }
        if stickers.isEmpty { NSLog("opi: 包里没有贴纸/贴纸映射 —— 贴纸页不可用") }
    }

    // ---------- 重建 ----------

    private func rebuildAll() {
        let palette = OpiTheme.palette
        if let root { OpiTheme.style(content: root, palette: palette) }
        titleLabel?.textColor = palette.fg
        rebuildTabs(palette)
        rebuildCategories(palette)
        rebuildGrid(palette)
        updateNotice()
    }

    private func rebuildTabs(_ palette: OpiPalette) {
        tabButtons.forEach { $0.removeFromSuperview() }
        tabButtons = []
        guard let host = tabsHost else { return }
        var tabs: [(Tab, String)] = [(.emoji, "表情"), (.kaomoji, "颜文字")]
        if stickersVisible { tabs.append((.stickers, "贴纸")) }
        if !tabs.contains(where: { $0.0 == tab }) { tab = .emoji }
        for (i, pair) in tabs.enumerated() {
            let b = NSButton(title: pair.1, target: self, action: #selector(tabClicked(_:)))
            b.frame = NSRect(x: CGFloat(i) * 64, y: 0, width: 60, height: 24)
            b.bezelStyle = .rounded
            b.setButtonType(.pushOnPushOff)
            b.font = .systemFont(ofSize: 12)
            b.tag = pair.0.rawValue
            b.state = pair.0 == tab ? .on : .off
            host.addSubview(b)
            tabButtons.append(b)
        }
    }

    private var groups: [OpiEmojiCategory] {
        switch tab {
        case .emoji: return emojiGroups
        case .kaomoji: return kaomojiGroups
        case .stickers: return []
        }
    }

    private func rebuildCategories(_ palette: OpiPalette) {
        categoryButtons.forEach { $0.removeFromSuperview() }
        categoryButtons = []
        guard let host = categoriesHost else { return }
        guard tab != .stickers else { return }
        for (i, group) in groups.enumerated() {
            let b = NSButton(title: group.name, target: self, action: #selector(categoryClicked(_:)))
            let width = max(54, CGFloat(group.name.count) * 14 + 20)
            let x = categoryButtons.reduce(CGFloat(0)) { $0 + $1.frame.width + 6 }
            b.frame = NSRect(x: x, y: 0, width: width, height: 24)
            b.bezelStyle = .rounded
            b.setButtonType(.pushOnPushOff)
            b.font = .systemFont(ofSize: 12)
            b.tag = i
            b.state = (query.isEmpty && i == category) ? .on : .off
            host.addSubview(b)
            categoryButtons.append(b)
        }
    }

    private func rebuildGrid(_ palette: OpiPalette) {
        gridHost?.subviews.forEach { $0.removeFromSuperview() }
        pageTexts = []
        pageStickers = []
        guard let grid = gridHost else { return }

        if tab == .stickers {
            let items = query.isEmpty ? stickers : searchStickers
            pages(pageSize: 12, count: items.count)
            let start = page * 12
            let slice = start < items.count ? Array(items[start..<min(start + 12, items.count)]) : []
            pageStickers = slice
            let cellW = CGFloat(776 / Self.stickerColumns) - 4
            let cellH: CGFloat = 88
            for (i, item) in slice.enumerated() {
                let cell = OpiFlippedView(frame: NSRect(
                    x: CGFloat(i % Self.stickerColumns) * (cellW + 4),
                    y: CGFloat(i / Self.stickerColumns) * cellH,
                    width: cellW, height: cellH - 4))
                let button = NSButton(frame: NSRect(x: 0, y: 0, width: cellW, height: 60))
                if let url = Bundle.main.url(forResource: item.png, withExtension: nil,
                                             subdirectory: "pet/stickers") {
                    button.image = NSImage(contentsOf: url)
                }
                button.imagePosition = .imageOnly
                button.imageScaling = .scaleProportionallyDown
                button.isBordered = false
                button.tag = i
                button.target = self
                button.action = #selector(pickSticker(_:))
                cell.addSubview(button)
                let label = NSTextField(labelWithString: item.label)
                label.frame = NSRect(x: 0, y: 60, width: cellW, height: 22)
                label.alignment = .center
                label.font = .systemFont(ofSize: 11)
                label.textColor = palette.fg
                cell.addSubview(label)
                grid.addSubview(cell)
            }
            pageLabel?.stringValue = pagerText(total: items.count, pages: max(1, pages(pageSize: 12, count: items.count)))
            updatePager(count: items.count, pageSize: 12)
            return
        }

        let items = currentTextItems()
        let isKaomoji = tab == .kaomoji
        let columns = isKaomoji ? Self.kaomojiColumns : Self.emojiColumns
        let pageSize = isKaomoji ? 36 : 72
        pages(pageSize: pageSize, count: items.count)
        let start = page * pageSize
        let slice = start < items.count ? Array(items[start..<min(start + pageSize, items.count)]) : []
        pageTexts = slice
        if slice.isEmpty {
            let empty = NSTextField(labelWithString: query.isEmpty ? "（这一类没有内容）" : "（没有匹配项）")
            empty.frame = NSRect(x: 0, y: 0, width: 400, height: 24)
            empty.textColor = palette.dim
            grid.addSubview(empty)
        }
        let cellW = CGFloat(776) / CGFloat(columns)
        let cellH: CGFloat = isKaomoji ? 46 : 46
        for (i, item) in slice.enumerated() {
            let b = NSButton(title: item.text, target: self, action: #selector(pickText(_:)))
            b.frame = NSRect(x: CGFloat(i % columns) * cellW,
                             y: CGFloat(i / columns) * cellH,
                             width: cellW - 2, height: cellH - 2)
            b.bezelStyle = .regularSquare
            b.isBordered = false
            b.font = .systemFont(ofSize: isKaomoji ? 14 : 20)
            b.tag = i
            b.toolTip = tab == .emoji ? "\(item.name) · \(item.group)" : item.group
            grid.addSubview(b)
        }
        pageLabel?.stringValue = pagerText(total: items.count,
                                           pages: max(1, pages(pageSize: pageSize, count: items.count)))
        updatePager(count: items.count, pageSize: pageSize)
    }

    private func currentTextItems() -> [OpiEmojiItem] {
        if !query.isEmpty { return searchItems }
        guard groups.indices.contains(category) else { return [] }
        return groups[category].items
    }

    /// 只把 page 钳回合法范围；返回值给调用方复用（避免再算一次）。
    @discardableResult
    private func pages(pageSize: Int, count: Int) -> Int {
        let total = max(1, (count + pageSize - 1) / pageSize)
        page = min(max(0, page), total - 1)
        return total
    }

    private func pagerText(total: Int, pages: Int) -> String {
        "\(page + 1) / \(pages) · \(total) 个"
    }

    private func updatePager(count: Int, pageSize: Int) {
        let total = max(1, (count + pageSize - 1) / pageSize)
        prevButton?.isEnabled = page > 0
        nextButton?.isEnabled = page + 1 < total
    }

    // ---------- 动作 ----------

    @objc private func tabClicked(_ sender: NSButton) {
        guard let next = Tab(rawValue: sender.tag) else { return }
        tab = next
        category = 0
        page = 0
        query = ""
        searchField?.stringValue = ""
        searchItems = []
        searchStickers = []
        rebuildAll()
    }

    @objc private func categoryClicked(_ sender: NSButton) {
        guard groups.indices.contains(sender.tag) else { return }
        category = sender.tag
        page = 0
        query = ""
        searchField?.stringValue = ""
        searchItems = []
        rebuildAll()
    }

    @objc private func pickText(_ sender: NSButton) {
        guard pageTexts.indices.contains(sender.tag) else { return }
        OpiClientHub.shared.commit(pageTexts[sender.tag].text)
    }

    @objc private func pickSticker(_ sender: NSButton) {
        guard pageStickers.indices.contains(sender.tag) else { return }
        copySticker(pageStickers[sender.tag])
    }

    @objc private func prevPage() {
        guard page > 0 else { return }
        page -= 1
        rebuildGrid(OpiTheme.palette)
        rebuildCategories(OpiTheme.palette)
    }

    @objc private func nextPage() {
        page += 1
        rebuildGrid(OpiTheme.palette)
        rebuildCategories(OpiTheme.palette)
    }

    func controlTextDidChange(_ obj: Notification) {
        guard let field = obj.object as? NSSearchField, field === searchField else { return }
        query = field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        page = 0
        let needle = query.lowercased()
        if query.isEmpty {
            searchItems = []
            searchStickers = []
        } else if tab == .stickers {
            searchStickers = stickers.filter {
                $0.label.lowercased().contains(needle)
                    || $0.shortcode.lowercased().contains(needle)
            }
        } else {
            searchItems = groups.flatMap { $0.items }.filter {
                $0.text.contains(query)
                    || $0.name.lowercased().contains(needle)
                    || $0.group.lowercased().contains(needle)
            }
        }
        rebuildGrid(OpiTheme.palette)
        rebuildCategories(OpiTheme.palette)
    }

    // ---------- 贴纸剪贴板 ----------

    /// PNG+TIFF 放在同一 item（应用优先取 PNG），GIF 原文件单列一项。
    /// Linux/iOS 的贴纸语义是「PNG+GIF 都给」；macOS 侧不合成 ⌘V。
    private func copySticker(_ item: OpiStickerItem) {
        let pb = NSPasteboard.general
        pb.clearContents()
        var objects: [NSPasteboardItem] = []

        let pngURL = Bundle.main.url(forResource: item.png, withExtension: nil,
                                     subdirectory: "pet/stickers")
        let png = pngURL.flatMap { try? Data(contentsOf: $0) }
        let tiff = pngURL.flatMap { NSImage(contentsOf: $0)?.tiffRepresentation }
        if png != nil || tiff != nil {
            let board = NSPasteboardItem()
            if let png { board.setData(png, forType: .png) }
            if let tiff { board.setData(tiff, forType: .tiff) }
            objects.append(board)
        }
        let gifURL = Bundle.main.url(forResource: item.gif, withExtension: nil,
                                     subdirectory: "pet/stickers-gif")
        if let gifURL, let gif = try? Data(contentsOf: gifURL), !gif.isEmpty {
            let board = NSPasteboardItem()
            board.setData(gif, forType: NSPasteboard.PasteboardType("com.compuserve.gif"))
            objects.append(board)
        }
        guard pb.writeObjects(objects), !objects.isEmpty else {
            showNotice("贴纸资源缺失，复制失败")
            return
        }
        showNotice("已复制「\(item.label)」（PNG + GIF）—— 按 ⌘V 粘贴")
    }

    private func showNotice(_ text: String) {
        noticeLabel?.stringValue = text
        noticeLabel?.textColor = OpiTheme.palette.accent
        noticeTimer?.invalidate()
        noticeTimer = Timer.scheduledTimer(withTimeInterval: 4, repeats: false) { [weak self] _ in
            self?.noticeLabel?.stringValue = ""
        }
    }

    private func updateNotice() {
        noticeLabel?.textColor = OpiTheme.palette.accent
        pageLabel?.textColor = OpiTheme.palette.dim
    }
}
