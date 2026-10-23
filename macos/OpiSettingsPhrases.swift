// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · macOS 上未运行 · 本机仅做 `swiftc -frontend -parse`
//
// OpiSettingsPhrases.swift —— 设置窗「高级」页：自定义短语（中文/英文两套，2026-10-24）。
//
// 两套在页内切换，文件、开关、引擎表完全独立。
// 数据流与 Android 设置页同一条：**先写宿主文件 → 再改 UI → 再推引擎**。
// 写盘失败 = 什么都没变；推引擎失败只提示（文件是唯一真源，进程重启会重推）。
// 设置窗与输入法在同一个 IMK 进程里，所以这里 push 后下一次按键就能看到短语，
// 不需要重启输入法。

import AppKit
import UniformTypeIdentifiers

extension OpiSettingsWindow {

    // ---------- 高级 ----------

    func buildAdvanced() {
        let kind = phraseKind
        // 打开/重建本页时从磁盘读一次并重推引擎（设置页即「重载」入口）。
        let state = OpiCustomPhraseStore.shared.loadAndPush(kind)
        phraseState = state
        let phrases = state.phrases
        let pageCount = OpiCustomPhraseCodec.pageCount(phrases.count)
        let page = OpiCustomPhraseCodec.clampPage(phrasePage, total: phrases.count)
        phrasePage = page

        label("高级", y: 8, size: 18, bold: true)
        let lang = kind == .chinese ? "中文" : "英文"
        let hint = kind == .chinese
            ? "中文套只在中文键盘（拼音/九键/十四键）匹配"
            : "英文套只在英文键盘匹配"
        note("自定义短语：输入完整字母后，短语排在候选最前，本页改动立即生效。\(hint)；文件在 \(kind.displayPath)。",
             y: 34, height: 34)
        label("自定义短语", y: 72, size: 14, bold: true)
        popup(["中文", "英文"], x: 120, y: 67, w: 110,
              selected: kind.rawValue,
              action: #selector(phraseKindChanged(_:)))

        checkbox("启用\(lang)自定义短语", y: 98, on: state.enabled,
                 action: #selector(phraseEnabledChanged(_:)))

        label("共 \(phrases.count) 条", x: 0, y: 128, w: 140, size: 13)
        if phrases.count >= OpiCustomPhraseCodec.maxPhrases {
            label("达到上限（最多 \(OpiCustomPhraseCodec.maxPhrases) 条）",
                  x: 150, y: 128, w: 300, size: 12, color: .systemOrange)
        }
        let add = button("添加", x: contentWidth - 70, y: 122, w: 70,
                         action: #selector(addPhrase))
        add.isEnabled = phrases.count < OpiCustomPhraseCodec.maxPhrases

        var cursor: CGFloat = 162
        if phrases.isEmpty {
            label("还没有短语。点右上角「添加」，或从下方导入。", y: cursor,
                  size: 12, color: .secondaryLabelColor)
            cursor += 36
        } else {
            for (offset, phrase) in OpiCustomPhraseCodec.pageSlice(phrases, page: page).enumerated() {
                let y = cursor + CGFloat(offset) * 30
                label(phrase.trigger, x: 0, y: y + 4, w: 100, size: 12, bold: true)
                let text = label(phrase.text, x: 110, y: y + 4,
                                 w: contentWidth - 110 - 170, size: 12)
                text.toolTip = phrase.text
                let row = page * OpiCustomPhraseCodec.pageSize + offset
                button("编辑", x: contentWidth - 158, y: y - 4, w: 72,
                       action: #selector(editPhrase(_:)), tag: row)
                button("删除", x: contentWidth - 80, y: y - 4, w: 72,
                       action: #selector(deletePhrase(_:)), tag: row)
            }
            cursor += CGFloat(min(phrases.count - page * OpiCustomPhraseCodec.pageSize,
                                  OpiCustomPhraseCodec.pageSize)) * 30 + 10
            let prev = button("‹ 上一页", x: 0, y: cursor, w: 90,
                              action: #selector(phrasePrevPage))
            prev.isEnabled = page > 0
            label("第 \(page + 1) / \(pageCount) 页", x: 100, y: cursor + 5,
                  w: 140, size: 12, color: .secondaryLabelColor)
            let next = button("下一页 ›", x: 250, y: cursor, w: 90,
                              action: #selector(phraseNextPage))
            next.isEnabled = page < pageCount - 1
            cursor += 46
        }

        label("导入与导出", y: cursor, size: 14, bold: true)
        cursor += 26
        note("导入支持 JSON / TSV / CSV / 字母=短语 / 字母 空格 短语；重复字母以新导入为准，任何一条非法则整份不写入。",
             y: cursor, height: 34)
        cursor += 42
        button("导入…", x: 0, y: cursor, w: 110, action: #selector(importPhrases))
        popup(["JSON", "TSV", "CSV", "字母=短语"], x: 140, y: cursor - 3, w: 120,
              selected: phraseExportFormat,
              action: #selector(phraseExportFormatChanged(_:)))
        button("导出…", x: 272, y: cursor, w: 110, action: #selector(exportPhrases))
        cursor += 40
        note("JSON 带 version 1，可原样再导入；行式格式不保留短语内的换行。",
             y: cursor, height: 20)

        setPageHeight(cursor + 40)
    }

    // ---------- 短语页动作 ----------

    @objc func phraseKindChanged(_ sender: NSPopUpButton) {
        phraseKind = OpiCustomPhraseKind(rawValue: sender.indexOfSelectedItem) ?? .chinese
        phrasePage = 0
        rebuildPage()
    }

    @objc func phraseEnabledChanged(_ sender: NSButton) {
        applyPhraseState(OpiCustomPhraseState(enabled: sender.state == .on,
                                              phrases: phraseState.phrases),
                         kind: phraseKind)
    }

    /// 一次变更的唯一落点：先存盘 → 改内存/UI → 推引擎。返回 nil = 已保存。
    @discardableResult
    private func applyPhraseState(_ state: OpiCustomPhraseState,
                                  kind: OpiCustomPhraseKind) -> String? {
        if let error = OpiCustomPhraseStore.shared.save(state, kind: kind) {
            alert("保存失败", info: error)
            rebuildPage()
            return error
        }
        phraseState = state
        phrasePage = OpiCustomPhraseCodec.clampPage(phrasePage, total: state.phrases.count)
        if let error = OpiCustomPhraseStore.shared.push(state, kind: kind) {
            alert("已保存，但引擎未接受", info: error)
        }
        rebuildPage()
        return nil
    }

    @objc func addPhrase() {
        guard phraseState.phrases.count < OpiCustomPhraseCodec.maxPhrases else {
            alert("达到上限（最多 \(OpiCustomPhraseCodec.maxPhrases) 条）")
            return
        }
        guard let phrase = promptPhrase(title: "添加短语", initial: nil,
                                        existing: phraseState.phrases) else { return }
        applyPhraseState(OpiCustomPhraseState(enabled: phraseState.enabled,
                                              phrases: phraseState.phrases + [phrase]),
                         kind: phraseKind)
    }

    @objc func editPhrase(_ sender: NSButton) {
        guard phraseState.phrases.indices.contains(sender.tag) else { return }
        var others = phraseState.phrases
        others.remove(at: sender.tag)
        guard let phrase = promptPhrase(title: "编辑短语",
                                        initial: phraseState.phrases[sender.tag],
                                        existing: others) else { return }
        var next = phraseState.phrases
        next[sender.tag] = phrase
        applyPhraseState(OpiCustomPhraseState(enabled: phraseState.enabled, phrases: next),
                         kind: phraseKind)
    }

    @objc func deletePhrase(_ sender: NSButton) {
        guard phraseState.phrases.indices.contains(sender.tag) else { return }
        let phrase = phraseState.phrases[sender.tag]
        let confirm = NSAlert()
        confirm.messageText = "删除短语「\(phrase.trigger)」？"
        confirm.informativeText = "只删除这一条，不可撤销。"
        confirm.addButton(withTitle: "删除")
        confirm.addButton(withTitle: "取消")
        guard confirm.runModal() == .alertFirstButtonReturn else { return }
        var next = phraseState.phrases
        next.remove(at: sender.tag)
        applyPhraseState(OpiCustomPhraseState(enabled: phraseState.enabled, phrases: next),
                         kind: phraseKind)
    }

    @objc func phrasePrevPage() {
        phrasePage = max(0, phrasePage - 1)
        rebuildPage()
    }

    @objc func phraseNextPage() {
        phrasePage += 1
        rebuildPage()
    }

    @objc func phraseExportFormatChanged(_ sender: NSPopUpButton) {
        phraseExportFormat = max(0, min(3, sender.indexOfSelectedItem))
    }

    /// 新增/编辑弹窗。校验与导入共用 Codec 的规则；重复字母（大小写不敏感）在这里挡。
    private func promptPhrase(title: String, initial: OpiCustomPhrase?,
                              existing: [OpiCustomPhrase]) -> OpiCustomPhrase? {
        var trigger = initial?.trigger ?? ""
        var text = initial?.text ?? ""
        while true {
            let alertPanel = NSAlert()
            alertPanel.messageText = title
            alertPanel.informativeText = "字母只能是英文字母（大小写不敏感）；输入完整字母后出现短语。"
            alertPanel.addButton(withTitle: "保存")
            alertPanel.addButton(withTitle: "取消")
            let box = NSView(frame: NSRect(x: 0, y: 0, width: 280, height: 56))
            let triggerField = NSTextField(frame: NSRect(x: 0, y: 30, width: 280, height: 24))
            triggerField.placeholderString = "字母（a–z，最多 \(OpiCustomPhraseCodec.maxTriggerLength) 个）"
            triggerField.stringValue = trigger
            let textField = NSTextField(frame: NSRect(x: 0, y: 0, width: 280, height: 24))
            textField.placeholderString = "短语"
            textField.stringValue = text
            box.addSubview(triggerField)
            box.addSubview(textField)
            alertPanel.accessoryView = box
            guard alertPanel.runModal() == .alertFirstButtonReturn else { return nil }

            trigger = triggerField.stringValue.trimmingCharacters(in: .whitespaces)
            text = textField.stringValue.trimmingCharacters(in: .whitespaces)
            if trigger.isEmpty {
                alert("字母不能为空")
            } else if !OpiCustomPhraseCodec.validTrigger(trigger) {
                alert("字母只能是英文字母（a–z）")
            } else if trigger.count > OpiCustomPhraseCodec.maxTriggerLength {
                alert("字母最多 \(OpiCustomPhraseCodec.maxTriggerLength) 个")
            } else if text.isEmpty {
                alert("短语不能为空")
            } else if existing.contains(where: { $0.trigger == trigger.lowercased() }) {
                alert("字母「\(trigger.lowercased())」已被其它短语使用")
            } else {
                return OpiCustomPhrase(trigger: trigger.lowercased(), text: text)
            }
        }
    }

    // ---------- 导入 / 导出 ----------

    @objc func importPhrases() {
        let picker = NSOpenPanel()
        picker.canChooseFiles = true
        picker.canChooseDirectories = false
        picker.allowsMultipleSelection = false
        picker.title = "选择短语文件（JSON / TSV / CSV / 字母=短语）"
        guard picker.runModal() == .OK, let url = picker.url else { return }

        let data: Data
        do {
            data = try Data(contentsOf: url)
        } catch {
            alert("读不了文件", info: error.localizedDescription)
            return
        }
        guard data.count <= OpiCustomPhraseCodec.maxImportBytes else {
            alert("导入失败", info: "文件超过 1 MB")
            return
        }
        guard let raw = String(data: data, encoding: .utf8) else {
            alert("导入失败", info: "文件不是 UTF-8 文本")
            return
        }

        switch OpiCustomPhraseCodec.decode(raw) {
        case let .invalid(reason):
            alert("导入失败", info: reason)
        case let .ok(imported, _):
            let merged = OpiCustomPhraseCodec.merge(existing: phraseState.phrases,
                                                    imported: imported)
            guard merged.phrases.count <= OpiCustomPhraseCodec.maxPhrases else {
                alert("导入失败",
                      info: "合并后达到上限（最多 \(OpiCustomPhraseCodec.maxPhrases) 条，当前 \(merged.phrases.count) 条）")
                return
            }
            let state = OpiCustomPhraseState(enabled: phraseState.enabled,
                                             phrases: merged.phrases)
            guard applyPhraseState(state, kind: phraseKind) == nil else { return }
            alert("导入完成", info: "新增 \(merged.added) 条，更新 \(merged.updated) 条")
        }
    }

    @objc func exportPhrases() {
        let format = max(0, min(3, phraseExportFormat))
        let extensions = ["json", "tsv", "csv", "txt"]
        let base = phraseKind == .chinese ? "opi-custom-phrases" : "opi-custom-phrases-en"
        let picker = NSSavePanel()
        picker.nameFieldStringValue = "\(base).\(extensions[format])"
        if let type = UTType(filenameExtension: extensions[format]) {
            picker.allowedContentTypes = [type]
        }
        guard picker.runModal() == .OK, let url = picker.url else { return }

        let text: String
        switch format {
        case 0: text = OpiCustomPhraseCodec.canonicalJSON(phraseState.phrases)
        case 1: text = OpiCustomPhraseCodec.tsv(phraseState.phrases)
        case 2: text = OpiCustomPhraseCodec.csv(phraseState.phrases)
        default: text = OpiCustomPhraseCodec.equalsText(phraseState.phrases)
        }
        do {
            try Data(text.utf8).write(to: url, options: [.atomic])
            alert("已导出", info: url.path)
        } catch {
            alert("导出失败", info: error.localizedDescription)
        }
    }
}
