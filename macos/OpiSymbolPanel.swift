// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// OpiSymbolPanel.swift —— 符号面板（macOS 的 Linux「符号」状态栏按钮等价物）。
//
// 布局逐项对齐 desktop/…/SymbolsPage.kt：左侧 **22 类**竖条（TSV 原序），右侧
// **12 列 × 6 行 = 72/页**网格，顶部搜索框，底部页码/翻页。数据真源是仓库里
// `crates/fcitx5-opi/data/symbol_categories.tsv`（打包脚本拷进 Resources；与
// Linux/Windows 同一份文件）。读不到时回退引擎的常用符号块（类目会少，打日志，
// 不假装 22 类都在）。
//
// 上屏**绕过引擎**：用户在面板上点的 `,` 就是要 `,`，不能再经中文标点映射
// （与 Android SymbolPanel / 桌面 onCommit 同一条纪律）。文本经 OpiClientHub
// 送进最近获得焦点的客户端（IMK `insertText`），面板保持打开以便连续点选。
//
// 字形过滤：与 iOS SymbolPanel.canRender 逐字同口径（CoreText 只问**单码位**
// 映射；多码位序列放行），避免本机字体没覆盖的码位画成空心方框。

import AppKit
import CoreText

// ---------- 数据 ----------

struct OpiSymbolCategory {
    let index: Int
    let label: String
    let symbols: [String]
}

/// 便于两个面板共用：AppKit 默认坐标系原点在左下，手工 frame 布局按「从上往下」
/// 排更接近读文档的顺序，故内容视图翻成左上原点。
final class OpiFlippedView: NSView {
    override var isFlipped: Bool { true }
}

// ---------- 面板 ----------

final class OpiSymbolPanel: NSObject, NSTextFieldDelegate {
    static let shared = OpiSymbolPanel()

    private static let pageSize = 72
    private static let columns = 12
    private static let rows = 6
    private static let cellW: CGFloat = 54
    private static let cellH: CGFloat = 46
    private static let panelW: CGFloat = 820
    private static let panelH: CGFloat = 430
    private static let sidebarW: CGFloat = 132

    private var panel: NSPanel?
    private var sidebarScroll: NSScrollView?
    private var gridHost: OpiFlippedView?
    private var pageLabel: NSTextField?
    private var prevButton: NSButton?
    private var nextButton: NSButton?
    private var searchField: NSSearchField?
    private var categoryButtons: [NSButton] = []
    private var categories: [OpiSymbolCategory] = []
    private var pageSymbols: [String] = []
    private var currentCategory = 0
    private var page = 0
    private var query = ""
    private var searchResults: [String] = []

    private override init() { super.init() }

    func show() {
        let panel = ensurePanel()
        // 每次打开都重读（iOS 侧的同一条纪律：空结果/旧数据不缓存）。
        categories = loadCategories()
        currentCategory = min(currentCategory, max(0, categories.count - 1))
        page = 0
        query = ""
        searchField?.stringValue = ""
        searchResults = []
        rebuild()
        panel.orderFrontRegardless()
    }

    // ---------- 建窗 ----------

    private func ensurePanel() -> NSPanel {
        if let panel = panel { return panel }
        let p = NSPanel(contentRect: NSRect(x: 0, y: 0, width: Self.panelW, height: Self.panelH),
                        styleMask: [.titled, .closable, .nonactivatingPanel],
                        backing: .buffered, defer: false)
        p.title = "符号与表情"
        p.isFloatingPanel = true
        p.level = .floating
        p.hidesOnDeactivate = false
        p.isReleasedWhenClosed = false
        // 默认不抢焦点：点符号时目标输入框仍是第一响应者；点搜索框才成为 key。
        p.becomesKeyOnlyIfNeeded = true
        p.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        p.center()

        let root = OpiFlippedView(frame: NSRect(x: 0, y: 0, width: Self.panelW, height: Self.panelH))
        root.autoresizingMask = [.width, .height]
        p.contentView = root

        let title = NSTextField(labelWithString: "符号与表情")
        title.frame = NSRect(x: 14, y: 10, width: 180, height: 22)
        title.font = .boldSystemFont(ofSize: 16)
        root.addSubview(title)

        let search = NSSearchField(frame: NSRect(x: Self.panelW - 234, y: 8, width: 220, height: 26))
        search.placeholderString = "搜索符号"
        search.delegate = self
        search.autoresizingMask = [.minXMargin]
        root.addSubview(search)
        searchField = search

        let scroll = NSScrollView(frame: NSRect(x: 10, y: 44, width: Self.sidebarW - 8,
                                                height: Self.panelH - 58))
        scroll.hasVerticalScroller = true
        scroll.autohidesScrollers = true
        scroll.drawsBackground = false
        scroll.borderType = .noBorder
        scroll.autoresizingMask = [.height]
        root.addSubview(scroll)
        sidebarScroll = scroll

        let column = OpiFlippedView(frame: NSRect(x: 0, y: 0, width: Self.sidebarW - 24, height: 10))
        scroll.documentView = column

        let gridX = Self.sidebarW + 12
        let grid = OpiFlippedView(frame: NSRect(x: gridX, y: 44,
                                                width: CGFloat(Self.columns) * Self.cellW,
                                                height: CGFloat(Self.rows) * Self.cellH))
        grid.autoresizingMask = [.height]
        root.addSubview(grid)
        gridHost = grid

        let prev = NSButton(title: "‹ 上一页", target: self, action: #selector(prevPage))
        prev.frame = NSRect(x: gridX, y: Self.panelH - 38, width: 90, height: 26)
        prev.bezelStyle = .rounded
        prev.autoresizingMask = [.minYMargin]
        root.addSubview(prev)
        prevButton = prev

        let label = NSTextField(labelWithString: "")
        label.frame = NSRect(x: gridX + 100, y: Self.panelH - 34, width: 240, height: 20)
        label.font = .systemFont(ofSize: 12)
        label.textColor = .secondaryLabelColor
        label.autoresizingMask = [.minYMargin]
        root.addSubview(label)
        pageLabel = label

        let next = NSButton(title: "下一页 ›", target: self, action: #selector(nextPage))
        next.frame = NSRect(x: gridX + 348, y: Self.panelH - 38, width: 90, height: 26)
        next.bezelStyle = .rounded
        next.autoresizingMask = [.minYMargin]
        root.addSubview(next)
        nextButton = next

        let hint = NSTextField(labelWithString: "点符号上屏 · 面板保持打开")
        hint.frame = NSRect(x: gridX + 452, y: Self.panelH - 34, width: 200, height: 20)
        hint.font = .systemFont(ofSize: 11)
        hint.textColor = .tertiaryLabelColor
        hint.autoresizingMask = [.minYMargin, .minXMargin]
        root.addSubview(hint)

        panel = p
        return p
    }

    // ---------- 渲染 ----------

    private func rebuild() {
        let symbols: [String]
        if query.isEmpty {
            symbols = categories.indices.contains(currentCategory) ? categories[currentCategory].symbols : []
        } else {
            symbols = searchResults
        }
        let pages = max(1, (symbols.count + Self.pageSize - 1) / Self.pageSize)
        page = min(max(0, page), pages - 1)
        let start = page * Self.pageSize
        let end = min(start + Self.pageSize, symbols.count)
        let slice = start < end ? Array(symbols[start..<end]) : []
        pageSymbols = slice

        gridHost?.subviews.forEach { $0.removeFromSuperview() }
        if slice.isEmpty {
            let empty = NSTextField(labelWithString: query.isEmpty ? "（这一类没有可显示的符号）" : "（没有匹配的符号）")
            empty.frame = NSRect(x: 0, y: 0, width: 400, height: 24)
            empty.textColor = .secondaryLabelColor
            gridHost?.addSubview(empty)
        } else {
            for (i, symbol) in slice.enumerated() {
                let b = NSButton(title: symbol, target: self, action: #selector(pick(_:)))
                b.frame = NSRect(x: CGFloat(i % Self.columns) * Self.cellW,
                                 y: CGFloat(i / Self.columns) * Self.cellH,
                                 width: Self.cellW - 2, height: Self.cellH - 2)
                b.bezelStyle = .regularSquare
                b.font = .systemFont(ofSize: 17)
                b.tag = i
                gridHost?.addSubview(b)
            }
        }

        pageLabel?.stringValue = "\(page + 1) / \(pages) · \(symbols.count) 个"
        prevButton?.isEnabled = page > 0
        nextButton?.isEnabled = page + 1 < pages
        rebuildSidebar()
    }

    private func rebuildSidebar() {
        categoryButtons.forEach { $0.removeFromSuperview() }
        categoryButtons = []
        guard let scroll = sidebarScroll, let column = scroll.documentView else { return }
        let width = column.frame.width
        let height = max(CGFloat(categories.count) * 30, scroll.contentSize.height)
        column.frame = NSRect(x: 0, y: 0, width: width, height: height)

        for (i, cat) in categories.enumerated() {
            let b = NSButton(title: "\(cat.index). \(cat.label)",
                             target: self, action: #selector(selectCategory(_:)))
            b.frame = NSRect(x: 0, y: CGFloat(i) * 30, width: width, height: 26)
            b.bezelStyle = .rounded
            b.setButtonType(.pushOnPushOff)
            b.alignment = .left
            b.font = .systemFont(ofSize: 12)
            b.tag = i
            b.state = (query.isEmpty && i == currentCategory) ? .on : .off
            column.addSubview(b)
            categoryButtons.append(b)
        }
    }

    // ---------- 动作 ----------

    @objc private func selectCategory(_ sender: NSButton) {
        guard categories.indices.contains(sender.tag) else { return }
        currentCategory = sender.tag
        page = 0
        query = ""
        searchResults = []
        searchField?.stringValue = ""
        rebuild()
    }

    @objc private func pick(_ sender: NSButton) {
        guard pageSymbols.indices.contains(sender.tag) else { return }
        OpiClientHub.shared.commit(pageSymbols[sender.tag])
    }

    @objc private func prevPage() {
        guard page > 0 else { return }
        page -= 1
        rebuild()
    }

    @objc private func nextPage() {
        page += 1
        rebuild()
    }

    func controlTextDidChange(_ obj: Notification) {
        guard let field = obj.object as? NSSearchField, field === searchField else { return }
        query = field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        page = 0
        if query.isEmpty {
            searchResults = []
        } else {
            // 空关键字会返回全部条目，故只在非空时搜（同 iOS/Android 的调用纪律）。
            searchResults = OpiEngine.shared.searchSymbols(keyword: query).filter(Self.canRender)
        }
        rebuild()
    }

    // ---------- 数据装载 ----------

    /// 随包类目表（`Resources/symbol_categories.tsv`，打包脚本拷入）。读不到/全坏 → 空。
    static func bundledCategories() -> [OpiSymbolCategory] {
        guard let url = Bundle.main.url(forResource: "symbol_categories", withExtension: "tsv"),
              let text = try? String(contentsOf: url, encoding: .utf8) else { return [] }
        return parse(text)
    }

    /// 设置页「符号与表情」显示用；不读引擎、不缓存。
    static func catalogSummary() -> String {
        let count = bundledCategories().count
        return count > 0
            ? "随包符号类目：\(count) 类 · 每页 12×6"
            : "未找到随包类目表 —— 面板会回退到引擎常用符号块（类目更少）"
    }

    private func loadCategories() -> [OpiSymbolCategory] {
        let parsed = Self.bundledCategories()
        if !parsed.isEmpty { return parsed }
        if Bundle.main.url(forResource: "symbol_categories", withExtension: "tsv") != nil {
            NSLog("opi: symbol_categories.tsv 解析出 0 类 —— 回退引擎常用符号块")
        } else {
            NSLog("opi: 包里没有 symbol_categories.tsv —— 回退引擎常用符号块（类目少于桌面 22）")
        }
        // 回退：引擎的 common 符号块（只有 6 块，标签/顺序都不等于 TSV，别混为一谈）。
        return OpiEngine.shared.symbolBlocks().enumerated().compactMap { i, block in
            guard let id = block.abiId else { return nil }
            let symbols = OpiEngine.shared.symbolsInBlock(id: id).filter(Self.canRender)
            return OpiSymbolCategory(index: i + 1,
                                     label: block.name.isEmpty ? "符号块 \(id)" : block.name,
                                     symbols: symbols)
        }
    }

    /// TSV：`序号<TAB>类目名<TAB>空格分隔的符号`。坏行跳过（同桌面 `SymbolCategories.parse`）。
    static func parse(_ text: String) -> [OpiSymbolCategory] {
        var out: [OpiSymbolCategory] = []
        for line in text.split(separator: "\n", omittingEmptySubsequences: false) {
            let parts = line.split(separator: "\t", omittingEmptySubsequences: false)
            guard parts.count >= 3, let index = Int(parts[0]) else { continue }
            let label = String(parts[1])
            let symbols = parts[2].split(separator: " ").map(String.init)
            guard !label.isEmpty, !symbols.isEmpty else { continue }
            out.append(OpiSymbolCategory(index: index, label: label,
                                         symbols: symbols.filter(Self.canRender)))
        }
        return out
    }

    /// 与 iOS `SymbolPanel.canRender` 逐字同口径：只判**单码位**；多码位序列
    /// （组合字符/ZWJ/旗帜）直接放行 —— 整串连字问字库会误判成没有。
    private static let probeFont = NSFont.systemFont(ofSize: 17)

    static func canRender(_ text: String) -> Bool {
        guard text.unicodeScalars.count == 1 else { return true }
        let ctFont = CTFontCreateWithName(probeFont.fontName as CFString, probeFont.pointSize, nil)
        var chars = Array(text.utf16)
        var glyphs = [CGGlyph](repeating: 0, count: chars.count)
        let ok = CTFontGetGlyphsForCharacters(ctFont, &chars, &glyphs, chars.count)
        return ok && glyphs.contains { $0 != 0 }
    }
}
