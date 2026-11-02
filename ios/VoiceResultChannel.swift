// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 宿主 App → 键盘扩展的**单向**语音回填通道。
//
// 首选 App Group `group.xyz.erik.opi`：需要签名时给宿主与扩展**都**加这个
// application-groups entitlement，并各自配置含该 group 的 provisioning profile。
// 本仓库产出的是 unsigned IPA（没有签名就没有共享容器），所以 `appGroupReady`
// 在未重签时恒为 false，自动退回 UIPasteboard：
//   · 宿主写 `opi-voice:` + 文本；
//   · 键盘只在 `hasFullAccess == true` 时读，读完清掉标记。
// 这不是降级借口：没有完全访问时键盘读不到，键盘会明确提示，不会静默失败。
//
// 本文件同时编进宿主 App 与键盘扩展（package-ios.sh 两边都带它）。

import UIKit

enum VoiceResultChannel {
    /// 与 `OpiClipboard.appGroupID` 同一个 group；改这里要同步那边。
    static let appGroupID = "group.xyz.erik.opi"
    static let defaultsKey = "opi.voice.result"
    static let pasteboardPrefix = "opi-voice:"

    /// 只有真拿到共享容器才算可用；`UserDefaults(suiteName:)` 在无 entitlement
    /// 时也会返回对象，单看它会把“本进程可见”误当成“跨进程可见”。
    static var appGroupReady: Bool {
        FileManager.default.containerURL(
            forSecurityApplicationGroupIdentifier: appGroupID
        ) != nil
    }

    static func store(_ text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        if appGroupReady, let defaults = UserDefaults(suiteName: appGroupID) {
            defaults.set(trimmed, forKey: defaultsKey)
            defaults.synchronize()
        } else {
            UIPasteboard.general.string = pasteboardPrefix + trimmed
        }
    }

    /// 读取并清除待插入结果。无 App Group 时调用方必须先确认完全访问。
    static func take() -> String? {
        if appGroupReady, let defaults = UserDefaults(suiteName: appGroupID) {
            guard let text = defaults.string(forKey: defaultsKey) else { return nil }
            defaults.removeObject(forKey: defaultsKey)
            return text.isEmpty ? nil : text
        }
        guard let raw = UIPasteboard.general.string, raw.hasPrefix(pasteboardPrefix) else {
            return nil
        }
        let text = String(raw.dropFirst(pasteboardPrefix.count))
        // 只在内容仍是我们的标记时才清空，避免覆盖用户后来复制的东西。
        if UIPasteboard.general.string == raw {
            UIPasteboard.general.string = ""
        }
        return text.isEmpty ? nil : text
    }
}
