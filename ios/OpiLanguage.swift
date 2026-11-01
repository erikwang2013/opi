// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 设置/关于页的中英语言层（2026-11-02）。只翻译这两类页面，键面与候选保持原样。
//
// 翻译表由 `shared/i18n/zh-en.tsv` 生成到 `en.lproj/Localizable.strings`
// （打包脚本同时放进宿主 App 与 appex）；这里直接读 en 表做「精确匹配 → 最长前缀
// 匹配」，与 Android/桌面同一套语义：动态数值（版本、MB、设备名）原样保留。
// 不用 NSLocalizedString 是因为前缀匹配（带动态数字的整句）需要整张表。

import UIKit

enum OpiText {
    private struct Pair {
        let zh: String
        let en: String
    }

    private static let pairs: [Pair] = load()
    private static let exact: [String: String] = pairs.reduce(into: [:]) { $0[$1.zh] = $1.en }
    // 与 Android/桌面一致：单字键只做精确匹配，前缀按 UTF-16 长度从长到短。
    private static let prefixes: [Pair] = pairs
        .filter { $0.zh.utf16.count >= 2 }
        .sorted { $0.zh.utf16.count > $1.zh.utf16.count }

    /// 进程启动时定一次：切换后当前会话保持原语言，重启才换（与弹窗承诺一致）。
    static let isEnglish: Bool = KeyboardPreferences.uiLanguage == "en"

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

/// 「界面语言」切换行。Apple 的 App 内语言要重启才生效：写 UserDefaults +
/// `AppleLanguages` 后弹窗说明，不做 bundle swizzle（不可靠也不该做）。
final class OpiLanguageRow: UIView {
    private let titleLabel = UILabel()
    private let zhButton = UIButton(type: .system)
    private let enButton = UIButton(type: .system)
    private var accent = KeyboardPalette.light.accent

    init() {
        super.init(frame: .zero)
        titleLabel.text = OpiText.t("界面语言")
        titleLabel.font = .systemFont(ofSize: 13)
        for (button, title) in [(zhButton, "中文"), (enButton, "English")] {
            button.setTitle(title, for: .normal)
            button.titleLabel?.font = .systemFont(ofSize: 13)
            button.contentEdgeInsets = UIEdgeInsets(top: 4, left: 8, bottom: 4, right: 8)
            button.layer.cornerRadius = 7
            button.backgroundColor = KeyboardPalette.light.keySurface(KeyboardPalette.light.keyBg)
        }
        zhButton.addAction(UIAction { [weak self] _ in self?.select("zh") }, for: .touchUpInside)
        enButton.addAction(UIAction { [weak self] _ in self?.select("en") }, for: .touchUpInside)

        let row = UIStackView(arrangedSubviews: [titleLabel, zhButton, enButton, UIView()])
        row.axis = .horizontal
        row.alignment = .center
        row.spacing = 8
        row.translatesAutoresizingMaskIntoConstraints = false
        addSubview(row)
        NSLayoutConstraint.activate([
            row.leadingAnchor.constraint(equalTo: leadingAnchor),
            row.trailingAnchor.constraint(equalTo: trailingAnchor),
            row.topAnchor.constraint(equalTo: topAnchor, constant: 2),
            row.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -2),
            row.heightAnchor.constraint(greaterThanOrEqualToConstant: 30),
        ])
        updateSelection()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    func applyTheme(_ p: KeyboardPalette) {
        accent = p.accent
        titleLabel.textColor = p.panelFg
        for button in [zhButton, enButton] {
            button.backgroundColor = p.keySurface(p.keyBg)
        }
        updateSelection()
    }

    private func select(_ code: String) {
        KeyboardPreferences.uiLanguage = code
        UserDefaults.standard.set([code == "en" ? "en" : "zh-Hans"], forKey: "AppleLanguages")
        updateSelection()
        let alert = UIAlertController(
            title: "重启 App 后生效 / Restart the app to apply",
            message: nil, preferredStyle: .alert
        )
        alert.addAction(UIAlertAction(title: OpiText.t("好"), style: .default))
        var host = window?.rootViewController
        while let presented = host?.presentedViewController { host = presented }
        host?.present(alert, animated: true)
    }

    private func updateSelection() {
        let english = KeyboardPreferences.uiLanguage == "en"
        for (button, selected) in [(zhButton, !english), (enButton, english)] {
            button.setTitleColor(selected ? accent : .secondaryLabel, for: .normal)
            button.titleLabel?.font = .systemFont(ofSize: 13, weight: selected ? .bold : .regular)
            button.layer.borderWidth = selected ? 2 : 1
            button.layer.borderColor = (selected
                ? accent
                : UIColor.secondaryLabel.withAlphaComponent(0.35)).cgColor
            button.accessibilityTraits = selected ? [.button, .selected] : .button
        }
    }
}
