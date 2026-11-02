// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 宿主进程内的语音引擎单例：启动/回前台预热一次，语音页复用同一个 handle。
//
// 预热（用户已确认）：宿主 App 每次 active 都调 `prewarm()`；模型已加载时是空操作，
// 未加载时才在后台 `opi_asr_load_model`。normal 档模型没下载时什么也不做 ——
// 下载必须由用户点按钮触发，不在后台偷跑 615MB。

import Foundation

extension Notification.Name {
    /// 模型加载/状态变化（主线程）；语音页据此刷新按钮与文案。
    static let opiVoiceServiceChanged = Notification.Name("opi.voice.service.changed")
}

final class VoiceService {
    static let shared = VoiceService()

    let engine = OpiAsrEngine()
    private let models = AsrModelManager.shared
    private let loadQueue = DispatchQueue(
        label: "xyz.erik.opi.asr.load", qos: .userInitiated
    )
    private(set) var loadedLanguage: String?
    private(set) var isLoading = false

    private static let languageKey = "opi.voice.language"

    var modelsRoot: URL? { models.readyRoot() }
    var usesBundledModels: Bool { models.bundledRoot != nil }
    var needsDownload: Bool { modelsRoot == nil }

    /// 语音页记住用户选过的语言；预热按它加载，避免打开页面后又重载一次。
    var preferredLanguage: String {
        get { UserDefaults.standard.string(forKey: Self.languageKey) ?? "auto" }
        set { UserDefaults.standard.set(newValue, forKey: Self.languageKey) }
    }

    func prewarm() {
        guard modelsRoot != nil, !engine.isLoaded, !isLoading else { return }
        prepare(language: preferredLanguage, completion: nil)
    }

    /// 保证引擎已按 `language` 加载。回调在主线程；失败参数是给用户看的文案。
    func prepare(language: String, completion: ((String?) -> Void)?) {
        if engine.isLoaded, loadedLanguage == language {
            completion?(nil)
            return
        }
        guard let root = modelsRoot else {
            completion?("语音模型未下载")
            return
        }
        if isLoading {
            completion?("正在准备语音模型，请稍候")
            return
        }
        isLoading = true
        loadQueue.async { [weak self] in
            guard let self else { return }
            let error = self.engine.load(modelsRoot: root, language: language)
            DispatchQueue.main.async {
                self.isLoading = false
                self.loadedLanguage = error == nil ? language : nil
                completion?(error)
                NotificationCenter.default.post(name: .opiVoiceServiceChanged, object: nil)
            }
        }
    }

    func cancel() {
        _ = engine.cancel()
    }
}
