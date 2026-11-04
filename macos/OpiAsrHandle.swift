// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! opi-asr C 句柄薄层（从 OpiVoice.swift 拆出守 500 行门禁）。

import Foundation

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
