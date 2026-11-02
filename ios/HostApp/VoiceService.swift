// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 宿主进程内的语音服务单例。iOS 用系统 SFSpeechRecognizer 做本地识别，
// 没有模型加载/下载状态；语音页只负责权限、录音和回填。

import Foundation

final class VoiceService {
    static let shared = VoiceService()

    let engine = OpiSpeechEngine()

    private static let languageKey = "opi.voice.language"

    /// 语音页记住用户选过的语言；auto 表示跟随系统当前语言。
    var preferredLanguage: String {
        get { UserDefaults.standard.string(forKey: Self.languageKey) ?? "auto" }
        set { UserDefaults.standard.set(newValue, forKey: Self.languageKey) }
    }

    func cancel() {
        engine.cancel()
    }
}
