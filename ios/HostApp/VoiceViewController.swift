// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 宿主 App 的语音页：系统 SFSpeechRecognizer 本地识别 → 回填键盘。
//
// 键盘扩展不请求麦克风、不创建识别任务（用户确认的方案）。键盘的麦克风按钮通过
// `opi-voice://record` 打开本页；识别完成把文本写进 VoiceResultChannel
// （App Group 或带前缀的 UIPasteboard），键盘下次可见时插入。
//
// 音频只在内存里经 AVAudioEngine tap 追加到识别请求，不落盘；请求设置
// `requiresOnDeviceRecognition = true`，没有服务器回退。
// ⚠️ 本机无 iOS SDK，未真机验证；以 Apple CI 编译 + 真机重签测试为准。

import AVFoundation
import Speech
import UIKit

final class VoiceViewController: UIViewController {

    private let autoStart: Bool
    private let service = VoiceService.shared

    let scroll = UIScrollView()
    let stack = UIStackView()
    let statusLabel = UILabel()
    let detailLabel = UILabel()
    let micButton = UIButton(type: .system)
    let languageControl = UISegmentedControl(items: ["自动", "中文", "English"])
    let resultView = UITextView()

    private var recording = false
    private var awaitingFinal = false
    private var starting = false
    private var didAutoStart = false
    private var finishTimeout: DispatchWorkItem?
    private var sessionActive: Bool { recording || awaitingFinal }

    init(autoStart: Bool) {
        self.autoStart = autoStart
        super.init(nibName: nil, bundle: nil)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 全代码构建") }
    deinit {
        NotificationCenter.default.removeObserver(self)
        finishTimeout?.cancel()
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "语音输入"
        view.backgroundColor = .systemGroupedBackground
        buildScroll()
        buildContent()
        languageControl.selectedSegmentIndex = index(for: service.preferredLanguage)
        languageControl.addTarget(self, action: #selector(languageChanged), for: .valueChanged)
        service.engine.onEvent = { [weak self] event in self?.handle(event) }
        NotificationCenter.default.addObserver(
            self, selector: #selector(appBackgrounded),
            name: UIApplication.didEnterBackgroundNotification, object: nil
        )
        statusLabel.text = "系统本地语音识别"
        detailLabel.text = "识别在本机完成；音频只在内存中处理，不落盘、不上传。"
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        guard autoStart, !didAutoStart else { return }
        didAutoStart = true
        beginRecordingIfReady()
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        abortSession(message: "录音已取消")
    }

    // MARK: - 权限与录音

    @objc func toggleRecording() {
        if sessionActive {
            if recording { stopAndFinish() }
        } else if !starting {
            beginRecordingIfReady()
        }
    }

    private func beginRecordingIfReady() {
        guard !sessionActive, !starting else { return }
        starting = true
        micButton.isEnabled = false
        service.preferredLanguage = selectedLanguage
        statusLabel.text = "正在请求语音识别权限…"
        detailLabel.text = "首次使用会请求语音识别与麦克风权限。"
        OpiSpeechEngine.requestSpeechAuthorization { [weak self] status in
            guard let self, self.starting else { return }
            switch status {
            case .authorized:
                self.requestMicrophone()
            case .denied:
                self.failStart("没有语音识别权限：请在系统设置 → 小欧输入法 中允许「语音识别」。")
            case .restricted:
                self.failStart("系统限制了语音识别（家长控制或设备策略）。")
            case .notDetermined:
                self.failStart("语音识别权限未授予，请重试。")
            @unknown default:
                self.failStart("语音识别权限状态未知。")
            }
        }
    }

    private func requestMicrophone() {
        guard starting else { return }
        let session = AVAudioSession.sharedInstance()
        switch session.recordPermission {
        case .granted:
            startAudio()
        case .denied:
            failStart("没有麦克风权限：请在系统设置 → 小欧输入法 中允许麦克风。")
        case .undetermined:
            session.requestRecordPermission { [weak self] granted in
                DispatchQueue.main.async {
                    guard let self, self.starting else { return }
                    if granted {
                        self.startAudio()
                    } else {
                        self.failStart(
                            "没有麦克风权限：请在系统设置 → 小欧输入法 中允许麦克风。"
                        )
                    }
                }
            }
        @unknown default:
            startAudio()
        }
    }

    private func startAudio() {
        statusLabel.text = "正在开始…"
        detailLabel.text = "请稍候。"
        // start 在主线程同步回调；guard 防页面在权限/启动之间被关闭后误开录音。
        service.engine.start(language: selectedLanguage) { [weak self] error in
            guard let self, self.starting else { return }
            self.starting = false
            self.micButton.isEnabled = true
            if let error {
                self.failStart(error)
                return
            }
            self.recording = true
            self.awaitingFinal = false
            self.resultView.text = ""
            self.micButton.setTitle("停止并识别", for: .normal)
            self.statusLabel.text = "正在聆听…"
            self.detailLabel.text = "说完点「停止并识别」。"
        }
    }

    private func stopAndFinish() {
        guard recording else { return }
        recording = false
        awaitingFinal = true
        service.engine.stop()
        statusLabel.text = "正在识别…"
        detailLabel.text = "请稍候。"
        micButton.isEnabled = false
        scheduleFinishTimeout()
    }

    private func scheduleFinishTimeout() {
        finishTimeout?.cancel()
        let work = DispatchWorkItem { [weak self] in
            guard let self, self.awaitingFinal else { return }
            self.showError("识别超时，请重试。")
        }
        finishTimeout = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 45, execute: work)
    }

    private func failStart(_ message: String) {
        starting = false
        micButton.isEnabled = true
        micButton.setTitle("重试录音", for: .normal)
        statusLabel.text = "语音识别不可用"
        detailLabel.text = message
    }

    // MARK: - 事件

    private func handle(_ event: OpiSpeechEvent) {
        switch event {
        case .partial(let text):
            guard !text.isEmpty else { return }
            resultView.text = text
        case .final(let text):
            finish(with: text.trimmingCharacters(in: .whitespacesAndNewlines))
        case .error(let message):
            showError(message)
        }
    }

    private func finish(with text: String) {
        recording = false
        awaitingFinal = false
        starting = false
        finishTimeout?.cancel()
        finishTimeout = nil
        micButton.isEnabled = true
        micButton.setTitle("重新录音", for: .normal)
        guard !text.isEmpty else {
            statusLabel.text = "没有识别到语音"
            detailLabel.text = "请靠近麦克风、放慢语速后重试。"
            return
        }
        resultView.text = text
        VoiceResultChannel.store(text)
        statusLabel.text = "已回填到键盘"
        detailLabel.text = "回到输入界面，键盘出现时会自动插入。"
            + "若没有插入，可在这里长按复制。"
    }

    private func showError(_ message: String) {
        recording = false
        awaitingFinal = false
        starting = false
        finishTimeout?.cancel()
        finishTimeout = nil
        service.engine.cancel()
        micButton.isEnabled = true
        micButton.setTitle("重试录音", for: .normal)
        statusLabel.text = "语音识别失败"
        detailLabel.text = message
    }

    @objc private func appBackgrounded() {
        abortSession(message: "录音已取消")
    }

    private func abortSession(message: String) {
        guard sessionActive || starting else { return }
        recording = false
        awaitingFinal = false
        starting = false
        finishTimeout?.cancel()
        finishTimeout = nil
        service.engine.cancel()
        micButton.isEnabled = true
        micButton.setTitle("重新录音", for: .normal)
        statusLabel.text = message
        detailLabel.text = "需要时再点一次「重新录音」。"
    }

    // MARK: - 语言

    private var selectedLanguage: String {
        switch languageControl.selectedSegmentIndex {
        case 1: return "zh"
        case 2: return "en"
        default: return "auto"
        }
    }

    private func index(for language: String) -> Int {
        switch language {
        case "zh": return 1
        case "en": return 2
        default: return 0
        }
    }

    @objc private func languageChanged() {
        service.preferredLanguage = selectedLanguage
        if sessionActive || starting {
            detailLabel.text = "语言已切换；下一次录音会按所选语言识别。"
        }
    }
}
