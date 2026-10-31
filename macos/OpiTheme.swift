// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// OpiTheme.swift —— macOS 自绘面板的皮肤/配色/状态栏几何。
//
// 12 款皮肤的名字与五个取色角色**逐值镜像** desktop/…/SkinPalette.kt（那份又是
// scripts/fcitx5_skins.py 的 Kotlin 镜像）。角色映射与 Linux 相同：
//   bg ← bg、fg ← text、hover ← hl_bg（带 alpha）、line ← sep、accent ← accent。
// 「不改动」= Linux 自绘条一直用的固定深色。macOS 没有 fcitx5 主题可写，所以
// 这套配色只作用于 **OPI 自绘的面板**（状态栏/符号/表情/虚拟键盘）；系统画的
// IMKCandidates 候选窗不受影响（README 的缺口表如实记着这条）。
//
// 「暗光 / 长辈」是 Android/Harmony 的本地显示模式（Linux 没有）：作为 OPI 面板的
// 额外覆盖层保留。长辈模式取 Android ElderPalette 的高对比色；暗光模式在皮肤为
// 「不改动」时用 Linux 的深色皮肤（选了别的皮肤则尊重皮肤）。
//
// 状态栏几何**逐值镜像** StatusBar.kt：BTN=30 / GAP=4 / PAD=6 / BAR_H=42，
// 统一缩放 75–150（默认 100）。不要在这里另存第二套尺寸。

import AppKit

// ---------- 通知 ----------

extension Notification.Name {
    /// 皮肤/暗光/长辈的任一改变。
    static let opiThemeChanged = Notification.Name("OpiThemeChanged")
    /// 状态栏按钮常驻/隐藏/缩放的改变。
    static let opiSettingsChanged = Notification.Name("OpiSettingsChanged")
    /// 引擎输入状态可能变了（模式/标点/全角/方案）。面板可借此重画。
    static let opiStateChanged = Notification.Name("OpiStateChanged")
}

// ---------- 皮肤 ----------

/// 顺序 = Linux `SkinOption` 枚举序；rawValue = Linux 配置 `Skin=` 的中文字面值。
/// 手改坏/旧版本写的值回「不改动」（同 Linux 的 `SkinOption.fromValue`）。
enum OpiSkin: String, CaseIterable {
    case keep = "不改动"
    case light = "浅色"
    case dark = "深色"
    case paper = "宣纸"
    case cinnabar = "朱砂"
    case fresh = "清爽"
    case azure = "湛蓝"
    case lemon = "柠檬"
    case sunset = "晚霞"
    case starlight = "星光"
    case wisteria = "紫藤"
    case pine = "松林"

    var title: String { rawValue }

    static func from(_ raw: String?) -> OpiSkin {
        guard let raw else { return .fresh }
        return OpiSkin(rawValue: raw) ?? .fresh
    }

    var palette: OpiPalette {
        switch self {
        case .keep:
            return OpiPalette(bg: rgb(0x2D2D2D), fg: rgb(0xE8E8E8),
                              hover: rgb(0x3F3F3F), line: rgb(0x4A4A4A),
                              accent: rgb(0xFF8A80))
        case .light:
            return OpiPalette(bg: rgb(0xFFFFFF), fg: rgb(0x1F1F1F),
                              hover: rgba(0xC0392B, 0x1F), line: rgb(0xD8D8D8),
                              accent: rgb(0xC0392B))
        case .dark:
            return OpiPalette(bg: rgb(0x1F1F1F), fg: rgb(0xE6E6E6),
                              hover: rgba(0xFF8A80, 0x26), line: rgb(0x4A4A4A),
                              accent: rgb(0xFF8A80))
        case .paper:
            return OpiPalette(bg: rgb(0xF5EFE1), fg: rgb(0x3D3222),
                              hover: rgba(0x8A5A2B, 0x1F), line: rgb(0xD8CDB4),
                              accent: rgb(0x8A5A2B))
        case .cinnabar:
            return OpiPalette(bg: rgb(0xB3272D), fg: rgb(0xFFF6EC),
                              hover: rgba(0xFFFFFF, 0x26), line: rgb(0xCC5A5F),
                              accent: rgb(0xFFD9A0))
        case .fresh:
            return OpiPalette(bg: rgb(0xE6F5EE), fg: rgb(0x16332B),
                              hover: rgba(0x2E7D6B, 0x26), line: rgb(0xB9D8CB),
                              accent: rgb(0x2E7D6B))
        case .azure:
            return OpiPalette(bg: rgb(0x3A679E), fg: rgb(0xF4F8FD),
                              hover: rgba(0xFFFFFF, 0x26), line: rgb(0x6E96C4),
                              accent: rgb(0xFFE3A8))
        case .lemon:
            return OpiPalette(bg: rgb(0xFBEE7E), fg: rgb(0x3A3000),
                              hover: rgba(0x7E6200, 0x26), line: rgb(0xD1C462),
                              accent: rgb(0x7E6200))
        case .sunset:
            return OpiPalette(bg: rgb(0xF7C79C), fg: rgb(0x4A2317),
                              hover: rgba(0xA63412, 0x26), line: rgb(0xD1A37F),
                              accent: rgb(0xA63412))
        case .starlight:
            return OpiPalette(bg: rgb(0x1B2138), fg: rgb(0xE8EBF7),
                              hover: rgba(0xFFD966, 0x33), line: rgb(0x3A4468),
                              accent: rgb(0xFFD966))
        case .wisteria:
            return OpiPalette(bg: rgb(0xEDE7FB), fg: rgb(0x3A2A63),
                              hover: rgba(0x6B4BC4, 0x26), line: rgb(0xCFC4EC),
                              accent: rgb(0x6B4BC4))
        case .pine:
            return OpiPalette(bg: rgb(0x1E3A2F), fg: rgb(0xDCEEE2),
                              hover: rgba(0x7FD3A2, 0x33), line: rgb(0x3C5C4C),
                              accent: rgb(0x7FD3A2))
        }
    }

    var next: OpiSkin {
        let all = OpiSkin.allCases
        return all[(all.firstIndex(of: self)! + 1) % all.count]
    }
}

private func rgb(_ hex: UInt32) -> NSColor {
    NSColor(srgbRed: CGFloat((hex >> 16) & 0xFF) / 255.0,
            green: CGFloat((hex >> 8) & 0xFF) / 255.0,
            blue: CGFloat(hex & 0xFF) / 255.0,
            alpha: 1.0)
}

/// `alpha` 是 0–255 的 **alpha 通道**（Linux 的 `hl_bg` 尾两位），不是整段 ARGB。
private func rgba(_ hex: UInt32, _ alpha: UInt32) -> NSColor {
    NSColor(srgbRed: CGFloat((hex >> 16) & 0xFF) / 255.0,
            green: CGFloat((hex >> 8) & 0xFF) / 255.0,
            blue: CGFloat(hex & 0xFF) / 255.0,
            alpha: CGFloat(alpha) / 255.0)
}

struct OpiPalette {
    let bg: NSColor
    let fg: NSColor
    let hover: NSColor
    let line: NSColor
    let accent: NSColor

    /// 次要文字（Linux 的 `BarDim` 同义：文字色降透明度）。
    var dim: NSColor { fg.withAlphaComponent(0.62) }
}

// ---------- 当前主题 ----------

enum OpiTheme {
    static let skinKey = "Skin"
    static let sceneKey = "Scene"
    static let darkKey = "DarkMode"
    static let elderKey = "ElderMode"

    static var skin: OpiSkin {
        get { OpiSkin.from(UserDefaults.standard.string(forKey: skinKey)) }
        set { UserDefaults.standard.set(newValue.rawValue, forKey: skinKey) }
    }

    /// 动态插画主题；空/未知 = 纯色皮肤（配色见 `skin`）。
    static var scene: OpiSceneTheme? {
        get { OpiSceneTheme(rawValue: UserDefaults.standard.string(forKey: sceneKey) ?? "") }
        set {
            if let newValue { UserDefaults.standard.set(newValue.rawValue, forKey: sceneKey) }
            else { UserDefaults.standard.removeObject(forKey: sceneKey) }
        }
    }

    /// 当前列表项：有动态主题时以它为准，否则是纯色皮肤。
    static var currentChoice: OpiSkinChoice {
        if let scene { return .scene(scene) }
        return .solid(skin)
    }

    static func select(_ choice: OpiSkinChoice) {
        switch choice {
        case .solid(let s):
            scene = nil
            skin = s
        case .scene(let t):
            skin = t.defaultSkin
            scene = t
        }
    }

    static var darkPanels: Bool {
        get { UserDefaults.standard.bool(forKey: darkKey) }
        set { UserDefaults.standard.set(newValue, forKey: darkKey) }
    }

    static var elderPanels: Bool {
        get { UserDefaults.standard.bool(forKey: elderKey) }
        set { UserDefaults.standard.set(newValue, forKey: elderKey) }
    }

    /// Android `ElderPalette` 的高对比色（只为 OPI 自绘面板）。
    static let elder = OpiPalette(bg: rgb(0xFFFFFF), fg: rgb(0x000000),
                                  hover: rgb(0xE0E0E0), line: rgb(0xBDBDBD),
                                  accent: rgb(0x263238))

    /// 当前面板配色。优先级：长辈 > 暗光（仅皮肤为「不改动」时）> 皮肤。
    static var palette: OpiPalette {
        if elderPanels { return elder }
        if darkPanels, skin == .keep { return OpiSkin.dark.palette }
        return skin.palette
    }

    static func cycleSkin() {
        let all = OpiSkinChoice.all
        let index = all.firstIndex(of: currentChoice) ?? -1
        select(all[(index + 1) % all.count])
        NotificationCenter.default.post(name: .opiThemeChanged, object: nil)
    }

    static func notifyChanged() {
        NotificationCenter.default.post(name: .opiThemeChanged, object: nil)
    }

    /// 统一给自绘面板涂底 + 1pt 边线（Linux 条的 border 同款）。
    static func style(content: NSView, palette: OpiPalette = OpiTheme.palette) {
        content.wantsLayer = true
        content.layer?.backgroundColor = palette.bg.cgColor
        content.layer?.borderColor = palette.line.cgColor
        content.layer?.borderWidth = 1
    }

    static func font(_ size: CGFloat, bold: Bool = false) -> NSFont {
        bold ? .boldSystemFont(ofSize: size) : .systemFont(ofSize: size)
    }

    static func attributed(_ text: String, color: NSColor, size: CGFloat,
                           bold: Bool = false, align: NSTextAlignment = .center) -> NSAttributedString {
        let style = NSMutableParagraphStyle()
        style.alignment = align
        return NSAttributedString(string: text, attributes: [
            .foregroundColor: color,
            .font: font(size, bold: bold),
            .paragraphStyle: style,
        ])
    }
}

// ---------- 状态栏设置（键名逐字沿用 Linux 配置） ----------

struct OpiBarSettings {
    var hidden = false
    var vk = false
    var symbols = true
    var trad = true
    var punct = true
    var lang = true
    var enpro = true
    var logo = true
    var fullwidth = true
    var settings = true
    var scale = OpiBarMetrics.defaultScale

    static func load(_ defaults: UserDefaults = .standard) -> OpiBarSettings {
        var s = OpiBarSettings()
        s.hidden = bool(defaults, "StatusBarHidden", false)
        s.vk = bool(defaults, "StatusBarVk", false)
        s.symbols = bool(defaults, "StatusBarSymbols", true)
        s.trad = bool(defaults, "StatusBarTrad", true)
        s.punct = bool(defaults, "StatusBarPunct", true)
        s.lang = bool(defaults, "StatusBarLang", true)
        s.enpro = bool(defaults, "StatusBarEnPro", true)
        s.logo = bool(defaults, "StatusBarLogo", true)
        s.fullwidth = bool(defaults, "StatusBarFullwidth", true)
        s.settings = bool(defaults, "StatusBarSettings", true)
        s.scale = OpiBarMetrics.clamp(defaults.object(forKey: "StatusBarScale") as? Int
                                      ?? OpiBarMetrics.defaultScale)
        return s
    }

    func save(_ defaults: UserDefaults = .standard) {
        defaults.set(hidden, forKey: "StatusBarHidden")
        defaults.set(vk, forKey: "StatusBarVk")
        defaults.set(symbols, forKey: "StatusBarSymbols")
        defaults.set(trad, forKey: "StatusBarTrad")
        defaults.set(punct, forKey: "StatusBarPunct")
        defaults.set(lang, forKey: "StatusBarLang")
        defaults.set(enpro, forKey: "StatusBarEnPro")
        defaults.set(logo, forKey: "StatusBarLogo")
        defaults.set(fullwidth, forKey: "StatusBarFullwidth")
        defaults.set(settings, forKey: "StatusBarSettings")
        defaults.set(scale, forKey: "StatusBarScale")
    }

    private static func bool(_ d: UserDefaults, _ key: String, _ fallback: Bool) -> Bool {
        guard d.object(forKey: key) != nil else { return fallback }
        return d.bool(forKey: key)
    }
}

// ---------- 状态栏几何（StatusBar.kt 的镜像） ----------

struct OpiBarMetrics {
    static let defaultScale = 100
    static let minScale = 75
    static let maxScale = 150
    /// 设置页的档位（Linux `STATUS_BAR_SCALE_CHOICES`）。
    static let scaleChoices = [75, 90, 100, 110, 125, 150]

    let btn: CGFloat
    let gap: CGFloat
    let pad: CGFloat
    let height: CGFloat
    let scale: CGFloat

    static func clamp(_ percent: Int) -> Int {
        min(max(percent, minScale), maxScale)
    }

    static func make(scale percent: Int) -> OpiBarMetrics {
        let k = CGFloat(clamp(percent)) / 100.0
        return OpiBarMetrics(btn: 30 * k, gap: 4 * k, pad: 6 * k,
                             height: 42 * k, scale: k)
    }

    func width(buttonCount: Int) -> CGFloat {
        guard buttonCount > 0 else { return 0 }
        return pad * 2 + btn * CGFloat(buttonCount) + gap * CGFloat(buttonCount - 1)
    }
}

// ---------- 自绘控件 ----------

/// AppKit 默认原点在左下；手工 frame 布局按「从上往下」排更接近读文档的顺序。
final class OpiFlippedView: NSView {
    override var isFlipped: Bool { true }
}

/// 无边框自绘按钮：皮肤底色 hover、attributedTitle 统一上色。
/// 面板都是 `nonactivatingPanel`，所以首击必须自己接住（否则第一次点击只激活进程）。
final class OpiHoverButton: NSButton {
    var highlightWhenOn = false
    var normalColor: NSColor = OpiTheme.palette.fg
    var baseFontSize: CGFloat = 13
    var baseBold = false

    private var palette: OpiPalette = OpiTheme.palette
    private var hovering = false
    private var tracking: NSTrackingArea?
    private var themedText = ""

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        common()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        common()
    }

    private func common() {
        isBordered = false
        bezelStyle = .regularSquare
        setButtonType(.momentaryChange)
        focusRingType = .none
        wantsLayer = true
        layer?.cornerRadius = 6
        layer?.masksToBounds = true
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    func apply(_ palette: OpiPalette, color: NSColor? = nil,
               size: CGFloat? = nil, bold: Bool? = nil) {
        self.palette = palette
        if let color { normalColor = color }
        if let size { baseFontSize = size }
        if let bold { baseBold = bold }
        layer?.cornerRadius = max(4, baseFontSize * 0.35)
        setThemedTitle(themedText)
        updateBackground()
    }

    func setThemedTitle(_ text: String, color: NSColor? = nil,
                        size: CGFloat? = nil, bold: Bool? = nil) {
        themedText = text
        if let color { normalColor = color }
        if let size { baseFontSize = size }
        if let bold { baseBold = bold }
        title = ""
        attributedTitle = OpiTheme.attributed(themedText, color: normalColor,
                                             size: baseFontSize, bold: baseBold)
        updateBackground()
    }

    override var state: NSControl.StateValue {
        didSet { updateBackground() }
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let tracking { removeTrackingArea(tracking) }
        let area = NSTrackingArea(rect: bounds,
                                  options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect],
                                  owner: self, userInfo: nil)
        addTrackingArea(area)
        tracking = area
    }

    override func mouseEntered(with event: NSEvent) {
        hovering = true
        updateBackground()
    }

    override func mouseExited(with event: NSEvent) {
        hovering = false
        updateBackground()
    }

    private func updateBackground() {
        let on = state == .on && highlightWhenOn
        let color: NSColor = hovering || on ? palette.hover : .clear
        layer?.backgroundColor = color.cgColor
    }
}
