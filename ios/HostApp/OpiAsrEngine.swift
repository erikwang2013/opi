// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// crates/opi-asr C ABI 的 Swift 薄包装（只有宿主 App 用；键盘扩展不链它）。
//
// 线程契约（见 crates/opi-asr/src/ffi.rs）：
//   · feed_pcm / start / finish / cancel / poll_event 可从任意线程调；
//   · load_model 阻塞到模型校验并创建识别器完成；
//   · destroy 必须是最后一个调用，不能与其它调用并发。
// 因此：load 在后台队列；poll 在自己队列；destroy 先停表再销毁，调用方保证
// 此时录音已停（VoiceViewController 的 stopAudio 在 cancel/reload 之前）。

import Foundation

enum AsrEvent {
    case level(Float)
    case segment(String)
    case final(String)
    case error(String)
}

final class OpiAsrEngine {
    private var handle: UnsafeMutableRawPointer?
    private var timer: DispatchSourceTimer?
    private let pollQueue = DispatchQueue(label: "xyz.erik.opi.asr.poll")
    private var eventBuffer: [CChar]

    /// 回调在主线程。`load` 期间的事件也会走这里（此时通常还没有 UI 监听）。
    var onEvent: ((AsrEvent) -> Void)?

    init() {
        eventBuffer = [CChar](
            repeating: 0,
            count: Int(opi_asr_max_event_bytes()) + 1 // +1 给 NUL；事件字节数上限见 ffi.rs
        )
    }

    deinit { destroy() }

    var isLoaded: Bool { handle != nil }

    /// 成功返回 nil，失败返回给用户看的错误。会先销毁旧引擎。
    func load(modelsRoot: URL, language: String) -> String? {
        destroy()
        let config: [String: Any] = [
            "models_dir": modelsRoot.path,
            "sense_voice_dir": "sensevoice",
            "whisper_dir": "whisper-small.en",
            "vad_model": "vad/silero_vad.onnx",
            "language": language,
            "num_threads": 2,
            // 下载/打包时已逐文件校验 SHA-256；设备上不再重复哈希 615MB。
            "verify_models": false,
            "max_segment_ms": 30000,
            "vad_min_silence_ms": 500,
            "vad_threshold": 0.5,
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: config),
              let json = String(data: data, encoding: .utf8) else {
            return "语音配置序列化失败"
        }
        guard let created = json.withCString({ opi_asr_create($0) }) else {
            return Self.lastError(handle: nil) ?? "语音引擎创建失败"
        }
        let rc = opi_asr_load_model(created)
        guard rc == 0 else {
            let message = Self.lastError(handle: created) ?? "模型加载失败（错误码 \(rc)）"
            opi_asr_destroy(created)
            return message
        }
        handle = created
        startPolling()
        return nil
    }

    @discardableResult
    func start() -> Int32 {
        guard let handle else { return -4 } // ERR_STATE
        return opi_asr_start(handle)
    }

    func feed(_ data: Data) {
        guard let handle, !data.isEmpty else { return }
        _ = data.withUnsafeBytes { raw in
            opi_asr_feed_pcm(
                handle,
                raw.bindMemory(to: UInt8.self).baseAddress,
                Int32(data.count)
            )
        }
        // -5 (busy) 表示 32 块队列满：丢掉这一小块，下一次回调继续送。
    }

    @discardableResult
    func finish() -> Int32 {
        guard let handle else { return -4 }
        return opi_asr_finish(handle)
    }

    @discardableResult
    func cancel() -> Int32 {
        guard let handle else { return -4 }
        return opi_asr_cancel(handle)
    }

    /// 停轮询并销毁。必须是最后一次调用。
    func destroy() {
        pollQueue.sync {
            timer?.cancel()
            timer = nil
            if let current = handle {
                opi_asr_destroy(current)
                handle = nil
            }
        }
    }

    private func startPolling() {
        pollQueue.sync {
            guard handle != nil else { return }
            let source = DispatchSource.makeTimerSource(queue: pollQueue)
            source.schedule(deadline: .now() + .milliseconds(100), repeating: .milliseconds(100))
            source.setEventHandler { [weak self] in self?.pollOnce() }
            source.resume()
            timer = source
        }
    }

    private func pollOnce() {
        guard let handle else { return }
        let written = opi_asr_poll_event(handle, &eventBuffer, Int32(eventBuffer.count))
        guard written > 0 else { return }
        let decoded = eventBuffer.withUnsafeBufferPointer { buffer in
            buffer.baseAddress.flatMap { String(validatingUTF8: $0) }
        }
        guard let text = decoded,
              let data = text.data(using: .utf8),
              let parsed = try? JSONSerialization.jsonObject(with: data),
              let object = parsed as? [String: Any],
              let type = object["type"] as? String else { return }

        let event: AsrEvent?
        switch type {
        case "level":
            event = .level((object["rms_db"] as? NSNumber)?.floatValue ?? -60)
        case "segment":
            event = .segment(object["text"] as? String ?? "")
        case "final":
            event = .final(object["text"] as? String ?? "")
        case "error":
            event = .error(object["message"] as? String ?? "语音识别失败")
        default:
            event = nil
        }
        guard let event else { return }
        DispatchQueue.main.async { [weak self] in self?.onEvent?(event) }
    }

    private static func lastError(handle: UnsafeMutableRawPointer?) -> String? {
        var buffer = [CChar](repeating: 0, count: 1024)
        let written = opi_asr_last_error(handle, &buffer, Int32(buffer.count))
        guard written > 0 else { return nil }
        return buffer.withUnsafeBufferPointer { pointer in
            pointer.baseAddress.flatMap { String(validatingUTF8: $0) }
        }
    }
}
