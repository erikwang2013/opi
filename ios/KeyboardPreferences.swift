// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 本端偏好（UserDefaults）—— 2026-10-27 从 `KeyboardTheme.swift` 拆出：文件原本正好
// 500 行，iOS 26 顶部圆角适配的注释一加就撞 500 行门禁（本仓惯例：`*_tests.rs` /
// `engine_punct.rs` 同一套先例）。语义一字未动。

import Foundation

/// 本端偏好（UserDefaults）。Android 用 `ImePreferences`；本端没有设置 App，
/// 但扩展自己的 UserDefaults 在 `RequestsOpenAccess=false` 下也能读写 ——
/// 它只是扩展容器内的持久化，不涉及宿主数据。
///
/// 持久化皮肤、键盘模式与设置面板里的引擎开关（2026-10-21）。输入模式另见
/// `InputModePreference.swift`（缺省 9 键、记住上次选择，对齐 Android `ImePreferences`）。
enum KeyboardPreferences {
    private static let defaults = UserDefaults.standard
    private static let skinKey = "opi.skin"
    private static let sceneKey = "opi.scene"
    private static let oneHandKey = "opi.mode.oneHand"
    private static let darkKey = "opi.mode.dark"
    private static let elderKey = "opi.mode.elder"
    private static let floatingKey = "opi.mode.floating"
    private static let gameKey = "opi.mode.game"
    private static let learnKey = "opi.setting.learn"
    private static let fuzzyKey = "opi.setting.fuzzy"
    private static let fuzzyPairsKey = "opi.setting.fuzzyPairs"
    private static let jianpinKey = "opi.setting.jianpin"
    private static let associationKey = "opi.setting.association"
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
        get { SkinOption(rawValue: defaults.string(forKey: skinKey) ?? "") ?? .fresh }
        set { defaults.set(newValue.rawValue, forKey: skinKey) }
    }

    /// 动态插画主题；空/未知 = 纯色皮肤（配色见 `skin`）。
    static var scene: SceneTheme? {
        get { SceneTheme(rawValue: defaults.string(forKey: sceneKey) ?? "") }
        set { defaults.set(newValue?.rawValue ?? "", forKey: sceneKey) }
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
                association: readBool(associationKey, default: false),
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
            defaults.set(newValue.association, forKey: associationKey)
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
