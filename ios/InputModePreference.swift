// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 输入模式的持久化（2026-10-21）。Android 的 `ImePreferences` 记住上次输入模式；
// iOS 此前启动时沿用了引擎当前方案（同一进程里可能是别的键盘实例留下的），
// 于是「重装/重启后默认 9 键」做不到。
//
// 存的是 `InputMode.rawValue`（枚举已显式声明 Int 原始值，新增项只往后追加）。
// **缺省值 = `.pinyin9`**：用户没选过时（键不存在）与值损坏时都回 9 键；
// 不能用 `integer(forKey:)` 的 0 来判「没设置过」—— 0 恰好是 `.pinyin26`。
//
// 另存一份「上次中文模式」：从英文 26 键切回中文时，控制器要回到它
// （Android `cycleMode` 的「回上次中文方案」）。它**不随英文选择被覆盖**。

import Foundation

enum InputModePreference {
    private static let defaults = UserDefaults.standard
    private static let modeKey = "opi.inputMode"
    private static let lastChineseKey = "opi.inputMode.lastChinese"

    /// 启动时恢复的模式。面板式（生僻字）也**照实恢复**：Android 的
    /// `ImePreferences.inputMode` 按 name 落盘，`restore` → `selectInputMode`
    /// 对 panel 模式直接打开生僻字面板；拒绝它会违反「记住上次选择」。
    /// 未知/损坏值才回 9 键。
    static var restored: InputMode {
        guard let raw = defaults.object(forKey: modeKey) as? Int,
              let mode = InputMode(rawValue: raw)
        else { return .pinyin9 }
        return mode
    }

    /// 启动时的「上次中文模式」。缺省/损坏/存成英文 → 跟当前模式或 9 键。
    static var restoredLastChinese: InputMode {
        if let raw = defaults.object(forKey: lastChineseKey) as? Int,
           let mode = InputMode(rawValue: raw),
           !mode.panel,
           mode != .english26 {
            return mode
        }
        let current = restored
        return (current != .english26 && !current.panel) ? current : .pinyin9
    }

    static var inputMode: InputMode {
        get { restored }
        set { defaults.set(newValue.rawValue, forKey: modeKey) }
    }

    static var lastChinese: InputMode {
        get { restoredLastChinese }
        set { defaults.set(newValue.rawValue, forKey: lastChineseKey) }
    }
}

/// 9 键左列末尾「自定义标点」键的内容（2026-10-24）。
/// **空串 = 还没选过**（键面「自定义」，点它开选择器）；选择器写入用户点的那个符号，
/// 之后点该格直提、长按更换。
enum PunctuationPreference {
    private static let defaults = UserDefaults.standard
    private static let key = "opi.punct.custom"

    static var custom: String {
        get { defaults.string(forKey: key) ?? "" }
        set { defaults.set(newValue, forKey: key) }
    }
}
