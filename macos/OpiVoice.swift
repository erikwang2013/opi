// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// OpiVoice.swift —— macOS 进程内语音输入（2026-11-02）。
//
// 与 iOS 不同，输入法进程可以自己开麦克风：AVAudioEngine 采集 → AVAudioConverter
// 转 16 kHz 单声道 PCM16 → crates/opi-asr 的 C ABI（macos/OpiAsrFFI.h 声明）。
// 识别结果走 `OpiClientHub.commit`（与符号面板同一条上屏通道）。
//
// 生命周期：activateServer/启动时后台预热已存在的模型（不阻塞 UI）；点状态栏
// 麦克风才下载/录音。音频只在内存里，不落盘、不写日志；错误只进状态栏。
//
// 线程：状态与 UI 都在 main；ASR 的 create/load/finish/cancel 串行在 voice 队列；
// `feed_pcm` 由音频线程直接调用（C ABI 允许，除 destroy 外可跨线程）。destroy
// 只发生在 voice 队列（OpiAsrHandle 的 deinit），且此时没有录音。

import AppKit
import AVFoundation

enum OpiVoiceState {
    case idle
    case downloading
    case loading
    case recording
    case transcribing
    case error
}

/// opi-asr 的薄桥。句柄由 Rust 分配，destroy 必须最后调用且不能与其它调用并发。
final class OpiAsrHandle {
    private var raw: UnsafeMutableRawPointer?

    init(configJSON: String) throws {
        raw = configJSON.withCString { opi_asr_create($0) }
        guard raw != nil else {
            throw OpiVoiceError.message("语音引擎创建失败：\(OpiAsrHandle.errorText(nil))")
        }
    }

    deinit {
        if let raw { opi_asr_destroy(raw) }
    }

    func loadModel() -> Int32 { opi_asr_load_model(raw) }
    func start() -> Int32 { opi_asr_start(raw) }
    func finish() -> Int32 { opi_asr_finish(raw) }
    func cancel() -> Int32 { opi_asr_cancel(raw) }

    func feed(_ samples: UnsafePointer<Int16>, count: Int) -> Int32 {
        let bytes = count * MemoryLayout<Int16>.size
        return samples.withMemoryRebound(to: UInt8.self, capacity: bytes) {
            opi_asr_feed_pcm(raw, $0, Int32(bytes))
        }
    }

    /// 非阻塞。返回 nil = 当前没有事件。
    func poll() -> [String: Any]? {
        let cap = Int(opi_asr_max_event_bytes())
        guard cap > 0 else { return nil }
        var buffer = [CChar](repeating: 0, count: cap)
        let length = buffer.withUnsafeMutableBufferPointer {
            opi_asr_poll_event(raw, $0.baseAddress, Int32(cap))
        }
        guard length > 0 else { return nil }
        let json = buffer.withUnsafeBufferPointer { String(cString: $0.baseAddress!) }
        guard let data = json.data(using: .utf8) else { return nil }
        return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
    }

    func lastError() -> String { OpiAsrHandle.errorText(raw) }

    private static func errorText(_ handle: UnsafeMutableRawPointer?) -> String {
        let cap = 4096
        var buffer = [CChar](repeating: 0, count: cap)
        let length = buffer.withUnsafeMutableBufferPointer {
            opi_asr_last_error(handle, $0.baseAddress, Int32(cap))
        }
        guard length > 0 else { return "未知错误" }
        return buffer.withUnsafeBufferPointer { String(cString: $0.baseAddress!) }
    }
}

final class OpiVoice {
    static let shared = OpiVoice()

    /// true = 用户点过麦克风；normal 包第一次点击会顺带下载模型。
    static var enabled: Bool {
        get { OpiSettingsStore.bool("VoiceEnabled", default: false) }
        set { OpiSettingsStore.set("VoiceEnabled", newValue) }
    }

    /// 当前输入模式对应的引擎（与 Rust 的 zh/auto -> SenseVoice、en -> Whisper 同判）。
    static var engine: OpiVoiceEngine {
        OpiEngine.shared.mode() == .english ? .whisper : .sensevoice
    }

    private let queue = DispatchQueue(label: "xyz.erik.opi.voice", qos: .utility)
    private let audio = AVAudioEngine()
    private var handle: OpiAsrHandle?
    private var loadedEngine: OpiVoiceEngine?
    private var converter: AVAudioConverter?
    private var tapInstalled = false
    private var captureLock = NSLock()
    private var capturing = false
    private var pollTimer: Timer?
    private var timeoutTimer: Timer?
    private var cancelLock = NSLock()
    private var downloadGeneration = 0
    private var lastProgressAt = Date.distantPast

    private(set) var state: OpiVoiceState = .idle
    private(set) var progress: Double = 0
    private(set) var lastError = ""

    private init() {}

    // ---------- 状态栏文案 ----------

    var badge: String? {
        switch state {
        case .idle: return nil
        case .downloading: return "\(Int((progress * 100).rounded()))"
        case .loading, .transcribing: return "..."
        case .recording: return "●"
        case .error: return "!"
        }
    }

    var tooltip: String {
        switch state {
        case .idle:
            if !lastError.isEmpty { return "语音输入不可用：\(lastError)；单击重试" }
            if OpiVoiceModels.readyRoot(OpiVoice.engine) == nil {
                let mb = Int(OpiVoice.engine.files.reduce(Int64(0)) { $0 + $1.size } + OpiVoiceModels.vad.size) / (1024 * 1024)
                return "语音输入：单击下载模型并启用（约 \(mb) MB，本地识别）"
            }
            return "语音输入：单击开始录音"
        case .downloading: return "正在下载语音模型 \(Int(progress * 100))%：单击取消"
        case .loading: return "正在加载语音模型，请稍候"
        case .recording: return "正在录音：单击停止，Option 单击取消"
        case .transcribing: return "正在识别：单击取消"
        case .error: return "语音输入出错：\(lastError)；单击重试"
        }
    }

    // ---------- 预热 / 生命周期 ----------

    /// 输入源激活或进程启动时调用。只预热已经存在的模型；normal 包的下载由
    /// 用户第一次点麦克风触发，不在激活时偷偷跑流量。
    func prewarmIfNeeded() {
        guard state == .idle else { return }
        let engine = OpiVoice.engine
        guard let root = OpiVoiceModels.readyRoot(engine) else { return }
        if handle != nil, loadedEngine == engine { return }
        load(engine: engine, root: root, quiet: true) { _ in }
    }

    /// 输入源失焦时取消正在进行的录音/识别（隐私优先；下载不受影响）。
    func inputDeactivated() {
        if state == .recording || state == .transcribing { cancelSession() }
    }

    // ---------- 状态栏动作 ----------

    func toggle(cancel: Bool = false) {
        if Thread.isMainThread { handleToggle(cancel) }
        else { DispatchQueue.main.async { self.handleToggle(cancel) } }
    }

    private func handleToggle(_ cancel: Bool) {
        switch state {
        case .recording:
            if cancel { cancelSession() } else { stopRecording() }
        case .transcribing:
            cancelSession()
        case .downloading:
            _ = bumpDownloadGeneration() // 旧下载的进度/完成回调全部作废
            state = .idle
            progress = 0
            refreshBar()
        case .loading:
            OpiStatusBar.shared.showNotice("语音模型加载中，请稍候")
        case .idle, .error:
            state = .idle
            lastError = ""
            begin()
        }
    }

    private func begin() {
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized:
            prepareAndRecord()
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .audio) { [weak self] granted in
                DispatchQueue.main.async {
                    guard let self else { return }
                    if granted { self.prepareAndRecord() }
                    else { self.fail("麦克风权限被拒绝：系统设置 → 隐私与安全性 → 麦克风") }
                }
            }
        case .denied, .restricted:
            fail("麦克风权限未开启：系统设置 → 隐私与安全性 → 麦克风")
        @unknown default:
            fail("无法获取麦克风权限")
        }
    }

    private func prepareAndRecord() {
        OpiVoice.enabled = true
        let engine = OpiVoice.engine
        if let root = OpiVoiceModels.readyRoot(engine) {
            load(engine: engine, root: root, quiet: false) { [weak self] ok in
                if ok { self?.startRecording() }
            }
            return
        }

        state = .downloading
        progress = 0
        let generation = bumpDownloadGeneration()
        refreshBar()
        queue.async { [weak self] in
            guard let self else { return }
            do {
                let root = try OpiVoiceModels.ensure(
                    engine,
                    isCancelled: { !self.isCurrentGeneration(generation) },
                    progress: { p in self.publish(progress: p, generation: generation) })
                DispatchQueue.main.async {
                    guard self.isCurrentGeneration(generation) else { return }
                    self.state = .idle
                    self.progress = 1
                    self.refreshBar()
                    OpiStatusBar.shared.showNotice("语音模型已就绪，点击麦克风开始")
                    self.load(engine: engine, root: root, quiet: true) { _ in }
                }
            } catch OpiVoiceError.cancelled {
                DispatchQueue.main.async {
                    guard self.isCurrentGeneration(generation) else { return }
                    self.state = .idle
                    self.progress = 0
                    self.refreshBar()
                }
            } catch {
                DispatchQueue.main.async {
                    guard self.isCurrentGeneration(generation) else { return }
                    self.fail("语音模型下载失败：\(error.localizedDescription)")
                }
            }
        }
    }

    /// 后台建句柄并装载模型。quiet = 预热：失败只留可读状态（tooltip），不弹提示。
    private func load(engine: OpiVoiceEngine, root: URL, quiet: Bool, completion: @escaping (Bool) -> Void) {
        guard state == .idle else { completion(false); return }
        if handle != nil, loadedEngine == engine { completion(true); return } // 复用句柄，不等重载
        guard let config = Self.configJSON(engine: engine, root: root) else {
            if quiet { lastError = "语音配置生成失败" } else { fail("语音配置生成失败") }
            completion(false)
            return
        }
        state = .loading
        refreshBar()
        queue.async { [weak self] in
            guard let self else { return }
            let old = self.handle
            do {
                let new = try OpiAsrHandle(configJSON: config)
                let rc = new.loadModel()
                guard rc == 0 else {
                    throw OpiVoiceError.message("语音模型加载失败：\(new.lastError())")
                }
                self.handle = new
                self.loadedEngine = engine
                _ = old?.cancel()
                DispatchQueue.main.async {
                    self.state = .idle
                    self.lastError = ""
                    self.refreshBar()
                    completion(true)
                }
            } catch {
                DispatchQueue.main.async {
                    self.state = .idle
                    if quiet {
                        self.lastError = error.localizedDescription
                        self.refreshBar()
                    } else {
                        self.fail(error.localizedDescription)
                    }
                    completion(false)
                }
            }
        }
    }

    // ---------- 录音 ----------

    private func startRecording() {
        guard let handle else { fail("语音引擎未就绪"); return }
        let rc = handle.start()
        guard rc == 0 else { fail("语音引擎启动失败（\(rc)）"); return }
        do {
            try startAudio()
        } catch {
            _ = handle.cancel()
            fail("麦克风启动失败：\(error.localizedDescription)")
            return
        }
        state = .recording
        lastError = ""
        refreshBar()
        startPolling()
    }

    private func startAudio() throws {
        let input = audio.inputNode
        let inputFormat = input.outputFormat(forBus: 0)
        guard inputFormat.sampleRate > 0, inputFormat.channelCount > 0 else {
            throw OpiVoiceError.message("没有可用的麦克风输入")
        }
        guard let outputFormat = AVAudioFormat(commonFormat: .pcmFormatInt16,
                                               sampleRate: 16_000, channels: 1,
                                               interleaved: true),
              let conv = AVAudioConverter(from: inputFormat, to: outputFormat) else {
            throw OpiVoiceError.message("无法把麦克风音频转换为 16 kHz 单声道")
        }
        conv.primeMethod = .none
        converter = conv
        captureLock.lock(); capturing = true; captureLock.unlock()
        input.installTap(onBus: 0, bufferSize: 4096, format: inputFormat) { [weak self] buffer, _ in
            self?.feed(buffer)
        }
        tapInstalled = true
        do {
            audio.prepare()
            try audio.start()
        } catch {
            input.removeTap(onBus: 0)
            tapInstalled = false
            converter = nil
            captureLock.lock(); capturing = false; captureLock.unlock()
            throw error
        }
    }

    /// 音频线程入口。convert 的输入块只喂一帧；feed_pcm 非阻塞，队列满就丢。
    private func feed(_ buffer: AVAudioPCMBuffer) {
        captureLock.lock(); let on = capturing; let conv = converter; captureLock.unlock()
        guard on, let conv, let handle else { return }
        let ratio = 16_000.0 / buffer.format.sampleRate
        let capacity = AVAudioFrameCount(Double(buffer.frameLength) * ratio) + 64
        guard capacity > 0, let output = AVAudioPCMBuffer(pcmFormat: conv.outputFormat, frameCapacity: capacity) else { return }
        var error: NSError?
        var supplied = false
        let status = conv.convert(to: output, error: &error) { _, inputStatus in
            if supplied {
                inputStatus.pointee = .noDataNow
                return nil
            }
            supplied = true
            inputStatus.pointee = .haveData
            return buffer
        }
        guard status != .error, output.frameLength > 0,
              let samples = output.int16ChannelData?[0] else { return }
        _ = handle.feed(samples, count: Int(output.frameLength))
    }

    private func stopRecording() {
        guard state == .recording else { return }
        state = .transcribing
        refreshBar()
        stopAudio()
        let current = handle
        queue.async { _ = current?.finish() }
        timeoutTimer?.invalidate()
        timeoutTimer = Timer.scheduledTimer(withTimeInterval: 20, repeats: false) { [weak self] _ in
            self?.fail("语音识别超时")
        }
    }

    private func cancelSession() {
        invalidateTimers()
        stopAudio()
        state = .idle
        progress = 0
        refreshBar()
        let current = handle
        queue.async { _ = current?.cancel() }
    }

    private func stopAudio() {
        captureLock.lock(); capturing = false; captureLock.unlock()
        if tapInstalled {
            audio.inputNode.removeTap(onBus: 0)
            tapInstalled = false
        }
        audio.stop()
        converter = nil
    }

    // ---------- 事件 ----------

    private func startPolling() {
        pollTimer?.invalidate()
        let timer = Timer(timeInterval: 0.1, repeats: true) { [weak self] _ in self?.poll() }
        RunLoop.main.add(timer, forMode: .common)
        pollTimer = timer
    }

    private func poll() {
        while let event = handle?.poll() {
            switch event["type"] as? String {
            case "final":
                let text = (event["text"] as? String ?? "")
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                invalidateTimers()
                state = .idle
                lastError = ""
                refreshBar()
                if text.isEmpty {
                    OpiStatusBar.shared.showNotice("没有识别到语音内容")
                } else {
                    OpiClientHub.shared.commit(text)
                }
                return
            case "error":
                fail("语音识别失败：\(event["message"] as? String ?? "未知错误")")
                return
            default:
                break
            }
        }
    }

    private func invalidateTimers() {
        pollTimer?.invalidate(); pollTimer = nil
        timeoutTimer?.invalidate(); timeoutTimer = nil
    }

    private func fail(_ message: String) {
        invalidateTimers()
        stopAudio()
        state = .error
        lastError = message
        refreshBar()
        OpiStatusBar.shared.showNotice(message)
        let current = handle
        queue.async { _ = current?.cancel() }
    }

    // ---------- 下载进度 ----------

    private func publish(progress value: Double, generation: Int) {
        DispatchQueue.main.async { [weak self] in
            guard let self, self.state == .downloading,
                  self.isCurrentGeneration(generation) else { return }
            let now = Date()
            guard now.timeIntervalSince(self.lastProgressAt) >= 0.3 || value >= 0.999 else { return }
            self.lastProgressAt = now
            self.progress = min(max(value, 0), 1)
            self.refreshBar()
        }
    }

    private func bumpDownloadGeneration() -> Int {
        cancelLock.lock(); downloadGeneration += 1
        let value = downloadGeneration
        cancelLock.unlock()
        return value
    }

    private func isCurrentGeneration(_ value: Int) -> Bool {
        cancelLock.lock(); defer { cancelLock.unlock() }
        return downloadGeneration == value
    }

    private func refreshBar() {
        if Thread.isMainThread { OpiStatusBar.shared.refresh() }
        else { DispatchQueue.main.async { OpiStatusBar.shared.refresh() } }
    }

    // ---------- 配置 ----------

    private static func configJSON(engine: OpiVoiceEngine, root: URL) -> String? {
        let config: [String: Any] = [
            "models_dir": root.path,
            "sense_voice_dir": "sensevoice", "whisper_dir": "whisper-small.en",
            "vad_model": "vad/silero_vad.onnx", "language": engine.language,
            "num_threads": 2, "verify_models": true,
            "max_segment_ms": 30_000, "vad_min_silence_ms": 500,
            "vad_threshold": 0.5,
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: config) else { return nil }
        return String(data: data, encoding: .utf8)
    }
}
