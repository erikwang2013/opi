// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 皮肤与键盘模式（2026-10-20）。参照实现是 Android `keyboard/Palette.kt` +
// `ime/KeyboardModes.kt`：**12 款皮肤、12 个色板角色、优先级「长辈 > 暗光 > 皮肤」**
// 全部逐字搬过来（值不重设计 —— 改了就是两端观感分叉）。
//
// ⚠️ 与 Android 的落地层差别：Android 的「悬浮 / 游戏」在 IME 窗口层
// （`OpiImeService.onConfigureWindow`）。iOS 键盘扩展**不能控制自己的窗口**
// （窗口属系统，`UIInputViewController` 只提供 `dismissKeyboard`），
// 所以这两项只保留可见的禁用项，不伪造支持 —— 见 `KeyboardPanels.swift` 与 README。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译；从未上机运行。

import UIKit

/// 键盘配色（`KeyboardPalette` 的 iOS 版）。角色与 Android 一一对应。
struct KeyboardPalette {
    let keyBg: UIColor
    let keyFg: UIColor
    let keyHighlightBg: UIColor
    let keyHighlightFg: UIColor
    let barBg: UIColor
    let barBufferFg: UIColor
    let barDimFg: UIColor
    let accent: UIColor
    let panelBg: UIColor
    let panelFg: UIColor
    let toolbarBg: UIColor
    let toolbarFg: UIColor
    /// 深色底（给 UIKit 的系统控件切 `.dark`，别让 `.label` 在深底上仍是黑字）。
    let isDark: Bool

    init(
        keyBg: UIColor, keyFg: UIColor,
        keyHighlightBg: UIColor, keyHighlightFg: UIColor,
        barBg: UIColor, barBufferFg: UIColor, barDimFg: UIColor,
        accent: UIColor, panelBg: UIColor, panelFg: UIColor,
        toolbarBg: UIColor, toolbarFg: UIColor,
        isDark: Bool = false
    ) {
        self.keyBg = keyBg
        self.keyFg = keyFg
        self.keyHighlightBg = keyHighlightBg
        self.keyHighlightFg = keyHighlightFg
        self.barBg = barBg
        self.barBufferFg = barBufferFg
        self.barDimFg = barDimFg
        self.accent = accent
        self.panelBg = panelBg
        self.panelFg = panelFg
        self.toolbarBg = toolbarBg
        self.toolbarFg = toolbarFg
        self.isDark = isDark
    }
}

/// 0xAARRGGBB（与 Android `Color(0x...)` 同一种写法，抄值时不会看错通道）。
private extension UIColor {
    convenience init(argb: UInt32) {
        self.init(
            red: CGFloat((argb >> 16) & 0xFF) / 255,
            green: CGFloat((argb >> 8) & 0xFF) / 255,
            blue: CGFloat(argb & 0xFF) / 255,
            alpha: CGFloat((argb >> 24) & 0xFF) / 255
        )
    }
}

extension KeyboardPalette {
    /// 浅色（默认观感）。值 = Android `LightPalette`。
    static let light = KeyboardPalette(
        keyBg: UIColor(argb: 0xFFE0E0E0), keyFg: UIColor(argb: 0xFF1F1F1F),
        keyHighlightBg: UIColor(argb: 0xFF546E7A), keyHighlightFg: UIColor(argb: 0xFFFFFFFF),
        barBg: UIColor(argb: 0xFFEEEEEE), barBufferFg: UIColor(argb: 0x8A000000),
        barDimFg: UIColor(argb: 0xFF757575), accent: UIColor(argb: 0xFF455A64),
        panelBg: UIColor(argb: 0xFFEEEEEE), panelFg: UIColor(argb: 0xFF1F1F1F),
        toolbarBg: UIColor(argb: 0xFFE0E0E0), toolbarFg: UIColor(argb: 0xFF455A64)
    )

    /// 暗光模式。
    static let dark = KeyboardPalette(
        keyBg: UIColor(argb: 0xFF2A2A2A), keyFg: UIColor(argb: 0xFFE6E6E6),
        keyHighlightBg: UIColor(argb: 0xFF4A6572), keyHighlightFg: UIColor(argb: 0xFFFFFFFF),
        barBg: UIColor(argb: 0xFF1F1F1F), barBufferFg: UIColor(argb: 0xCCE6E6E6),
        barDimFg: UIColor(argb: 0xFF9E9E9E), accent: UIColor(argb: 0xFF9FB4BE),
        panelBg: UIColor(argb: 0xFF1F1F1F), panelFg: UIColor(argb: 0xFFE6E6E6),
        toolbarBg: UIColor(argb: 0xFF2A2A2A), toolbarFg: UIColor(argb: 0xFFCFD8DC),
        isDark: true
    )

    /// 长辈模式：高对比（字号由 `KeyboardTheme.fontScale` 放大）。
    static let elder = KeyboardPalette(
        keyBg: UIColor(argb: 0xFFFAFAFA), keyFg: UIColor(argb: 0xFF000000),
        keyHighlightBg: UIColor(argb: 0xFF263238), keyHighlightFg: UIColor(argb: 0xFFFFFFFF),
        barBg: UIColor(argb: 0xFFFFFFFF), barBufferFg: UIColor(argb: 0xFF000000),
        barDimFg: UIColor(argb: 0xFF424242), accent: UIColor(argb: 0xFF263238),
        panelBg: UIColor(argb: 0xFFFFFFFF), panelFg: UIColor(argb: 0xFF000000),
        toolbarBg: UIColor(argb: 0xFFF5F5F5), toolbarFg: UIColor(argb: 0xFF263238)
    )

    static let paper = KeyboardPalette(
        keyBg: UIColor(argb: 0xFFE7DFCC), keyFg: UIColor(argb: 0xFF3D3222),
        keyHighlightBg: UIColor(argb: 0xFF8A5A2B), keyHighlightFg: UIColor(argb: 0xFFFFFBF2),
        barBg: UIColor(argb: 0xFFF5EFE1), barBufferFg: UIColor(argb: 0xCC3D3222),
        barDimFg: UIColor(argb: 0xFF8A7A5B), accent: UIColor(argb: 0xFF8A5A2B),
        panelBg: UIColor(argb: 0xFFF5EFE1), panelFg: UIColor(argb: 0xFF3D3222),
        toolbarBg: UIColor(argb: 0xFFE7DFCC), toolbarFg: UIColor(argb: 0xFF8A5A2B)
    )

    static let cinnabar = KeyboardPalette(
        keyBg: UIColor(argb: 0xFF9E2228), keyFg: UIColor(argb: 0xFFFFF6EC),
        keyHighlightBg: UIColor(argb: 0xFFFFD9A0), keyHighlightFg: UIColor(argb: 0xFF5A1012),
        barBg: UIColor(argb: 0xFF8A1C21), barBufferFg: UIColor(argb: 0xCCFFF6EC),
        barDimFg: UIColor(argb: 0xFFE8BCB4), accent: UIColor(argb: 0xFFFFD9A0),
        panelBg: UIColor(argb: 0xFF9E2228), panelFg: UIColor(argb: 0xFFFFF6EC),
        toolbarBg: UIColor(argb: 0xFF8A1C21), toolbarFg: UIColor(argb: 0xFFFFD9A0),
        isDark: true
    )

    static let fresh = KeyboardPalette(
        keyBg: UIColor(argb: 0xFFCFE9DF), keyFg: UIColor(argb: 0xFF16332B),
        keyHighlightBg: UIColor(argb: 0xFF2E7D6B), keyHighlightFg: UIColor(argb: 0xFFFFFFFF),
        barBg: UIColor(argb: 0xFFE6F5EE), barBufferFg: UIColor(argb: 0xCC16332B),
        barDimFg: UIColor(argb: 0xFF4F7A6C), accent: UIColor(argb: 0xFF2E7D6B),
        panelBg: UIColor(argb: 0xFFE6F5EE), panelFg: UIColor(argb: 0xFF16332B),
        toolbarBg: UIColor(argb: 0xFFCFE9DF), toolbarFg: UIColor(argb: 0xFF2E7D6B)
    )

    static let azure = KeyboardPalette(
        keyBg: UIColor(argb: 0xFF3C6AA3), keyFg: UIColor(argb: 0xFFF4F8FD),
        keyHighlightBg: UIColor(argb: 0xFFCFE6FF), keyHighlightFg: UIColor(argb: 0xFF12395C),
        barBg: UIColor(argb: 0xFF3A679E), barBufferFg: UIColor(argb: 0xF0F4F8FD),
        barDimFg: UIColor(argb: 0xFFC9DCF2), accent: UIColor(argb: 0xFFFFE3A8),
        panelBg: UIColor(argb: 0xFF3A679E), panelFg: UIColor(argb: 0xFFF4F8FD),
        toolbarBg: UIColor(argb: 0xFF33598A), toolbarFg: UIColor(argb: 0xFFFFE3A8),
        isDark: true
    )

    static let lemon = KeyboardPalette(
        keyBg: UIColor(argb: 0xFFF9EA6C), keyFg: UIColor(argb: 0xFF3A3000),
        keyHighlightBg: UIColor(argb: 0xFF7E6200), keyHighlightFg: UIColor(argb: 0xFFFFFDF0),
        barBg: UIColor(argb: 0xFFFBEE7E), barBufferFg: UIColor(argb: 0xCC3A3000),
        barDimFg: UIColor(argb: 0xFF6E5F1E), accent: UIColor(argb: 0xFF7E6200),
        panelBg: UIColor(argb: 0xFFFBEE7E), panelFg: UIColor(argb: 0xFF3A3000),
        toolbarBg: UIColor(argb: 0xFFF9EA6C), toolbarFg: UIColor(argb: 0xFF5A4A00)
    )

    static let sunset = KeyboardPalette(
        keyBg: UIColor(argb: 0xFFF4BC8A), keyFg: UIColor(argb: 0xFF4A2317),
        keyHighlightBg: UIColor(argb: 0xFFA63412), keyHighlightFg: UIColor(argb: 0xFFFFF3E8),
        barBg: UIColor(argb: 0xFFF7C79C), barBufferFg: UIColor(argb: 0xCC4A2317),
        barDimFg: UIColor(argb: 0xFF8E5A40), accent: UIColor(argb: 0xFFA63412),
        panelBg: UIColor(argb: 0xFFF7C79C), panelFg: UIColor(argb: 0xFF4A2317),
        toolbarBg: UIColor(argb: 0xFFF4BC8A), toolbarFg: UIColor(argb: 0xFFB4441F)
    )

    static let starlight = KeyboardPalette(
        keyBg: UIColor(argb: 0xFF262E4A), keyFg: UIColor(argb: 0xFFE8EBF7),
        keyHighlightBg: UIColor(argb: 0xFFFFD966), keyHighlightFg: UIColor(argb: 0xFF23283F),
        barBg: UIColor(argb: 0xFF161B2E), barBufferFg: UIColor(argb: 0xCCE8EBF7),
        barDimFg: UIColor(argb: 0xFF9AA3C0), accent: UIColor(argb: 0xFFFFD966),
        panelBg: UIColor(argb: 0xFF1B2138), panelFg: UIColor(argb: 0xFFE8EBF7),
        toolbarBg: UIColor(argb: 0xFF262E4A), toolbarFg: UIColor(argb: 0xFFFFD966),
        isDark: true
    )

    static let wisteria = KeyboardPalette(
        keyBg: UIColor(argb: 0xFFE3D9F8), keyFg: UIColor(argb: 0xFF3A2A63),
        keyHighlightBg: UIColor(argb: 0xFF6B4BC4), keyHighlightFg: UIColor(argb: 0xFFF7F3FF),
        barBg: UIColor(argb: 0xFFEDE7FB), barBufferFg: UIColor(argb: 0xCC3A2A63),
        barDimFg: UIColor(argb: 0xFF6E5E96), accent: UIColor(argb: 0xFF6B4BC4),
        panelBg: UIColor(argb: 0xFFEDE7FB), panelFg: UIColor(argb: 0xFF3A2A63),
        toolbarBg: UIColor(argb: 0xFFE3D9F8), toolbarFg: UIColor(argb: 0xFF5A3AA8)
    )

    static let pine = KeyboardPalette(
        keyBg: UIColor(argb: 0xFF26473A), keyFg: UIColor(argb: 0xFFDCEEE2),
        keyHighlightBg: UIColor(argb: 0xFF7FD3A2), keyHighlightFg: UIColor(argb: 0xFF17301F),
        barBg: UIColor(argb: 0xFF1E3A2F), barBufferFg: UIColor(argb: 0xCCDCEEE2),
        barDimFg: UIColor(argb: 0xFF9DBFAE), accent: UIColor(argb: 0xFF7FD3A2),
        panelBg: UIColor(argb: 0xFF1E3A2F), panelFg: UIColor(argb: 0xFFDCEEE2),
        toolbarBg: UIColor(argb: 0xFF26473A), toolbarFg: UIColor(argb: 0xFF7FD3A2),
        isDark: true
    )
}

/// 皮肤（与 Android `SkinOption` 同名、同次序、同 id —— 持久化值跨端一致）。
enum SkinOption: String, CaseIterable {
    case follow, light, dark, paper, cinnabar, fresh, azure, lemon, sunset, starlight, wisteria, pine

    var label: String {
        switch self {
        case .follow: return "不改动"
        case .light: return "浅色"
        case .dark: return "深色"
        case .paper: return "宣纸"
        case .cinnabar: return "朱砂"
        case .fresh: return "清爽"
        case .azure: return "湛蓝"
        case .lemon: return "柠檬"
        case .sunset: return "晚霞"
        case .starlight: return "星光"
        case .wisteria: return "紫藤"
        case .pine: return "松林"
        }
    }

    var palette: KeyboardPalette {
        switch self {
        case .follow, .light: return .light
        case .dark: return .dark
        case .paper: return .paper
        case .cinnabar: return .cinnabar
        case .fresh: return .fresh
        case .azure: return .azure
        case .lemon: return .lemon
        case .sunset: return .sunset
        case .starlight: return .starlight
        case .wisteria: return .wisteria
        case .pine: return .pine
        }
    }
}

/// 键盘模式（对齐 Android `KeyboardModes`；繁体是引擎模式，不在这里）。
struct KeyboardModes: Equatable {
    enum OneHand: String, CaseIterable {
        case off, left, right
    }

    var floating = false
    var game = false
    var oneHand: OneHand = .off
    var dark = false
    var elder = false

    var anyOn: Bool { floating || game || oneHand != .off || dark || elder }
}

/// 双拼方案名（顺序 = `opi_set_shuangpin_layout` 的下标，0..5）。
/// ⚠️ 与三端逐字一致（Android `SHUANGPIN_LAYOUTS_ANDROID`、fcitx5 `opi_config.h`、
/// `crates/engine-core/src/engine_shuangpin.rs`）—— 改顺序会让用户设置静默跳到别的方案。
let SHUANGPIN_LAYOUT_NAMES = ["自然码", "微软", "小鹤", "智能ABC", "紫光", "拼音加加"]

/// 五笔旋钮的名字表（顺序 = 引擎枚举序 = `opi_set_wubi_*` 的 Int 值）。
let WUBI_STYLE_NAMES = ["纯五笔", "五笔拼音"]
let WUBI_MIXED_ENTRY_NAMES = ["直接字母", "z键"]
let WUBI_RETURN_NAMES = ["清空", "上屏原文"]
/// 候选个数档位（下限 3 是产品档位，上限 = 引擎上限；缺省 5）。
let WUBI_CANDIDATE_LIMITS = Array(3...9)
let WUBI_CANDIDATE_LIMIT_DEFAULT = 5

/// 设置面板里的引擎开关（Android `ImePreferences` 的可用子集）。
///
/// **只放 C ABI 有读写出口的项**（`macos/OpiFFI.h`）：学习、模糊音（总开关 +
/// 逐组名）、简拼、纠错、中文标点、双拼布局、五笔五个旋钮；再加上本端表情页
/// 的 `showOwn`。英文专业模式、悬浮/游戏等没有出口/系统不可用的项**不放进来**，
/// 面板里以「不可用」明文标注 —— 不做假开关。
struct KeyboardSettings: Equatable {
    /// 学习（用户词频）。缺省开（Android 同）。
    var learn = true
    /// 模糊音总开关。缺省关（Android 同）。
    var fuzzy = false
    /// 逗号分隔的规范组名；空串 = 未设置 ⇒ 引擎按 `fuzzy` 布尔回落
    /// （与 Android `fuzzyPairs` 的合同一字不差）。
    var fuzzyPairs = ""
    /// 简拼。缺省开。
    var jianpin = true
    /// 拼音纠错。缺省开。
    var correction = true
    /// 中文标点。缺省开。
    var chinesePunct = true
    /// 双拼方案名（`SHUANGPIN_LAYOUT_NAMES` 之一）。
    var shuangpinLayout = SHUANGPIN_LAYOUT_NAMES[0]
    /// 五笔：输入风格（0 纯五笔 / 1 五笔拼音混输）。
    var wubiStyle = 0
    /// 五笔：混输进入方式（0 直接字母 / 1 z 键）。
    var wubiMixedEntry = 0
    /// 五笔：候选个数（3...9）。
    var wubiCandidateLimit = WUBI_CANDIDATE_LIMIT_DEFAULT
    /// 五笔：回车行为（0 清空 / 1 上屏原文）。
    var wubiReturn = 0
    /// 五笔：四码唯一自动上屏（缺省开）。
    var wubiAutoCommit = true
    /// 表情页显示「自有」贴纸分类（Android `showOwn`，缺省隐藏）。
    var showOwn = false
}

enum KeyboardTheme {
    /// 三条配色轴的优先级：长辈 > 暗光 > 皮肤（与 Android `ImeScreen` 的 when 同序）。
    static func palette(skin: SkinOption, modes: KeyboardModes) -> KeyboardPalette {
        if modes.elder { return .elder }
        if modes.dark { return .dark }
        return skin.palette
    }

    /// 长辈模式的字号倍数（Android `fontScale * 1.18f`）。
    static func fontScale(_ modes: KeyboardModes) -> CGFloat {
        modes.elder ? 1.18 : 1
    }
}

/// 本端偏好（UserDefaults）。Android 用 `ImePreferences`；本端没有设置 App，
/// 但扩展自己的 UserDefaults 在 `RequestsOpenAccess=false` 下也能读写 ——
/// 它只是扩展容器内的持久化，不涉及宿主数据。
///
/// 持久化皮肤、键盘模式与设置面板里的引擎开关（2026-10-21）。输入模式另见
/// `InputModePreference.swift`（缺省 9 键、记住上次选择，对齐 Android `ImePreferences`）。
enum KeyboardPreferences {
    private static let defaults = UserDefaults.standard
    private static let skinKey = "opi.skin"
    private static let oneHandKey = "opi.mode.oneHand"
    private static let darkKey = "opi.mode.dark"
    private static let elderKey = "opi.mode.elder"
    private static let floatingKey = "opi.mode.floating"
    private static let gameKey = "opi.mode.game"
    private static let learnKey = "opi.setting.learn"
    private static let fuzzyKey = "opi.setting.fuzzy"
    private static let fuzzyPairsKey = "opi.setting.fuzzyPairs"
    private static let jianpinKey = "opi.setting.jianpin"
    private static let correctionKey = "opi.setting.correction"
    private static let chinesePunctKey = "opi.setting.chinesePunct"
    private static let shuangpinKey = "opi.setting.shuangpinLayout"
    private static let showOwnKey = "opi.setting.showOwn"
    private static let wubiStyleKey = "opi.setting.wubiStyle"
    private static let wubiEntryKey = "opi.setting.wubiMixedEntry"
    private static let wubiLimitKey = "opi.setting.wubiCandidateLimit"
    private static let wubiReturnKey = "opi.setting.wubiReturn"
    private static let wubiAutoCommitKey = "opi.setting.wubiAutoCommit"

    /// `UserDefaults.bool` 对缺失键返回 false；缺省为 true 的项必须区分
    /// 「没设置过」与「用户关了」。
    private static func readBool(_ key: String, default def: Bool) -> Bool {
        defaults.object(forKey: key) == nil ? def : defaults.bool(forKey: key)
    }

    static var skin: SkinOption {
        get { SkinOption(rawValue: defaults.string(forKey: skinKey) ?? "") ?? .follow }
        set { defaults.set(newValue.rawValue, forKey: skinKey) }
    }

    static var modes: KeyboardModes {
        get {
            KeyboardModes(
                floating: defaults.bool(forKey: floatingKey),
                game: defaults.bool(forKey: gameKey),
                oneHand: KeyboardModes.OneHand(
                    rawValue: defaults.string(forKey: oneHandKey) ?? ""
                ) ?? .off,
                dark: defaults.bool(forKey: darkKey),
                elder: defaults.bool(forKey: elderKey)
            )
        }
        set {
            defaults.set(newValue.floating, forKey: floatingKey)
            defaults.set(newValue.game, forKey: gameKey)
            defaults.set(newValue.oneHand.rawValue, forKey: oneHandKey)
            defaults.set(newValue.dark, forKey: darkKey)
            defaults.set(newValue.elder, forKey: elderKey)
        }
    }

    /// 设置面板的引擎开关（缺省值与 Android `ImePreferences` 对齐）。
    static var settings: KeyboardSettings {
        get {
            let layout = defaults.string(forKey: shuangpinKey) ?? ""
            let style = defaults.integer(forKey: wubiStyleKey)
            let entry = defaults.integer(forKey: wubiEntryKey)
            let limit = defaults.integer(forKey: wubiLimitKey)
            let ret = defaults.integer(forKey: wubiReturnKey)
            return KeyboardSettings(
                learn: readBool(learnKey, default: true),
                fuzzy: readBool(fuzzyKey, default: false),
                fuzzyPairs: defaults.string(forKey: fuzzyPairsKey) ?? "",
                jianpin: readBool(jianpinKey, default: true),
                correction: readBool(correctionKey, default: true),
                chinesePunct: readBool(chinesePunctKey, default: true),
                shuangpinLayout: SHUANGPIN_LAYOUT_NAMES.contains(layout)
                    ? layout : SHUANGPIN_LAYOUT_NAMES[0],
                wubiStyle: (0...1).contains(style) ? style : 0,
                wubiMixedEntry: (0...1).contains(entry) ? entry : 0,
                wubiCandidateLimit: WUBI_CANDIDATE_LIMITS.contains(limit)
                    ? limit : WUBI_CANDIDATE_LIMIT_DEFAULT,
                wubiReturn: (0...1).contains(ret) ? ret : 0,
                wubiAutoCommit: readBool(wubiAutoCommitKey, default: true),
                showOwn: readBool(showOwnKey, default: false)
            )
        }
        set {
            defaults.set(newValue.learn, forKey: learnKey)
            defaults.set(newValue.fuzzy, forKey: fuzzyKey)
            defaults.set(newValue.fuzzyPairs, forKey: fuzzyPairsKey)
            defaults.set(newValue.jianpin, forKey: jianpinKey)
            defaults.set(newValue.correction, forKey: correctionKey)
            defaults.set(newValue.chinesePunct, forKey: chinesePunctKey)
            defaults.set(newValue.shuangpinLayout, forKey: shuangpinKey)
            defaults.set(newValue.wubiStyle, forKey: wubiStyleKey)
            defaults.set(newValue.wubiMixedEntry, forKey: wubiEntryKey)
            defaults.set(newValue.wubiCandidateLimit, forKey: wubiLimitKey)
            defaults.set(newValue.wubiReturn, forKey: wubiReturnKey)
            defaults.set(newValue.wubiAutoCommit, forKey: wubiAutoCommitKey)
            defaults.set(newValue.showOwn, forKey: showOwnKey)
        }
    }
}

extension KeyboardViewController {

    /// 换肤 + 长辈字号 + 单手（启动与皮肤/键盘模式面板改完后调）。
    func applyTheme() {
        layout.setKeyboardModes(modes, traditional: engine.mode() == .traditional)
        layout.applyTheme(
            KeyboardTheme.palette(skin: skin, modes: modes),
            fontScale: KeyboardTheme.fontScale(modes)
        )
    }

    /// 关于面板的外链。**先复制再尝试打开**：键盘扩展不保证被允许直接开 URL，
    /// 打不开时至少剪贴板里有完整地址（面板里有同样一句提示）。
    func openExternal(_ raw: String) {
        guard let url = URL(string: raw) else { return }
        UIPasteboard.general.string = raw
        extensionContext?.open(url) { _ in }
    }
}

// MARK: - 模式循环（26 键）

extension KeyboardViewController {

    /// 模式三态循环：中 → 繁 → 英 → 中。
    ///
    /// **逐字对齐 Android `ImeScreen.kt` 的 `toggleMode()`**（含前两跳先 `clear()`）——
    /// 离开拼音类模式前清掉打了一半的拼音，否则残留缓冲会被下一个空格/回车意外提交。
    /// 本端不自创第三个模式的判定，理由见 `OpiMode.traditional` 的注释。
    ///
    /// **2026-10-21 起**：软键盘上的模式键/工具条「中」/候选栏「中」在**非中文态**
    /// 由控制器先拦下，走 `applyInputMode(lastChineseInputMode)`（回上次中文方案，
    /// 与 Android 的 else 档同义）。本函数只负责**中→繁、繁→英**两跳；硬件的
    /// `Ctrl+'` 在 `KeyboardHardware.performHotkey` 里走 `applyInputMode`（同步键面），
    /// 也不走这里。
    /// 两条路径都保留 —— 别把控制器那条折进来，也别删掉本函数的 else 档。
    ///
    /// ⚠️ 这里**没有**走「先提交未完成的缓冲」那条路，而且**原因不是「调不到」** ——
    /// `opi_toggle_symbol` / `opi_toggle_fullwidth` / `opi_fullwidth_state` 三个出口
    /// 头文件里**已经有了**（2026-09-28 实测 `cabi.rs` 31 ↔ `OpiFFI.h` 31、差集为空、签名不一致 0），
    /// `KeyboardHardware.swift` 的热键路径正在调它们。真正的原因是**语义不同**：
    /// `Engine::toggle_symbol` 是 **Pinyin ⇄ Symbol**（那是 `Ctrl+\` 那条热键），
    /// 而本函数是**中→繁→英**三态循环 —— 拿它来切模式是把两个出口当成一个用。
    /// 所以本函数照 Android `ImeScreen.kt` 的既有约定 `clear()`。
    ///
    /// ⚠️ **已裁决（2026-09-28，team-lead）：保持 `clear()`**，与 Android 的 `toggleMode()` 一致
    /// —— 本仓的价值里有「四端一致」，单方面改成「提交」会造出一条**只有 Apple 两平台不同**的行为。
    /// `toggle_symbol` 之所以要提交是因为**场景不同**：用户伸手去够符号时，那段拼音不该凭空消失；
    /// 而模式轮转是**换一种输入方式**，用户预期就是丢掉重来。
    /// **不要再问一遍，也不要顺手改成 `toggleSymbol()`** —— 那会连模式目标一起改掉。
    func cycleMode() {
        switch engine.mode() {
        case .pinyin:
            engine.clear()
            engine.switchMode(.traditional)
        case .traditional:
            engine.clear()
            engine.switchMode(.english)
            // 26 键面必须配全拼：这条腿不走 applyInputMode，否则 scheme 会留在
            // 上一套（如 9 键），之后从英文键面开繁体就会键面/引擎错位。
            engine.setScheme(InputMode.english26.scheme)
            // 与 Android `switchLanguageKeepingBuffer(InputMode.ENGLISH26)` 同一条：
            // 键面/菜单高亮跟着引擎走，`lastChineseInputMode` 仍留着回程目标。
            layout.setInputMode(.english26)
            // 这条腿不走 applyInputMode，持久化要在这里补（下次启动仍是英文 26 键）。
            InputModePreference.inputMode = .english26
        default:
            // English / Number / Symbol 一律回拼音（与 Android 的 `else` 分支同义）。
            engine.switchMode(.pinyin)
            // 软键盘在控制器里已拦下非中文态；这里兜住其它调用方：键面也要回中文。
            layout.setInputMode(lastChineseInputMode)
            InputModePreference.inputMode = lastChineseInputMode
        }
        updateModeLabel()
        refresh()
    }

    /// 模式键的键面字 = **点它会切到哪**（2026-10-12 晚与 Android 同步的裁决：
    /// 用户实测「切到英文键盘后键面还写着英，点下去却是中文键盘，键面该是『中』才对」）。
    /// 逐条对齐本端 [`cycleMode`] 的次序：中→繁、繁→英、英/数字/符号→中。
    /// 非中文态实际由控制器走 `applyInputMode(lastChineseInputMode)`（回上次中文方案，
    /// 与 Android 同义）；键面字仍按**各自**的循环给，别照抄另一端的实现。
    /// ⚠️ internal：`KeyboardHardware.swift` 的热键路径要调它。
    static func modeKeyLabel(of mode: OpiMode) -> String {
        switch mode {
        case .pinyin: return "繁"
        case .traditional: return "英"
        default: return "中"
        }
    }
}
