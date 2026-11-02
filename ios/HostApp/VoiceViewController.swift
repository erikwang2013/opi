// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 宿主 App 的语音页：16 kHz 单声道 PCM 采集 → crates/opi-asr → 回填键盘。
//
// 为什么采集在宿主：键盘扩展不请求麦克风、不创建 ASR 引擎（用户确认的方案）。
// 键盘的麦克风按钮通过 `opi-voice://record` 打开本页；识别完成把文本写进
// VoiceResultChannel（App Group 或带前缀的 UIPasteboard），键盘下次可见时插入。
//
// 录音只在内存里转成 PCM16 直接喂引擎，不落盘（与 crates/opi-asr 的隐私承诺一致）。
// ⚠️ 本机无 iOS SDK，未真机验证；以 Apple CI 编译 + 真机重签测试为准。

import AVFoundation
import UIKit

final class VoiceViewController: UIViewController {

    private let autoStart: Bool
    private let service = VoiceService.shared
    private let models = AsrModelManager.shared
    private let downloader = AsrModelDownloader.shared

    let scroll = UIScrollView()
    let stack = UIStackView()
    let statusLabel = UILabel()
    let detailLabel = UILabel()
    let progressView = UIProgressView(progressViewStyle: .default)
    let downloadButton = UIButton(type: .system)
    let micButton = UIButton(type: .system)
    let languageControl = UISegmentedControl(items: ["自动", "中文", "English"])
    let resultView = UITextView()

    private let recorder = VoiceRecorder()
    private var recording = false
    private var awaitingFinal = false
    private var didAutoStart = false
    private var wantsStart = false
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
            self, selector: #selector(serviceChanged),
            name: .opiVoiceServiceChanged, object: nil
        )
        NotificationCenter.default.addObserver(
            self, selector: #selector(appBackgrounded),
            name: UIApplication.didEnterBackgroundNotification, object: nil
        )
        refreshModelState()
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

    // MARK: - 模型状态 / 下载

    @objc private func serviceChanged() {
        refreshModelState()
        if wantsStart, !service.isLoading {
            wantsStart = false
            if service.loadedLanguage != nil {
                beginRecordingIfReady()
            } else {
                showError("语音模型加载失败，请重试")
            }
            return
        }
        if service.modelsRoot != nil, autoStart, !didAutoStart, !sessionActive {
            didAutoStart = true
            beginRecordingIfReady()
        }
    }

    private func refreshModelState() {
        if service.modelsRoot != nil {
            micButton.isEnabled = true
            downloadButton.isHidden = true
            progressView.isHidden = true
            if service.usesBundledModels {
                statusLabel.text = "完整包：语音模型已内置"
                detailLabel.text = "无需下载，直接可用。"
            } else if service.isLoading {
                statusLabel.text = "正在加载语音模型…"
                detailLabel.text = "首次加载需要几秒。"
            } else {
                statusLabel.text = "语音模型已就绪"
                detailLabel.text = "识别只在本机进行。"
            }
            return
        }
        micButton.isEnabled = false
        if downloader.isDownloading {
            downloadButton.isHidden = true
            progressView.isHidden = false
            statusLabel.text = "正在下载语音模型…"
            return
        }
        progressView.isHidden = true
        downloadButton.isHidden = false
        downloadButton.isEnabled = true
        downloadButton.setTitle(
            "下载语音模型（约 \(AsrModels.megabytes(AsrModels.totalBytes))）", for: .normal
        )
        statusLabel.text = "普通包不含语音模型"
        detailLabel.text = "首次使用需下载约 \(AsrModels.megabytes(AsrModels.totalBytes))"
            + "（SenseVoice + Whisper + VAD）。失败可重试。"
    }

    @objc func downloadModels() {
        guard !downloader.isDownloading else { return }
        let root: URL
        do {
            root = try models.prepareDownloadRoot()
        } catch {
            showError("无法创建模型目录：\(error.localizedDescription)")
            return
        }
        downloadButton.isEnabled = false
        downloadButton.isHidden = true
        progressView.isHidden = false
        progressView.progress = 0
        statusLabel.text = "正在下载语音模型…"
        detailLabel.text = "准备中…"
        downloader.start(
            root: root,
            files: AsrModels.allFiles,
            progress: { [weak self] fraction, name in
                guard let self else { return }
                self.progressView.progress = Float(fraction)
                let done = Int64(Double(AsrModels.totalBytes) * fraction)
                let phase = name.isEmpty ? "校验已有文件" : name
                self.detailLabel.text = "\(phase) · "
                    + "\(AsrModels.megabytes(done)) / \(AsrModels.megabytes(AsrModels.totalBytes))"
            },
            completion: { [weak self] error in
                guard let self else { return }
                if let error {
                    self.statusLabel.text = "模型下载失败，可重试"
                    self.detailLabel.text = error.localizedDescription
                    self.downloadButton.setTitle("重试下载", for: .normal)
                    self.downloadButton.isHidden = false
                    self.downloadButton.isEnabled = true
                    self.progressView.isHidden = true
                    return
                }
                self.progressView.progress = 1
                self.service.prewarm()
                self.refreshModelState()
            }
        )
    }

    // MARK: - 录音

    @objc func toggleRecording() {
        if sessionActive {
            if recording { stopAndFinish() }
        } else {
            beginRecordingIfReady()
        }
    }

    private func beginRecordingIfReady() {
        guard !sessionActive else { return }
        guard service.modelsRoot != nil else {
            statusLabel.text = "先下载语音模型"
            refreshModelState()
            return
        }
        if service.isLoading {
            wantsStart = true
            micButton.isEnabled = false
            statusLabel.text = "正在准备语音模型…"
            detailLabel.text = "首次加载需要几秒。"
            return
        }
        let language = selectedLanguage
        service.preferredLanguage = language
        statusLabel.text = "正在准备语音模型…"
        detailLabel.text = "首次加载需要几秒。"
        micButton.isEnabled = false
        service.prepare(language: language) { [weak self] error in
            guard let self else { return }
            self.micButton.isEnabled = true
            if let error {
                self.showError(error)
                return
            }
            self.requestMicrophone()
        }
    }

    private func requestMicrophone() {
        let session = AVAudioSession.sharedInstance()
        switch session.recordPermission {
        case .granted:
            startAudio()
        case .denied:
            showError("没有麦克风权限：请在系统设置 → 小欧输入法 中允许麦克风。")
        case .undetermined:
            session.requestRecordPermission { [weak self] granted in
                DispatchQueue.main.async {
                    guard let self else { return }
                    if granted { self.startAudio() } else {
                        self.showError("没有麦克风权限：请在系统设置 → 小欧输入法 中允许麦克风。")
                    }
                }
            }
        @unknown default:
            startAudio()
        }
    }

    private func startAudio() {
        let rc = service.engine.start()
        guard rc == 0 else {
            showError("无法开始识别（错误码 \(rc)）")
            return
        }
        do {
            try recorder.start { [weak self] data in
                self?.service.engine.feed(data)
            }
        } catch {
            _ = service.engine.cancel()
            recorder.stop()
            showError("无法打开麦克风：\(error.localizedDescription)")
            return
        }
        recording = true
        awaitingFinal = false
        resultView.text = ""
        micButton.setTitle("停止并识别", for: .normal)
        statusLabel.text = "正在聆听…"
        detailLabel.text = "说完点「停止并识别」。"
    }

    private func stopAndFinish() {
        guard recording else { return }
        recording = false
        awaitingFinal = true
        stopAudio()
        let rc = service.engine.finish()
        guard rc == 0 else {
            showError("结束识别失败（错误码 \(rc)）")
            return
        }
        statusLabel.text = "正在识别…"
        detailLabel.text = "请稍候。"
        micButton.isEnabled = false
        scheduleFinishTimeout()
    }

    private func stopAudio() {
        recorder.stop()
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

    // MARK: - 事件

    private func handle(_ event: AsrEvent) {
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            switch event {
            case .segment(let text):
                guard !text.isEmpty else { return }
                let existing = self.resultView.text ?? ""
                self.resultView.text = existing.isEmpty ? text : existing + "\n" + text
            case .final(let text):
                self.awaitingFinal = false
                self.finishTimeout?.cancel()
                self.finishTimeout = nil
                self.finish(with: text.trimmingCharacters(in: .whitespacesAndNewlines))
            case .error(let message):
                self.showError(message)
            case .level:
                break
            }
        }
    }

    private func finish(with text: String) {
        recording = false
        awaitingFinal = false
        stopAudio()
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
        finishTimeout?.cancel()
        finishTimeout = nil
        stopAudio()
        _ = service.engine.cancel()
        micButton.isEnabled = true
        micButton.setTitle("重试录音", for: .normal)
        statusLabel.text = "语音识别失败"
        detailLabel.text = message
    }

    @objc private func appBackgrounded() {
        abortSession(message: "录音已取消")
    }

    private func abortSession(message: String) {
        guard sessionActive else { return }
        recording = false
        awaitingFinal = false
        finishTimeout?.cancel()
        finishTimeout = nil
        stopAudio()
        _ = service.engine.cancel()
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
        if service.isLoading || sessionActive {
            detailLabel.text = "语言已切换；下一次录音会按所选语言加载模型。"
        }
    }
}
