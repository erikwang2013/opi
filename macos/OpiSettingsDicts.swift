// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// OpiSettingsDicts.swift —— 设置窗的词库 / 五笔 / 关于页。

import AppKit
import UniformTypeIdentifiers

extension OpiSettingsWindow {

    // ---------- 词库 ----------

    func buildDicts() {
        setPageHeight(900)
        label("词库", y: 8, size: 18, bold: true)
        note("导入的 TSV 复制到 ~/Library/Application Support/opi/dicts/，重启时自动装载；停用只影响引擎内存层，原文件保留。随包系统词典（luna/trad/stroke）不能启停；wubi86 走单独的码表装载，不在本列表。",
             y: 34, height: 40)

        button("导入词库…", x: contentWidth - 130, y: 4, w: 120,
               action: #selector(importDict))

        dictEntries = OpiEngine.shared.dictList()
        var cursor: CGFloat = 86
        if dictEntries.isEmpty {
            label("没有可用的词库信息（引擎未装载？）", y: cursor, size: 12,
                  color: .secondaryLabelColor)
            cursor += 30
        } else {
            for (i, entry) in dictEntries.prefix(12).enumerated() {
                let y = 82 + CGFloat(i) * 34
                label("\(entry.name)  ·  \(entry.count) 条", x: 0, y: y + 4, w: 300, size: 12)
                if entry.system {
                    label(entry.enabled ? "随包" : "未装载", x: 310, y: y + 4, w: 70,
                          size: 11, color: .secondaryLabelColor)
                } else {
                    let on = checkbox("启用", x: 310, y: y, on: entry.enabled,
                                      action: #selector(dictToggle(_:)), tag: i)
                    on.frame.size.width = 70
                    button("删除", x: 400, y: y - 2, w: 70,
                           action: #selector(dictRemove(_:)), tag: i)
                }
            }
            if dictEntries.count > 12 {
                label("……其余 \(dictEntries.count - 12) 条未显示", y: 78 + 12 * 34,
                      size: 11, color: .secondaryLabelColor)
            }
            cursor = 82 + CGFloat(min(dictEntries.count, 12)) * 34 + 24
        }

        // ---- 用户词 ----
        label("用户词", x: 0, y: cursor, size: 14, bold: true)
        cursor += 30
        note("引擎的用户词表（学习选词的产物）。导出/导入用引擎原生 JSON，不在本层解析或改格式。",
             y: cursor, height: 34)
        cursor += 42
        button("导出用户词…", x: 0, y: cursor, w: 130, action: #selector(exportUserWords))
        button("导入用户词…", x: 140, y: cursor, w: 130, action: #selector(importUserWords))
        button("清空用户词…", x: 280, y: cursor, w: 130, action: #selector(clearUserWords))
        cursor += 42

        label("删除单个词", x: 0, y: cursor + 4, w: 90, size: 12)
        let field = NSTextField(frame: NSRect(x: 96, y: cursor, width: 220, height: 26))
        field.placeholderString = "输入要删除的词"
        host?.addSubview(field)
        userWordField = field
        button("删除", x: 326, y: cursor - 1, w: 70,
               action: #selector(removeUserWordClicked))
        note("只删除用户词表里的条目；系统词库/导入词库不受影响。",
             y: cursor + 34, height: 20)
    }

    @objc func importDict() {
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
            try fm.createDirectory(at: dir, withIntermediateDirectories: true,
                                   attributes: nil)
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
        alert("已导入 \(dst.deletingPathExtension().lastPathComponent)", info: "\(count) 条")
    }

    @objc func dictToggle(_ sender: NSButton) {
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

    @objc func dictRemove(_ sender: NSButton) {
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

    func persistDisabled() {
        let disabled = OpiEngine.shared.dictList()
            .filter { !$0.system && !$0.enabled }
            .map { $0.name }
        OpiSettingsStore.setDisabledDicts(Set(disabled))
    }

    // ---------- 用户词 ----------

    @objc func exportUserWords() {
        let json = OpiEngine.shared.exportUserWords()
        guard !json.isEmpty else {
            alert("没有可导出的用户词", info: "引擎返回空串（词库未装载或用户词表为空）。")
            return
        }
        let picker = NSSavePanel()
        picker.allowedContentTypes = [.json]
        picker.nameFieldStringValue = "opi-user-words.json"
        guard picker.runModal() == .OK, let url = picker.url else { return }
        do {
            try json.write(to: url, atomically: true, encoding: .utf8)
            alert("用户词已导出", info: url.path)
        } catch {
            alert("导出失败", info: error.localizedDescription)
        }
    }

    @objc func importUserWords() {
        let picker = NSOpenPanel()
        picker.canChooseFiles = true
        picker.canChooseDirectories = false
        picker.allowsMultipleSelection = false
        picker.allowedContentTypes = [.json]
        picker.title = "选择用户词 JSON"
        guard picker.runModal() == .OK, let url = picker.url else { return }
        do {
            let json = try String(contentsOf: url, encoding: .utf8)
            let count = OpiEngine.shared.importUserWords(json: json)
            guard count >= 0 else {
                alert("导入失败", info: "引擎拒绝：非法 JSON / 版本不符 / 词表过大 / 未装载。")
                return
            }
            OpiEngine.shared.markUserWordsDirty() // 导入结果也要进自动落盘
            alert("用户词已导入", info: "\(count) 条")
        } catch {
            alert("读不了文件", info: error.localizedDescription)
        }
    }

    @objc func clearUserWords() {
        let confirm = NSAlert()
        confirm.messageText = "清空用户词？"
        confirm.informativeText = "学习积累的用户词会全部删除，系统词库与导入词库不受影响。"
        confirm.addButton(withTitle: "清空")
        confirm.addButton(withTitle: "取消")
        guard confirm.runModal() == .alertFirstButtonReturn else { return }
        // 文件删除与内存清空必须串行（内部先取消待落盘），否则防抖保存会把旧词写回。
        if OpiEngine.clearUserWordsAndFile() {
            alert("已清空用户词")
        } else {
            alert("内存已清空，但用户词文件删不掉", info: "重启后旧词可能复活，请检查文件权限。")
        }
    }

    @objc func removeUserWordClicked() {
        let text = userWordField?.stringValue
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        guard !text.isEmpty else {
            alert("请输入要删除的词")
            return
        }
        OpiEngine.shared.removeUserWord(text)
        OpiEngine.shared.markUserWordsDirty() // 删除也要落盘
        userWordField?.stringValue = ""
        alert("已请求删除", info: "若用户词表里有「\(text)」，已移除。")
    }

    // ---------- 五笔 ----------

    func buildWubi() {
        setPageHeight(520)
        let engine = OpiEngine.shared
        let loaded = engine.wubiLoaded
        label("五笔", y: 8, size: 18, bold: true)
        if loaded {
            note("wubi86 码表已装载。以下旋钮与 Linux 同名同义，改动立即生效（越界由引擎钳制，不清缓冲）。",
                 y: 34, height: 34)
        } else {
            note("wubi86 码表未装载：五笔方案不会有候选。把 wubi86.opid 放进包的 Resources/dicts/ 或 ~/Library/Application Support/opi/ 后重启。下列值仍可先保存。",
                 y: 34, color: .systemOrange, height: 40)
        }

        let style = loaded ? engine.wubiStyle() : OpiSettingsStore.int("WubiStyle", default: 0)
        label("风格", x: 0, y: 92, w: 90, size: 12)
        popup(["纯五笔", "五笔拼音混输"], x: 96, y: 87, w: 170,
              selected: style, action: #selector(wubiStyleChanged(_:)))

        let mixed = loaded ? engine.wubiMixedEntry()
            : OpiSettingsStore.int("WubiMixedEntry", default: 0)
        label("混入方式", x: 300, y: 92, w: 80, size: 12)
        popup(["直接字母", "z 键"], x: 386, y: 87, w: 130,
              selected: mixed, action: #selector(wubiMixedChanged(_:)))

        let limit = loaded ? engine.wubiCandidateLimit()
            : max(3, min(9, OpiSettingsStore.int("WubiCandidateLimit", default: 5)))
        label("候选个数", x: 0, y: 136, w: 90, size: 12)
        popup((3...9).map { String($0) }, x: 96, y: 131, w: 90,
              selected: max(0, min(6, limit - 3)),
              action: #selector(wubiLimitChanged(_:)))

        let ret = loaded ? engine.wubiReturnStyle()
            : OpiSettingsStore.int("WubiReturn", default: 0)
        label("回车行为", x: 300, y: 136, w: 80, size: 12)
        popup(["清空缓冲", "上屏原文"], x: 386, y: 131, w: 130,
              selected: ret == 0 ? 0 : 1, action: #selector(wubiReturnChanged(_:)))

        let auto = loaded ? engine.wubiAutoCommit()
            : OpiSettingsStore.bool("WubiAutoCommit", default: true)
        checkbox("唯一候选自动上屏", y: 176, on: auto,
                 action: #selector(wubiAutoChanged(_:)))
        note("自动上屏默认开；关闭后唯一候选也停在候选栏。回车行为只对非空缓冲的五笔态生效。",
             y: 206, height: 34)

        button("切换到五笔", y: 254, w: 130, action: #selector(useWubi))
        note("五笔方案与拼音/笔画共用同一个引擎实例；切换方案会清空当前缓冲（引擎约定）。",
             y: 290, height: 20)
    }

    @objc func wubiStyleChanged(_ sender: NSPopUpButton) {
        let v = sender.indexOfSelectedItem == 1 ? 1 : 0
        OpiEngine.shared.setWubiStyle(v)
        OpiSettingsStore.set("WubiStyle", v)
    }

    @objc func wubiMixedChanged(_ sender: NSPopUpButton) {
        let v = sender.indexOfSelectedItem == 1 ? 1 : 0
        OpiEngine.shared.setWubiMixedEntry(v)
        OpiSettingsStore.set("WubiMixedEntry", v)
    }

    @objc func wubiLimitChanged(_ sender: NSPopUpButton) {
        let v = max(3, min(9, sender.indexOfSelectedItem + 3))
        OpiEngine.shared.setWubiCandidateLimit(v)
        OpiSettingsStore.set("WubiCandidateLimit", v)
    }

    @objc func wubiReturnChanged(_ sender: NSPopUpButton) {
        let v = sender.indexOfSelectedItem == 1 ? 1 : 0
        OpiEngine.shared.setWubiReturnStyle(v)
        OpiSettingsStore.set("WubiReturn", v)
    }

    @objc func wubiAutoChanged(_ sender: NSButton) {
        let on = sender.state == .on
        OpiEngine.shared.setWubiAutoCommit(on)
        OpiSettingsStore.set("WubiAutoCommit", on)
    }

    @objc func useWubi() {
        guard OpiEngine.shared.isSchemeAvailable(.wubi) else {
            alert("五笔码表未装载", info: "把 wubi86.opid 放进包的 Resources/dicts/ 或 ~/Library/Application Support/opi/。")
            return
        }
        OpiSettingsStore.applyScheme(.wubi)
        OpiClientHub.shared.refreshState()
        alert("已切换到五笔")
    }

    // ---------- 关于 ----------

    func buildAbout() {
        setPageHeight(610)
        aboutUpdateLabel = nil
        aboutDownloadButton = nil
        let version = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "未知"
        label("OPI 小欧输入法", y: 8, size: 20, bold: true)
        label("macOS · InputMethodKit", y: 40, size: 13)
        label("版本 \(version)", y: 66, size: 12, color: .secondaryLabelColor)
        // 窗口里的项目宠物（与状态栏 LOGO、菜单「关于」同一份资源）。
        if let url = Bundle.main.url(forResource: "idle", withExtension: "png",
                                     subdirectory: "pet/stickers"),
           let pet = NSImage(contentsOf: url) {
            let view = NSImageView(frame: NSRect(x: contentWidth - 76, y: 8,
                                                 width: 72, height: 72))
            view.image = pet
            view.imageScaling = .scaleProportionallyUpOrDown
            host?.addSubview(view)
        }

        // 更新检测（2026-10-22）：打开设置自动检测；这里只显示结论并可手动重测。
        aboutUpdateLabel = label(OpiUpdateChecker.shared.statusText, y: 88, size: 12,
                                 color: .secondaryLabelColor)
        button("检测更新", x: 0, y: 108, w: 96,
               action: #selector(checkUpdatesManually))
        let download = button("前往下载", x: 104, y: 108, w: 96,
                              action: #selector(openUpdateDownload))
        download.isHidden = OpiUpdateChecker.shared.downloadURL == nil
        aboutDownloadButton = download

        linkLabel("主页地址：https://opi.erik.xyz", url: "https://opi.erik.xyz", y: 150)
        linkLabel("项目地址：https://github.com/erikwang2013/opi",
                  url: "https://github.com/erikwang2013/opi", y: 172)
        linkLabel("博客地址：https://erik.xyz", url: "https://erik.xyz", y: 194)
        linkLabel("隐私政策：https://opi.erik.xyz/privacy/index.html",
                  url: "https://opi.erik.xyz/privacy/index.html", y: 216)

        label("草案构建：未签名、未公证、未在真机运行验证。", y: 254, size: 12,
              color: .systemOrange)
        note("Linux 对照：状态栏（LOGO/中英/标点/简繁/模式/全角/软键盘/符号/设置）、设置八页、12 皮肤、暗光/长辈、符号/表情/颜文字/贴纸、26/9/14/笔画自绘键盘。",
             y: 288, height: 40)
        note("明确不可用：ENPRO（englishpro 在 macOS 无控制面，状态栏按钮禁用）；系统候选窗皮肤/字号；空缓冲时软键盘退格不能删宿主文本。贴纸走剪贴板（PNG+GIF），不注入 ⌘V。",
             y: 338, height: 50)
        note("完整缺口表与「最不确定的 API」见 macos/README.md。", y: 396, height: 20)
        refreshAboutUpdate()
    }

    /// 检测结果回来后只改这一行和下载按钮；不在别的页重建控件。
    func refreshAboutUpdate() {
        guard page == .about else { return }
        aboutUpdateLabel?.stringValue = OpiUpdateChecker.shared.statusText
        aboutUpdateLabel?.textColor =
            OpiUpdateChecker.shared.downloadURL == nil ? .secondaryLabelColor : .systemOrange
        aboutDownloadButton?.isHidden = OpiUpdateChecker.shared.downloadURL == nil
    }

    @objc func checkUpdatesManually() {
        OpiUpdateChecker.shared.check(manual: true)
    }

    @objc func openUpdateDownload() {
        guard let url = OpiUpdateChecker.shared.downloadURL else { return }
        NSWorkspace.shared.open(url)
    }
}
