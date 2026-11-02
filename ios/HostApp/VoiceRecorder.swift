// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 麦克风采集：AVAudioEngine 输入 tap → AVAudioConverter → 16 kHz 单声道 PCM16。
// 只喂回调，不碰 ASR、不落盘。权限由 VoiceViewController 先申请。

import AVFoundation
import Foundation

final class VoiceRecorder {
    private let engine = AVAudioEngine()
    private var converter: AVAudioConverter?
    private var tapInstalled = false
    private var onPCM: ((Data) -> Void)?

    deinit { stop() }

    /// 会设置 AVAudioSession `.record` 并启动引擎；失败抛错且已清理。
    func start(onPCM: @escaping (Data) -> Void) throws {
        let session = AVAudioSession.sharedInstance()
        try session.setCategory(.record, mode: .measurement, options: [])
        try session.setActive(true)

        let input = engine.inputNode
        let inputFormat = input.inputFormat(forBus: 0)
        guard inputFormat.sampleRate > 0,
              let outputFormat = AVAudioFormat(
                  commonFormat: .pcmFormatInt16,
                  sampleRate: 16000,
                  channels: 1,
                  interleaved: true
              ),
              let converter = AVAudioConverter(from: inputFormat, to: outputFormat) else {
            throw NSError(
                domain: "opi.voice", code: 1,
                userInfo: [NSLocalizedDescriptionKey: "麦克风格式不可用"]
            )
        }
        self.converter = converter
        self.onPCM = onPCM
        input.installTap(onBus: 0, bufferSize: 1024, format: inputFormat) {
            [weak self] buffer, _ in
            self?.feed(buffer)
        }
        tapInstalled = true
        engine.prepare()
        do {
            try engine.start()
        } catch {
            stop()
            throw error
        }
    }

    func stop() {
        if engine.isRunning { engine.stop() }
        if tapInstalled {
            engine.inputNode.removeTap(onBus: 0)
            tapInstalled = false
        }
        converter = nil
        onPCM = nil
        try? AVAudioSession.sharedInstance().setActive(
            false, options: .notifyOthersOnDeactivation
        )
    }

    private func feed(_ buffer: AVAudioPCMBuffer) {
        guard let converter, let onPCM else { return }
        let ratio = converter.outputFormat.sampleRate / converter.inputFormat.sampleRate
        let capacity = AVAudioFrameCount(Double(buffer.frameLength) * ratio) + 64
        guard let output = AVAudioPCMBuffer(
            pcmFormat: converter.outputFormat, frameCapacity: capacity
        ) else { return }
        var provided = false
        var conversionError: NSError?
        let status = converter.convert(to: output, error: &conversionError) {
            _, inputStatus in
            if provided {
                inputStatus.pointee = .noDataNow
                return nil
            }
            provided = true
            inputStatus.pointee = .haveData
            return buffer
        }
        if status == .error { return }
        guard output.frameLength > 0, let samples = output.int16ChannelData?[0] else { return }
        let data = Data(
            bytes: samples, count: Int(output.frameLength) * MemoryLayout<Int16>.size
        )
        onPCM(data)
    }
}
