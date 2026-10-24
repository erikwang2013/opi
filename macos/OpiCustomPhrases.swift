// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · macOS 上未运行 · 本机仅做 `swiftc -frontend -parse`
//
// OpiCustomPhrases.swift —— 自定义短语的**宿主真源 + 编解码**（2026-10-24）。
//
// 语义真源在 Rust：`crates/engine-core/src/custom_phrases.rs`（校验/匹配/上限）。
// 引擎只持内存、不收 `enabled` 字段；本文件负责两件事：
//   1. 落盘到 `~/Library/Application Support/opi/custom_phrases.json`（中文）/
//      `custom_phrases_en.json`（英文）（schema = 引擎 JSON + `"enabled"`）；
//   2. 把规范化后的 JSON（**不含 enabled**）经对应引擎出口整表推给引擎。
// 中英两套（2026-10-24）文件、开关、引擎表完全独立，编解码与校验共用。
//
// 失败语义：读不到/解析失败 → 空表 + 缺省启用（不删坏文件，下次保存原子覆盖）；
// 写盘失败 → 调用方拿错误、UI 不变；推引擎失败 → 宿主文件已是对的，只提示。
//
// 导入格式：JSON（对象或数组；条目可对象或二元数组）、TSV、CSV（可带引号）、
// 逐行 `字母=短语` / `字母 空格 短语`。导出另支持 CSV 与 `字母=短语`。
// ⚠️ 行式格式表达不了文本内换行（UI 是单行输入，JSON 才是无损格式）。

import Foundation

// ---------- 值 ----------

struct OpiCustomPhrase: Equatable {
    /// 归一后的小写 ASCII 字母（引擎按小写匹配）。
    let trigger: String
    let text: String
}

struct OpiCustomPhraseState: Equatable {
    /// 缺省**关**（2026-10-25 用户裁决）：与引擎 `CustomPhrases::default()` 一致。
    var enabled = false
    var phrases: [OpiCustomPhrase] = []
}

/// 中英两套（2026-10-24）：文件、开关、引擎表完全独立；同一套编解码/校验。
enum OpiCustomPhraseKind: Int, CaseIterable {
    case chinese, english

    /// 中文套沿用旧文件不改名（与 Linux/Windows 的同名约定一致）。
    var fileName: String {
        switch self {
        case .chinese: return "custom_phrases.json"
        case .english: return "custom_phrases_en.json"
        }
    }

    var displayPath: String { "~/Library/Application Support/opi/\(fileName)" }
}

enum OpiCustomPhraseDecode {
    /// `enabled` 只在宿主文件里出现；普通导入文件为 nil。
    case ok([OpiCustomPhrase], Bool?)
    case invalid(String)
}

// ---------- 编解码 ----------

enum OpiCustomPhraseCodec {

    static let version = 1
    /// 设置页每页条数。
    static let pageSize = 10
    /// 镜像引擎的 `MAX_CUSTOM_PHRASES` / `MAX_TRIGGER_LEN`（越限引擎整表拒收）。
    static let maxPhrases = 2000
    static let maxTriggerLength = 32
    /// 导入上限：手写短语表 1 MiB 已远超正常规模，超限在读入前挡掉。
    static let maxImportBytes = 1024 * 1024

    /// 非空、纯 ASCII 字母（大小写都收，保存时统一小写）。
    static func validTrigger(_ raw: String) -> Bool {
        !raw.isEmpty && raw.unicodeScalars.allSatisfy {
            (65...90).contains($0.value) || (97...122).contains($0.value)
        }
    }

    /// 引擎合同 JSON（**不含** enabled —— 引擎只认 version + phrases）。
    static func canonicalJSON(_ phrases: [OpiCustomPhrase]) -> String {
        json(enabled: nil, phrases: phrases)
    }

    /// 宿主文件 JSON（引擎形状 + enabled）。
    static func stateJSON(enabled: Bool, phrases: [OpiCustomPhrase]) -> String {
        json(enabled: enabled, phrases: phrases)
    }

    static func tsv(_ phrases: [OpiCustomPhrase]) -> String {
        lines(phrases) { "\($0.trigger)\t\($0.text)" }
    }

    static func equalsText(_ phrases: [OpiCustomPhrase]) -> String {
        lines(phrases) { "\($0.trigger)=\($0.text)" }
    }

    static func csv(_ phrases: [OpiCustomPhrase]) -> String {
        lines(phrases) { "\(csvField($0.trigger)),\(csvField($0.text))" }
    }

    private static func lines(_ phrases: [OpiCustomPhrase],
                              _ row: (OpiCustomPhrase) -> String) -> String {
        phrases.isEmpty ? "" : phrases.map(row).joined(separator: "\n") + "\n"
    }

    private static func csvField(_ s: String) -> String {
        if s.contains(where: { $0 == "," || $0 == "\"" || $0.isNewline }) {
            return "\"" + s.replacingOccurrences(of: "\"", with: "\"\"") + "\""
        }
        return s
    }

    private static func json(enabled: Bool?, phrases: [OpiCustomPhrase]) -> String {
        var root: [String: Any] = [
            "version": version,
            "phrases": phrases.map { ["trigger": $0.trigger, "text": $0.text] },
        ]
        if let enabled { root["enabled"] = enabled }
        guard let data = try? JSONSerialization.data(withJSONObject: root,
                                                     options: [.sortedKeys]),
              let text = String(data: data, encoding: .utf8) else {
            NSLog("opi: 自定义短语 JSON 序列化失败（不应发生）")
            return ""
        }
        return text
    }

    // ---------- 解码 ----------

    /// 自动识别格式。空文件 = 非法；JSON（首字符 `{`/`[`）优先，否则按行解析。
    static func decode(_ raw: String) -> OpiCustomPhraseDecode {
        var text = raw
        if text.hasPrefix("\u{FEFF}") { text.removeFirst() }
        text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if text.isEmpty { return .invalid("文件是空的") }
        if text.hasPrefix("{") || text.hasPrefix("[") { return decodeJSON(text) }
        return decodeLines(text)
    }

    private static func decodeJSON(_ text: String) -> OpiCustomPhraseDecode {
        guard let data = text.data(using: .utf8) else { return .invalid("文件不是 UTF-8") }
        let root: Any
        do {
            root = try JSONSerialization.jsonObject(with: data)
        } catch {
            return .invalid("JSON 格式错误：\(error.localizedDescription)")
        }
        var enabled: Bool?
        let entries: [Any]
        switch root {
        case let object as [String: Any]:
            if let v = object["version"],
               !((v as? NSNumber)?.doubleValue == Double(version)) {
                return .invalid("不支持的文件版本：\(v)")
            }
            guard let list = object["phrases"] else { return .invalid("缺少 phrases 字段") }
            guard let array = list as? [Any] else { return .invalid("phrases 必须是数组") }
            entries = array
            if let e = object["enabled"] as? Bool { enabled = e }
        case let array as [Any]:
            entries = array
        default:
            return .invalid("顶层必须是 JSON 对象或数组")
        }

        var out: [OpiCustomPhrase] = []
        for (index, entry) in entries.enumerated() {
            let label = "第 \(index + 1) 条："
            let trigger: Any?
            let text: Any?
            if let object = entry as? [String: Any] {
                trigger = object["trigger"]
                text = object["text"]
            } else if let pair = entry as? [Any], pair.count == 2 {
                trigger = pair[0]
                text = pair[1]
            } else {
                return .invalid("\(label)格式不对")
            }
            guard let t = trigger as? String, let x = text as? String else {
                return .invalid("\(label)字母和短语都必须是字符串")
            }
            do {
                out.append(try checked(t, x, label: label))
            } catch let error as OpiPhraseError {
                return .invalid(error.message)
            } catch {
                return .invalid("\(label)格式不对")
            }
        }
        let normalized = normalize(out)
        if normalized.count > maxPhrases {
            return .invalid("达到上限（最多 \(maxPhrases) 条）")
        }
        return .ok(normalized, enabled)
    }

    private static func decodeLines(_ text: String) -> OpiCustomPhraseDecode {
        var out: [OpiCustomPhrase] = []
        for (index, rawLine) in text.components(separatedBy: .newlines).enumerated() {
            let line = rawLine.trimmingCharacters(in: .whitespaces)
            if line.isEmpty { continue }
            do {
                out.append(try lineEntry(line))
            } catch let error as OpiPhraseError {
                return .invalid("第 \(index + 1) 行：\(error.message)")
            } catch {
                return .invalid("第 \(index + 1) 行：格式不对")
            }
        }
        if out.isEmpty { return .invalid("没有可导入的短语") }
        let normalized = normalize(out)
        if normalized.count > maxPhrases {
            return .invalid("达到上限（最多 \(maxPhrases) 条）")
        }
        return .ok(normalized, nil)
    }

    /// 行式：以引号开头或最早出现的分隔符是逗号 → 按 CSV；否则按 `字母<分隔符>短语`。
    private static func lineEntry(_ line: String) throws -> OpiCustomPhrase {
        if line.hasPrefix("\"") { return try csvEntry(line) }
        guard let at = line.firstIndex(where: {
            $0 == "\t" || $0 == "=" || $0 == "," || $0.isWhitespace
        }) else {
            throw OpiPhraseError("缺少分隔符（应为 字母=短语 / 字母 短语 / TSV / CSV）")
        }
        if line[at] == "," { return try csvEntry(line) }
        let trigger = String(line[..<at])
        let text = String(line[line.index(after: at)...])
        return try checked(trigger, text, label: "")
    }

    /// 单行 CSV：支持引号字段与 `""` 转义；必须恰好两列。
    private static func csvEntry(_ line: String) throws -> OpiCustomPhrase {
        var fields: [String] = []
        var field = ""
        var quoted = false
        var i = line.startIndex
        while i < line.endIndex {
            let c = line[i]
            if quoted {
                if c == "\"" {
                    let next = line.index(after: i)
                    if next < line.endIndex, line[next] == "\"" {
                        field.append("\"")
                        i = line.index(after: next)
                        continue
                    }
                    quoted = false
                    i = next
                    continue
                }
                field.append(c)
                i = line.index(after: i)
                continue
            }
            switch c {
            case "\"":
                if !field.isEmpty { throw OpiPhraseError("引号只能出现在字段开头") }
                quoted = true
            case ",":
                fields.append(field)
                field = ""
            default:
                field.append(c)
            }
            i = line.index(after: i)
        }
        if quoted { throw OpiPhraseError("引号未闭合") }
        fields.append(field)
        guard fields.count == 2 else { throw OpiPhraseError("应为两列（字母,短语）") }
        return try checked(fields[0], fields[1], label: "")
    }

    private struct OpiPhraseError: Error {
        let message: String
        init(_ message: String) { self.message = message }
    }

    private static func checked(_ trigger: String, _ rawText: String,
                                label: String) throws -> OpiCustomPhrase {
        let t = trigger.trimmingCharacters(in: .whitespaces)
        let x = rawText.trimmingCharacters(in: .whitespaces)
        if t.isEmpty { throw OpiPhraseError("\(label)字母不能为空") }
        if !validTrigger(t) { throw OpiPhraseError("\(label)字母只能是英文字母") }
        if t.count > maxTriggerLength {
            throw OpiPhraseError("\(label)字母过长（上限 \(maxTriggerLength) 个）")
        }
        if x.isEmpty { throw OpiPhraseError("\(label)短语不能为空") }
        return OpiCustomPhrase(trigger: t.lowercased(), text: x)
    }

    // ---------- 合并 / 分页 ----------

    /// 去重并规范化：trigger 小写，后出现者覆盖文本，位置取首次出现处。
    static func normalize(_ phrases: [OpiCustomPhrase]) -> [OpiCustomPhrase] {
        var position: [String: Int] = [:]
        var out: [OpiCustomPhrase] = []
        for p in phrases {
            let t = p.trigger.lowercased()
            let normalized = OpiCustomPhrase(trigger: t, text: p.text)
            if let at = position[t] {
                out[at] = normalized
            } else {
                position[t] = out.count
                out.append(normalized)
            }
        }
        return out
    }

    /// 后导入的 trigger 覆盖已有；文本一字不差的重复既不计新增也不计更新。
    static func merge(existing: [OpiCustomPhrase],
                      imported: [OpiCustomPhrase])
        -> (phrases: [OpiCustomPhrase], added: Int, updated: Int) {
        var out = normalize(existing)
        var index: [String: Int] = [:]
        for (i, p) in out.enumerated() { index[p.trigger] = i }
        var added = 0
        var updated = 0
        for p in normalize(imported) {
            if let at = index[p.trigger] {
                if out[at].text != p.text {
                    out[at] = p
                    updated += 1
                }
            } else {
                index[p.trigger] = out.count
                out.append(p)
                added += 1
            }
        }
        return (out, added, updated)
    }

    /// 空表也算 1 页（页面显示「第 1 / 1 页」而不是 0）。
    static func pageCount(_ total: Int) -> Int {
        total <= 0 ? 1 : (total + pageSize - 1) / pageSize
    }

    static func clampPage(_ page: Int, total: Int) -> Int {
        min(max(0, page), pageCount(total) - 1)
    }

    static func pageSlice(_ phrases: [OpiCustomPhrase], page: Int) -> [OpiCustomPhrase] {
        let p = clampPage(page, total: phrases.count)
        return Array(phrases.dropFirst(p * pageSize).prefix(pageSize))
    }
}

// ---------- 宿主文件 ----------

/// 进程级单例。设置窗与输入法在**同一个 IMK 进程**里（见 macos/README.md），
/// 所以保存后立即 `push` 即生效，不需要重启输入法；进程重启时 main.swift 重推。
final class OpiCustomPhraseStore {
    static let shared = OpiCustomPhraseStore()

    private init() {}

    /// 与词库同目录：`~/Library/Application Support/opi/`。
    func fileURL(_ kind: OpiCustomPhraseKind) -> URL {
        OpiEngine.dataDirectory.appendingPathComponent(kind.fileName)
    }

    /// 缺失/读失败/内容非法 → 缺省值（启用、空表），不删坏文件。
    func load(_ kind: OpiCustomPhraseKind) -> OpiCustomPhraseState {
        let url = fileURL(kind)
        guard let data = try? Data(contentsOf: url) else { return OpiCustomPhraseState() }
        guard let text = String(data: data, encoding: .utf8) else {
            NSLog("opi: 自定义短语文件不是 UTF-8：%@", url.path)
            return OpiCustomPhraseState()
        }
        switch OpiCustomPhraseCodec.decode(text) {
        case let .ok(phrases, enabled):
            return OpiCustomPhraseState(enabled: enabled ?? false, phrases: phrases)
        case let .invalid(reason):
            NSLog("opi: 自定义短语文件无效（按空表处理）：%@ —— %@", url.path, reason)
            return OpiCustomPhraseState()
        }
    }

    /// 原子写。返回 nil = 成功；否则是**面向用户**的原因，旧文件原样保留。
    func save(_ state: OpiCustomPhraseState, kind: OpiCustomPhraseKind) -> String? {
        let text = OpiCustomPhraseCodec.stateJSON(enabled: state.enabled,
                                                  phrases: state.phrases)
        guard !text.isEmpty else { return "序列化失败" }
        do {
            try FileManager.default.createDirectory(at: OpiEngine.dataDirectory,
                                                    withIntermediateDirectories: true)
            try Data(text.utf8).write(to: fileURL(kind), options: [.atomic])
            return nil
        } catch {
            return error.localizedDescription
        }
    }

    /// 整表推给引擎（启用态用读回确认写口真的收下了）。返回 nil = 成功。
    func push(_ state: OpiCustomPhraseState, kind: OpiCustomPhraseKind) -> String? {
        let json = OpiCustomPhraseCodec.canonicalJSON(state.phrases)
        var accepted = false
        switch kind {
        case .chinese:
            if OpiEngine.shared.setCustomPhrases(json: json) < 0 {
                return "引擎拒绝了短语表（未装载或校验不过）"
            }
            OpiEngine.shared.customPhrasesEnabled = state.enabled
            accepted = OpiEngine.shared.customPhrasesEnabled == state.enabled
        case .english:
            if OpiEngine.shared.setEnglishCustomPhrases(json: json) < 0 {
                return "引擎拒绝了短语表（未装载或校验不过）"
            }
            OpiEngine.shared.englishCustomPhrasesEnabled = state.enabled
            accepted = OpiEngine.shared.englishCustomPhrasesEnabled == state.enabled
        }
        return accepted ? nil : "引擎未接受启用状态"
    }

    /// 进程启动 / 打开设置页时整表重推。失败只记日志（不能因此崩输入法）。
    @discardableResult
    func loadAndPush(_ kind: OpiCustomPhraseKind) -> OpiCustomPhraseState {
        let state = load(kind)
        if let error = push(state, kind: kind) {
            NSLog("opi: %@自定义短语推引擎失败：%@",
                  kind == .chinese ? "中文" : "英文", error)
        }
        return state
    }
}
