// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// OpiSettingsWindow.swift —— 设置窗（macOS 的 Linux 设置窗等价物）。
//
// 左侧导航逐页对照 desktop/…/SettingsWindow.kt 的页签：常用 / 按键 / 符号与表情 /
// 词库 / 关于。**未实现的页不画空壳**：外观与状态栏在 Linux 上改的是 fcitx5 皮肤/
// 悬浮条，macOS 没有对应控制面（状态栏以输入法菜单代替）；五笔的 ABI 在，但随包
// 词库与设置页不在本轮范围 —— 差异都记在 README 的缺口表。
//
// 开关的**默认值不在这里**：初值一律读引擎（Rust 侧 `RankOptions::default()` 是
// 真源），只有用户改过才写 UserDefaults（键名沿用 Linux 配置键）。
// 词库页的导入是「复制进 `Application Support/opi/dicts/` + 立即导入」，重启后由
// `OpiEngine.loadImportedDicts` 重放；停用名单写 `DisabledDicts`（与 Linux 同名键）。

import AppKit
import UniformTypeIdentifiers

final class OpiSettingsWindow: NSObject {
    enum Page: Int, CaseIterable {
        case common, keys, symbols, dicts, about

        var title: String {
            switch self {
            case .common: return "常用"
            case .keys: return "按键"
            case .symbols: return "符号与表情"
            case .dicts: return "词库"
            case .about: return "关于"
            }
        }
    }

    static let shared = OpiSettingsWindow()

    private static let width: CGFloat = 760
    private static let height: CGFloat = 540
    private static let navW: CGFloat = 138

    private var panel: NSPanel?
    private var host: OpiFlippedView?
    private var navButtons: [NSButton] = []
    private var page: Page = .common
    private var dictEntries: [OpiDictEntry] = []

    private override init() { super.init() }

    func show(page: Page = .common) {
        let p = ensurePanel()
        self.page = page
        rebuildNav()
        rebuildPage()
        NSApp.activate(ignoringOtherApps: true)
        p.makeKeyAndOrderFront(nil)
    }

    // ---------- 建窗 ----------

    private func ensurePanel() -> NSPanel {
        if let panel = panel { return panel }
        let p = NSPanel(contentRect: NSRect(x: 0, y: 0, width: Self.width, height: Self.height),
                        styleMask: [.titled, .closable, .miniaturizable],
                        backing: .buffered, defer: false)
        p.title = "OPI 设置"
        p.isReleasedWhenClosed = false
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

        let content = OpiFlippedView(frame: NSRect(x: Self.navW, y: 12,
                                                   width: Self.width - Self.navW - 12,
                                                   height: Self.height - 24))
        content.autoresizingMask = [.width, .height]
        root.addSubview(content)
        host = content
        panel = p
        return p
    }

    private func rebuildNav() {
        for b in navButtons { b.state = (b.tag == page.rawValue) ? .on : .off }
    }

    private func rebuildPage() {
        host?.subviews.forEach { $0.removeFromSuperview() }
        switch page {
        case .common: buildCommon()
        case .keys: buildKeys()
        case .symbols: buildSymbols()
        case .dicts: buildDicts()
        case .about: buildAbout()
        }
    }

    // ---------- 常用 ----------

    private func buildCommon() {
        let engine = OpiEngine.shared
        label("常用", y: 8, size: 16, bold: true)
        label("输入偏好（改动立即生效；重启后从 UserDefaults 自动套用）",
              y: 32, size: 11, color: .secondaryLabelColor)

        _ = checkbox("学习（记住选词）", y: 60, on: engine.learnerEnabled(),
                     action: #selector(learnChanged(_:)))
        _ = checkbox("中文标点（。，；：“”……）", y: 88, on: engine.chinesePunctEnabled(),
                     action: #selector(punctChanged(_:)))
        _ = checkbox("模糊音", y: 116, on: engine.fuzzyEnabled(),
                     action: #selector(fuzzyMasterChanged(_:)))

        let names = engine.fuzzyPairNames()
        if names.isEmpty {
            label("（模糊音组名不可用：引擎未装载）", x: 24, y: 146, size: 11, color: .secondaryLabelColor)
        } else {
            let mask = Int(engine.fuzzyPairsMask())
            for (i, name) in names.enumerated() {
                let b = checkbox(name, x: 24 + CGFloat(i % 2) * 200, y: 144 + CGFloat(i / 2) * 26,
                                 on: mask & (1 << i) != 0,
                                 action: #selector(fuzzyPairChanged(_:)), tag: i)
                b.frame.size.width = 190
            }
        }

        let after = 144 + CGFloat(max(1, (names.count + 1) / 2)) * 26 + 8
        _ = checkbox("简拼", x: 24, y: after, on: engine.jianpinEnabled(),
                     action: #selector(jianpinChanged(_:)))
        _ = checkbox("拼音纠错", x: 240, y: after, on: engine.correctionEnabled(),
                     action: #selector(correctionChanged(_:)))

        let schemeY = after + 38
        label("拼音方式", x: 24, y: schemeY, w: 80, size: 12)
        let scheme = NSPopUpButton(frame: NSRect(x: 104, y: schemeY - 3, width: 120, height: 26),
                                   pullsDown: false)
        scheme.addItems(withTitles: OpiPinyinMethod.allCases.map { $0.title })
        scheme.selectItem(at: OpiPinyinMethod.from(scheme: engine.schemeCode()).rawValue)
        scheme.target = self
        scheme.action = #selector(schemeChanged(_:))
        host?.addSubview(scheme)

        label("双拼布局", x: 250, y: schemeY, w: 80, size: 12)
        let layout = NSPopUpButton(frame: NSRect(x: 330, y: schemeY - 3, width: 150, height: 26),
                                   pullsDown: false)
        layout.addItems(withTitles: OpiShuangpinLayout.names)
        layout.selectItem(at: min(max(0, engine.shuangpinLayout()), OpiShuangpinLayout.names.count - 1))
        layout.target = self
        layout.action = #selector(layoutChanged(_:))
        host?.addSubview(layout)

        label("模糊音逐对勾选会覆盖总开关；全不选 = 关闭。",
              x: 24, y: schemeY + 40, size: 11, color: .secondaryLabelColor)
    }

    // ---------- 按键 ----------

    private func buildKeys() {
        label("按键", y: 8, size: 16, bold: true)
        label("快捷键固定在 InputController.swift（与 Linux/Windows 同一张表；本页只读）",
              y: 32, size: 11, color: .secondaryLabelColor)
        let rows = [
            "⌃ '        英文 ⇄ 拼音",
            "⌃ \\        拼音 ⇄ 符号模式",
            "⇧ Space   全角 ⇄ 半角",
            "⌃ ⇧ F      简体 ⇄ 繁体",
            "⌃ ⇧ Z      打开符号面板",
        ]
        for (i, text) in rows.enumerated() {
            label(text, y: 68 + CGFloat(i) * 30, size: 13)
        }
        label("macOS 的输入法菜单（菜单栏输入源图标）提供模式/标点/全角/符号/设置的入口。",
              y: 68 + CGFloat(rows.count) * 30 + 10, size: 11, color: .secondaryLabelColor)
    }

    // ---------- 符号与表情 ----------

    private func buildSymbols() {
        label("符号与表情", y: 8, size: 16, bold: true)
        label(OpiSymbolPanel.catalogSummary(), y: 40)
        let open = NSButton(title: "打开符号面板…", target: self, action: #selector(openSymbols))
        open.frame = NSRect(x: 0, y: 72, width: 150, height: 30)
        open.bezelStyle = .rounded
        host?.addSubview(open)
        label("面板：22 类竖条 + 12×6 网格 + 搜索；点符号直接上屏且面板保持打开。",
              y: 116, size: 11, color: .secondaryLabelColor)
        label("表情页：引擎已有 `opi_emoji_symbols` / `opi_emoji_groups` 出口，",
              y: 152, size: 11, color: .secondaryLabelColor)
        label("macOS 面板尚未接入（缺口见 macos/README.md）。",
              y: 168, size: 11, color: .secondaryLabelColor)
    }

    // ---------- 词库 ----------

    private func buildDicts() {
        label("词库", y: 8, size: 16, bold: true)
        label("导入的 TSV 复制到 ~/Library/Application Support/opi/dicts/，重启时自动装载；",
              y: 32, size: 11, color: .secondaryLabelColor)
        label("停止启用只影响引擎内存层，原文件保留。系统词典（随包）不能启停。",
              y: 48, size: 11, color: .secondaryLabelColor)

        let importButton = NSButton(title: "导入词库…", target: self, action: #selector(importDict))
        importButton.frame = NSRect(x: 430, y: 4, width: 120, height: 28)
        importButton.bezelStyle = .rounded
        host?.addSubview(importButton)

        dictEntries = OpiEngine.shared.dictList()
        if dictEntries.isEmpty {
            label("没有可用的词库信息（引擎未装载？）", y: 82, size: 12, color: .secondaryLabelColor)
            return
        }
        // shortcut: 只列前 12 条（导入层通常个位数）；更多时换 NSScrollView。
        for (i, entry) in dictEntries.prefix(12).enumerated() {
            let y = 78 + CGFloat(i) * 34
            label("\(entry.name)  ·  \(entry.count) 条", x: 0, y: y + 4, w: 260, size: 12)
            if entry.system {
                // trad/stroke 没随包装载时 count=0/enabled=false —— 如实写「未装载」，
                // 别把它显示成「随包 0 条」。
                label(entry.enabled ? "随包" : "未装载", x: 280, y: y + 4, w: 60,
                      size: 11, color: .secondaryLabelColor)
            } else {
                let on = checkbox("启用", x: 280, y: y, on: entry.enabled,
                                  action: #selector(dictToggle(_:)), tag: i)
                on.frame.size.width = 70
                let remove = NSButton(title: "删除", target: self, action: #selector(dictRemove(_:)))
                remove.frame = NSRect(x: 370, y: y - 2, width: 70, height: 26)
                remove.bezelStyle = .rounded
                remove.tag = i
                host?.addSubview(remove)
            }
        }
        if dictEntries.count > 12 {
            label("……其余 \(dictEntries.count - 12) 条未显示", x: 0, y: 78 + 12 * 34,
                  size: 11, color: .secondaryLabelColor)
        }
    }

    // ---------- 关于 ----------

    private func buildAbout() {
        let version = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "未知"
        label("OPI 小欧输入法", y: 8, size: 20, bold: true)
        label("macOS · InputMethodKit", y: 40, size: 13)
        label("版本 \(version)", y: 66, size: 12, color: .secondaryLabelColor)
        label("草案构建：未签名、未公证、未在真机运行验证。", y: 106, size: 12, color: .systemOrange)
        label("与 Linux 的已知差异：表情页、皮肤、悬浮状态栏未接；", y: 146, size: 11, color: .secondaryLabelColor)
        label("状态栏入口以输入法菜单呈现。完整缺口表见 macos/README.md。", y: 162, size: 11, color: .secondaryLabelColor)
    }

    // ---------- 控件助手 ----------

    @discardableResult
    private func label(_ text: String, x: CGFloat = 0, y: CGFloat, w: CGFloat = 580,
                       size: CGFloat = 13, bold: Bool = false,
                       color: NSColor = .labelColor) -> NSTextField {
        let l = NSTextField(labelWithString: text)
        l.frame = NSRect(x: x, y: y, width: w, height: 20)
        l.font = bold ? .boldSystemFont(ofSize: size) : .systemFont(ofSize: size)
        l.textColor = color
        host?.addSubview(l)
        return l
    }

    @discardableResult
    private func checkbox(_ title: String, x: CGFloat = 0, y: CGFloat, on: Bool,
                          action: Selector, tag: Int = 0) -> NSButton {
        let b = NSButton(checkboxWithTitle: title, target: self, action: action)
        b.frame = NSRect(x: x, y: y, width: 360, height: 22)
        b.state = on ? .on : .off
        b.tag = tag
        host?.addSubview(b)
        return b
    }

    // ---------- 动作：常用 ----------

    @objc private func selectPage(_ sender: NSButton) {
        guard let p = Page(rawValue: sender.tag) else { return }
        page = p
        rebuildNav()
        rebuildPage()
    }

    @objc private func learnChanged(_ sender: NSButton) {
        let on = sender.state == .on
        OpiEngine.shared.setLearner(on)
        OpiSettingsStore.set("Learn", on)
    }

    @objc private func punctChanged(_ sender: NSButton) {
        let on = sender.state == .on
        OpiEngine.shared.setChinesePunct(on)
        OpiSettingsStore.set("ChinesePunct", on)
    }

    @objc private func fuzzyMasterChanged(_ sender: NSButton) {
        let on = sender.state == .on
        OpiEngine.shared.setFuzzy(on)
        OpiSettingsStore.set("Fuzzy", on)
        // 总开关优先：清掉逐对覆盖，否则下次启动逐对值会把总开关盖回去。
        OpiSettingsStore.remove("FuzzyPairs")
        rebuildPage()
    }

    @objc private func fuzzyPairChanged(_ sender: NSButton) {
        guard sender.tag >= 0, sender.tag < 32 else { return }
        let engine = OpiEngine.shared
        let names = engine.fuzzyPairNames()
        let mask = Int(engine.fuzzyPairsMask()) ^ (1 << sender.tag)
        let selected = names.enumerated().compactMap { mask & (1 << $0.offset) != 0 ? $0.element : nil }
        engine.setFuzzyPairs(selected)
        OpiSettingsStore.set("FuzzyPairs", selected.joined(separator: ","))
        OpiSettingsStore.set("Fuzzy", !selected.isEmpty)
        rebuildPage()
    }

    @objc private func jianpinChanged(_ sender: NSButton) {
        let on = sender.state == .on
        OpiEngine.shared.setJianpin(on)
        OpiSettingsStore.set("Jianpin", on)
    }

    @objc private func correctionChanged(_ sender: NSButton) {
        let on = sender.state == .on
        OpiEngine.shared.setCorrection(on)
        OpiSettingsStore.set("Correction", on)
    }

    @objc private func schemeChanged(_ sender: NSPopUpButton) {
        guard let method = OpiPinyinMethod(rawValue: sender.indexOfSelectedItem) else { return }
        OpiEngine.shared.setScheme(code: method.schemeCode)
        OpiSettingsStore.set("PinyinMethod", method.rawValue)
    }

    @objc private func layoutChanged(_ sender: NSPopUpButton) {
        let index = sender.indexOfSelectedItem
        guard OpiShuangpinLayout.names.indices.contains(index) else { return }
        OpiEngine.shared.setShuangpinLayout(index)
        OpiSettingsStore.set("ShuangpinLayout", index)
    }

    // ---------- 动作：符号 ----------

    @objc private func openSymbols() {
        OpiSymbolPanel.shared.show()
    }

    // ---------- 动作：词库 ----------

    @objc private func importDict() {
        let picker = NSOpenPanel()
        picker.canChooseFiles = true
        picker.canChooseDirectories = false
        picker.allowsMultipleSelection = false
        picker.title = "选择词库 TSV（词<TAB>拼音<TAB>频次）"
        if let tsv = UTType(filenameExtension: "tsv") { picker.allowedContentTypes = [tsv] }
        guard picker.runModal() == .OK, let src = picker.url else { return }

        let fm = FileManager.default
        let dir = OpiEngine.dictsDirectory
        do {
            try fm.createDirectory(at: dir, withIntermediateDirectories: true, attributes: nil)
        } catch {
            alert("建不了词库目录", info: error.localizedDescription)
            return
        }

        let stem = src.deletingPathExtension().lastPathComponent
        // 落盘名固定 `.tsv`：`loadImportedDicts` 按小写扩展名重放，删除也按
        // `<name>.tsv` 找文件 —— 保留原大小写会让「删除后重启又回来」。
        var dst = dir.appendingPathComponent("\(stem).tsv")
        var copied = false
        if src.standardizedFileURL.path != dst.standardizedFileURL.path {
            var n = 2
            while fm.fileExists(atPath: dst.path) {
                dst = dir.appendingPathComponent("\(stem)-\(n).tsv")
                n += 1
            }
            do {
                try fm.copyItem(at: src, to: dst)
                copied = true
            } catch {
                alert("导入失败（复制文件）", info: error.localizedDescription)
                return
            }
        }

        let count = OpiEngine.shared.importDict(path: dst.path)
        guard count >= 0 else {
            if copied { try? fm.removeItem(at: dst) }
            alert("导入失败", info: "文件读不到，或没有一行合法（UTF-8 TSV，三列：词<TAB>拼音<TAB>频次）。")
            return
        }
        rebuildPage()
        let name = dst.deletingPathExtension().lastPathComponent
        label("已导入 \(name)：\(count) 条", y: 78 + CGFloat(min(dictEntries.count, 12)) * 34 + 8,
              size: 12, color: .systemGreen)
    }

    @objc private func dictToggle(_ sender: NSButton) {
        guard dictEntries.indices.contains(sender.tag) else { return }
        let entry = dictEntries[sender.tag]
        // 名字含逗号写不进 Linux 同名的逗号分隔名单 —— 拒绝 + 说明，别静默失效。
        guard !entry.name.contains(",") else {
            alert("这个名字不能启停", info: "词库名含逗号，停用名单无法表达。")
            rebuildPage()
            return
        }
        guard OpiEngine.shared.setDictEnabled(entry.name, sender.state == .on) else {
            alert("引擎拒绝了启停", info: "系统词典或名字认不得：\(entry.name)")
            rebuildPage()
            return
        }
        persistDisabled()
        rebuildPage()
    }

    @objc private func dictRemove(_ sender: NSButton) {
        guard dictEntries.indices.contains(sender.tag) else { return }
        let entry = dictEntries[sender.tag]
        guard !entry.system else { return }
        let confirm = NSAlert()
        confirm.messageText = "删除词库「\(entry.name)」？"
        confirm.informativeText = "会同时删掉 \(OpiEngine.dictsDirectory.path)/\(entry.name).tsv，不可撤销。"
        confirm.addButton(withTitle: "删除")
        confirm.addButton(withTitle: "取消")
        guard confirm.runModal() == .alertFirstButtonReturn else { return }

        let file = OpiEngine.dictsDirectory.appendingPathComponent("\(entry.name).tsv")
        let fm = FileManager.default
        if fm.fileExists(atPath: file.path) {
            do {
                try fm.removeItem(at: file)
            } catch {
                alert("删不掉文件", info: error.localizedDescription)
                return
            }
        }
        if !OpiEngine.shared.removeDict(entry.name) {
            NSLog("opi: 引擎里没删掉词库 %@（文件已删，重启后不再装载）", entry.name)
        }
        persistDisabled()
        rebuildPage()
    }

    private func persistDisabled() {
        let disabled = OpiEngine.shared.dictList()
            .filter { !$0.system && !$0.enabled }
            .map { $0.name }
        OpiSettingsStore.setDisabledDicts(Set(disabled))
    }

    // ---------- 提示 ----------

    private func alert(_ message: String, info: String = "") {
        let a = NSAlert()
        a.messageText = message
        a.informativeText = info
        a.addButton(withTitle: "好")
        a.runModal()
    }
}
