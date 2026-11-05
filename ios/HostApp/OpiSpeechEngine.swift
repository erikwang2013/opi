// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// iOS 宿主 App 的语音引擎：系统 SFSpeechRecognizer + 本地强制识别。
//
// 用户已确认改用系统 on-device 识别：不再链接 opi_asr/sherpa/onnxruntime，
// 也不下载任何模型。音频只经 AVAudioEngine 的 tap 追加到识别请求，进程退出即丢，
// 不落盘、不上传；`requiresOnDeviceRecognition = true` 禁止服务器回退。
// 识别结果仍由 VoiceService/语音页写入 VoiceResultChannel 回填键盘。

import AVFoundation
import Foundation
import Speech

enum OpiSpeechEvent {
    case partial(String)
    case final(String)
    case error(String)
}

final class OpiSpeechEngine {
    private let audio = AVAudioEngine()
    private var request: SFSpeechAudioBufferRecognitionRequest?
    private var task: SFSpeechRecognitionTask?
    private var tapInstalled = false
    private var accepting = false
    private var awaitingFinal = false

    /// 主线程回调；partial 只在录音中发出，final/error 各至多一次。
    var onEvent: ((OpiSpeechEvent) -> Void)?

    var isActive: Bool { accepting || awaitingFinal }

    static func requestSpeechAuthorization(
        completion: @escaping (SFSpeechRecognizerAuthorizationStatus) -> Void
    ) {
        let status = SFSpeechRecognizer.authorizationStatus()
        guard status == .notDetermined else {
            DispatchQueue.main.async { completion(status) }
            return
        }
        SFSpeechRecognizer.requestAuthorization { newStatus in
            DispatchQueue.main.async { completion(newStatus) }
        }
    }

    /// 成功返回 nil；失败返回给用户看的文案。回调在主线程同步或异步调用。
    func start(language: String, completion: @escaping (String?) -> Void) {
        guard !isActive else {
            completion("正在识别，请稍候")
            return
        }
        guard SFSpeechRecognizer.authorizationStatus() == .authorized else {
            completion("没有语音识别权限")
            return
        }
        guard let recognizer = Self.recognizer(for: language) else {
            completion("系统不支持所选语言的语音识别")
            return
        }
        guard recognizer.supportsOnDeviceRecognition else {
            completion("此设备或语言暂不支持本地语音识别；请在系统设置中下载听写语言后重试。")
            return
        }
        let session = AVAudioSession.sharedInstance()
        do {
            try session.setCategory(.record, mode: .measurement, options: [])
            try session.setActive(true)
        } catch {
            completion("无法启用麦克风：\(error.localizedDescription)")
            return
        }
        let input = audio.inputNode
        let format = input.inputFormat(forBus: 0)
        guard format.sampleRate > 0 else {
            stopAudio()
            completion("麦克风不可用")
            return
        }
        let request = SFSpeechAudioBufferRecognitionRequest()
        request.shouldReportPartialResults = true
        request.requiresOnDeviceRecognition = true
        request.taskHint = .dictation
        self.request = request
        accepting = true
        awaitingFinal = false
        task = recognizer.recognitionTask(with: request) { [weak self] result, error in
            DispatchQueue.main.async { self?.handle(result: result, error: error) }
        }
        input.installTap(onBus: 0, bufferSize: 1024, format: format) { buffer, _ in
            request.append(buffer)
        }
        tapInstalled = true
        audio.prepare()
        do {
            try audio.start()
        } catch {
            cancel()
            completion("无法打开麦克风：\(error.localizedDescription)")
            return
        }
        completion(nil)
    }

    /// 停止采集并等待系统给出最终结果；超时由语音页取消。
    func stop() {
        guard accepting else { return }
        accepting = false
        awaitingFinal = true
        // 先停采集/拆 tap，再 endAudio，避免音频线程在结束之后继续 append。
        stopAudio()
        request?.endAudio()
    }

    /// 立即取消并清理；可重复调用。
    func cancel() {
        accepting = false
        awaitingFinal = false
        task?.cancel()
        task = nil
        stopAudio()
        request?.endAudio()
        request = nil
    }

    private func handle(result: SFSpeechRecognitionResult?, error: Error?) {
        if let result {
            // 用户口径（2026-11-05）：语音上屏到光标处不能带任何符号；系统识别器
            // 会自动补标点，这里只保留字母/汉字/数字（词间留一个空格）。
            let text = Self.cleanSpeechText(result.bestTranscription.formattedString)
            if result.isFinal {
                let wasActive = accepting || awaitingFinal
                finishState()
                if wasActive { onEvent?(.final(text)) }
                return
            }
            if accepting { onEvent?(.partial(text)) }
        }
        if let error, accepting || awaitingFinal {
            finishState()
            onEvent?(.error(error.localizedDescription))
        }
    }

    private func finishState() {
        accepting = false
        awaitingFinal = false
        task = nil
        request = nil
        stopAudio()
    }

    private func stopAudio() {
        if audio.isRunning { audio.stop() }
        if tapInstalled {
            audio.inputNode.removeTap(onBus: 0)
            tapInstalled = false
        }
        try? AVAudioSession.sharedInstance().setActive(
            false, options: .notifyOthersOnDeactivation
        )
    }

    private static func cleanSpeechText(_ raw: String) -> String {
        var out = ""
        var pendingSpace = false
        for scalar in raw.unicodeScalars {
            if CharacterSet.alphanumerics.contains(scalar) {
                if pendingSpace && !out.isEmpty { out.append(" ") }
                out.unicodeScalars.append(scalar)
                pendingSpace = false
            } else if CharacterSet.whitespacesAndNewlines.contains(scalar) {
                pendingSpace = !out.isEmpty
            }
        }
        return out
    }

    private static func recognizer(for language: String) -> SFSpeechRecognizer? {
        let locale: Locale
        switch language {
        case "zh": locale = Locale(identifier: "zh-CN")
        case "en": locale = Locale(identifier: "en-US")
        default: locale = Locale.current
        }
        return SFSpeechRecognizer(locale: locale)
    }
}
