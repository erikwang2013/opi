// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 键盘侧语音入口：不创建 ASR 引擎、不请求麦克风，只做两件事：
//   1. 工具条麦克风 → 打开宿主 App 的语音页（`opi-voice://record`）；
//   2. 键盘每次可见时检查一次共享回填结果，读到了就走 `insert()`。
//
// 系统拒绝打开时**必须**有可见提示（不能静默）；没有完全访问且没有 App Group
// 时，粘贴板通道读不到，提示用户去宿主 App 复制。

import UIKit

extension KeyboardViewController {
    private static let voiceLaunchKey = "opi.voice.launchPending"

    func openVoiceFromKeyboard() {
        // 记一笔“刚把用户送去语音页”：回来后若结果读不到，才能给出具体原因，
        // 而不是让用户以为键盘坏了。
        UserDefaults.standard.set(true, forKey: Self.voiceLaunchKey)
        guard let url = URL(string: "opi-voice://record") else { return }
        guard let context = extensionContext else {
            clearVoiceLaunch()
            showVoiceMessage("请打开小欧 App 使用语音")
            return
        }
        context.open(url) { [weak self] opened in
            guard !opened else { return }
            DispatchQueue.main.async {
                self?.clearVoiceLaunch()
                self?.showVoiceMessage("请打开小欧 App 使用语音")
            }
        }
    }

    /// 由 `OpiClipboard.keyboardDidAppear` 在键盘可见时调（最早的可交互时机）。
    func consumeVoiceResultIfNeeded() {
        let launched = UserDefaults.standard.bool(forKey: Self.voiceLaunchKey)

        if VoiceResultChannel.appGroupReady {
            if let text = VoiceResultChannel.take() {
                clearVoiceLaunch()
                insert(text)
                showVoiceMessage("已插入语音识别结果")
            } else if launched {
                clearVoiceLaunch()
                showVoiceMessage("未找到语音结果；请在小欧 App 中重试或复制文本")
            }
            return
        }

        // 无 App Group：UIPasteboard 只在完全访问下可读。
        guard hasFullAccess else {
            if launched {
                clearVoiceLaunch()
                showVoiceMessage("语音结果需完全访问才能回填；请在小欧 App 中复制")
            }
            return
        }
        if let text = VoiceResultChannel.take() {
            clearVoiceLaunch()
            insert(text)
            showVoiceMessage("已插入语音识别结果")
        } else if launched {
            clearVoiceLaunch()
            showVoiceMessage("未找到语音结果；请在小欧 App 中重试或复制文本")
        }
    }

    private func clearVoiceLaunch() {
        UserDefaults.standard.removeObject(forKey: Self.voiceLaunchKey)
    }

    private func showVoiceMessage(_ message: String) {
        OpiClipboard.shared.showFlash(message)
    }
}
