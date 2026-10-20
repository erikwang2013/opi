// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// OpiEmojiData.swift —— 表情/颜文字/贴纸的数据类型与 TSV 装载。
// 面板实现在 OpiEmojiPanel.swift；拆开是为了守住每文件 <500 行。
//
// 数据与 Linux/Android 同一份仓库文件（打包脚本拷进 Resources）：
//   data/raw/emoji.tsv      表情<TAB>英文名<TAB>中文分组（分组按连续同名段聚合）
//   data/raw/kaomoji.tsv    颜文字<TAB>分类
//   shared/pet/emoji-font/map.tsv  贴纸标签（mood/codepoint/file/label_zh/shortcode）

import AppKit

struct OpiEmojiItem {
    let text: String
    let name: String
    let group: String
}

struct OpiEmojiCategory {
    let name: String
    let items: [OpiEmojiItem]
}

struct OpiStickerItem {
    let label: String
    let shortcode: String
    let png: String
    let gif: String
}

extension OpiEmojiPanel {

    static func loadEmoji() -> [OpiEmojiCategory] {
        guard let url = Bundle.main.url(forResource: "emoji", withExtension: "tsv"),
              let text = try? String(contentsOf: url, encoding: .utf8) else { return [] }
        var out: [OpiEmojiCategory] = []
        var name = ""
        var items: [OpiEmojiItem] = []
        func flush() {
            if !name.isEmpty, !items.isEmpty { out.append(OpiEmojiCategory(name: name, items: items)) }
            items = []
        }
        for line in text.split(separator: "\n", omittingEmptySubsequences: true) {
            let cols = line.split(separator: "\t", omittingEmptySubsequences: false).map(String.init)
            guard cols.count >= 3, cols[0] != "emoji" else { continue }
            if cols[2] != name { flush(); name = cols[2] }
            items.append(OpiEmojiItem(text: cols[0], name: cols[1], group: cols[2]))
        }
        flush()
        return out
    }

    static func loadKaomoji() -> [OpiEmojiCategory] {
        guard let url = Bundle.main.url(forResource: "kaomoji", withExtension: "tsv"),
              let text = try? String(contentsOf: url, encoding: .utf8) else { return [] }
        var out: [OpiEmojiCategory] = []
        var name = ""
        var items: [OpiEmojiItem] = []
        func flush() {
            if !name.isEmpty, !items.isEmpty { out.append(OpiEmojiCategory(name: name, items: items)) }
            items = []
        }
        for line in text.split(separator: "\n", omittingEmptySubsequences: true) {
            let cols = line.split(separator: "\t", omittingEmptySubsequences: false).map(String.init)
            guard cols.count >= 2, cols[0] != "kaomoji" else { continue }
            if cols[1] != name { flush(); name = cols[1] }
            items.append(OpiEmojiItem(text: cols[0], name: cols[0], group: cols[1]))
        }
        flush()
        return out
    }

    static func loadStickers() -> [OpiStickerItem] {
        guard let url = Bundle.main.url(forResource: "pet-map", withExtension: "tsv"),
              let text = try? String(contentsOf: url, encoding: .utf8) else { return [] }
        var out: [OpiStickerItem] = []
        for line in text.split(separator: "\n", omittingEmptySubsequences: true) {
            let cols = line.split(separator: "\t", omittingEmptySubsequences: false).map(String.init)
            guard cols.count >= 5, cols[0] != "mood" else { continue }
            let stem = (cols[2] as NSString).deletingPathExtension
            out.append(OpiStickerItem(label: cols[3], shortcode: cols[4],
                                      png: cols[2], gif: "\(stem).gif"))
        }
        return out
    }

    /// 设置页「符号与表情」页的摘要（不读引擎、不缓存）。
    static func catalogSummary() -> String {
        let emoji = loadEmoji().reduce(0) { $0 + $1.items.count }
        let kaomoji = loadKaomoji().reduce(0) { $0 + $1.items.count }
        let stickers = loadStickers().count
        return "表情 \(emoji) 个 · 颜文字 \(kaomoji) 个 · 贴纸 \(stickers) 个（ShowOwn）"
    }
}
