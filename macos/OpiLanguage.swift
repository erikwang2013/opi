// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 设置/关于页的中英语言层（2026-11-02）。只翻译设置窗里的页面，菜单与键面保持原样。
//
// 翻译表由 `shared/i18n/zh-en.tsv` 生成到 `Contents/Resources/en.lproj/Localizable.strings`
// （见 scripts/apple/package-macos.sh）；这里直接读 en 表做「精确匹配 → 最长前缀匹配」，
// 与 Android/桌面同一套语义：动态数值（版本、设备名）原样保留。

import Foundation

enum OpiText {
    static let languageKey = "OpiUiLanguage"

    private struct Pair {
        let zh: String
        let en: String
    }

    /// 进程启动时定一次：切换后当前会话保持原语言，重启才换（与弹窗承诺一致）。
    static let isEnglish: Bool = UserDefaults.standard.string(forKey: languageKey) == "en"

    private static let pairs: [Pair] = load()
    private static let exact: [String: String] = pairs.reduce(into: [:]) { $0[$1.zh] = $1.en }
    // 与 Android/桌面一致：单字键只做精确匹配，前缀按 UTF-16 长度从长到短。
    private static let prefixes: [Pair] = pairs
        .filter { $0.zh.utf16.count >= 2 }
        .sorted { $0.zh.utf16.count > $1.zh.utf16.count }

    static func t(_ text: String) -> String {
        guard isEnglish, !text.isEmpty else { return text }
        if let hit = exact[text] { return hit }
        for pair in prefixes where text.hasPrefix(pair.zh) {
            return pair.en + String(text.dropFirst(pair.zh.count))
        }
        return text
    }

    private static func load() -> [Pair] {
        guard let path = Bundle.main.path(forResource: "Localizable", ofType: "strings",
                                          inDirectory: "en.lproj"),
              let table = NSDictionary(contentsOfFile: path) as? [String: String]
        else { return [] }
        return table.map { Pair(zh: $0.key, en: $0.value) }
    }
}
