// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// OpiVoiceModels.swift —— 语音模型的目录、校验与逐文件下载（2026-11-02）。
//
// 文件清单（size + SHA-256）与 desktop/…/SpeechModels.kt、Android 下载器、
// 完整包打包脚本共用同一张表。落点：
//   full 包：Bundle.main.resourceURL/asr（由打包脚本拷入）
//   normal ：~/Library/Application Support/opi/asr
// 下载只写用户目录；音频从不落盘。manifest.json 的 schema 是
// crates/opi-asr/src/manifest.rs 的 ModelManifest（role 绑定模型文件）。
//
// URL：https://opi.erik.xyz/download/components/asr/<模型目录>/<文件>，
// VAD 在 vad/silero_vad.onnx。每个文件下载后先核对 size 与 SHA-256，再原子改名；
// 失败重试 3 次。不引第三方依赖（CryptoKit 是系统框架）。

import CryptoKit
import Foundation

enum OpiVoiceError: LocalizedError {
    case message(String)
    case cancelled

    var errorDescription: String? {
        switch self {
        case .message(let text): return text
        case .cancelled: return "已取消"
        }
    }
}

/// 语言 → 引擎的路由与 crates/opi-asr/src/config.rs 一致：
/// auto/zh -> SenseVoice，en -> Whisper。
enum OpiVoiceEngine: String {
    case sensevoice
    case whisper

    struct ModelFile {
        let name: String
        let size: Int64
        let sha256: String
        let role: String
    }

    var dirName: String {
        self == .whisper ? "whisper-small.en" : "sensevoice"
    }

    /// AsrConfig.language。SenseVoice 自带语言识别，Whisper 固定英文。
    var language: String { self == .whisper ? "en" : "auto" }

    var files: [ModelFile] {
        switch self {
        case .sensevoice:
            return [
                ModelFile(name: "model.int8.onnx", size: 239_233_841,
                          sha256: "c71f0ce00bec95b07744e116345e33d8cbbe08cef896382cf907bf4b51a2cd51",
                          role: "model"),
                ModelFile(name: "tokens.txt", size: 315_894,
                          sha256: "f449eb28dc567533d7fa59be34e2abca8784f771850c78a47fb731a31429a1dc",
                          role: "tokens"),
            ]
        case .whisper:
            return [
                ModelFile(name: "small.en-encoder.int8.onnx", size: 112_442_483,
                          sha256: "8bdac288f369aa94ee2194059238c465ed82ea9d47ee8fa4a8c0a891873e462f",
                          role: "encoder"),
                ModelFile(name: "small.en-decoder.int8.onnx", size: 262_223_042,
                          sha256: "710ccf890e10f3faa15f51ec346081a2723c9f3adb6e4da81c6573a5a6f877fb",
                          role: "decoder"),
                ModelFile(name: "small.en-tokens.txt", size: 835_554,
                          sha256: "306cd27f03c1a714eca7108e03d66b7dc042abe8c258b44c199a7ed9838dd930",
                          role: "tokens"),
            ]
        }
    }
}

enum OpiVoiceModels {
    /// 站点按文件分发目录；完整包不经过这里。
    static let baseURL = URL(string: "https://opi.erik.xyz/download/components/asr/")!
    static let vad = OpiVoiceEngine.ModelFile(
        name: "silero_vad.onnx", size: 643_854,
        sha256: "9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6",
        role: "vad")
    static let attempts = 3

    static var userRoot: URL { OpiEngine.dataDirectory.appendingPathComponent("asr") }

    static var bundleRoot: URL? {
        guard let resources = Bundle.main.resourceURL else { return nil }
        let url = resources.appendingPathComponent("asr")
        return FileManager.default.fileExists(atPath: url.path) ? url : nil
    }

    /// 可加载的模型根：包内优先（full），其次用户目录（normal 下载）。
    static func readyRoot(_ engine: OpiVoiceEngine) -> URL? {
        for root in [bundleRoot, userRoot].compactMap({ $0 }) where isReady(engine, at: root) {
            return root
        }
        return nil
    }

    /// 只看 size，不哈希：UI 会频繁调用，load_model 自己会再全量校验。
    static func isReady(_ engine: OpiVoiceEngine, at root: URL) -> Bool {
        let fm = FileManager.default
        let dir = root.appendingPathComponent(engine.dirName)
        guard fm.fileExists(atPath: dir.appendingPathComponent("manifest.json").path) else {
            return false
        }
        guard engine.files.allSatisfy({ size(of: dir.appendingPathComponent($0.name)) == $0.size })
        else { return false }
        return size(of: vadURL(root)) == vad.size
    }

    /// 确保模型就绪。只在用户目录下载；返回可加载的模型根。
    /// `isCancelled` 在文件之间与重试之间检查（单个下载不可中断）。
    static func ensure(
        _ engine: OpiVoiceEngine,
        isCancelled: () -> Bool,
        progress: @escaping (Double) -> Void
    ) throws -> URL {
        if let ready = readyRoot(engine) { return ready }

        let root = userRoot
        let fm = FileManager.default
        try fm.createDirectory(at: root, withIntermediateDirectories: true)

        var all: [(file: OpiVoiceEngine.ModelFile, source: URL, dest: URL)] =
            engine.files.map {
                ($0,
                 baseURL.appendingPathComponent(engine.dirName).appendingPathComponent($0.name),
                 root.appendingPathComponent(engine.dirName).appendingPathComponent($0.name))
            }
        all.append((vad, baseURL.appendingPathComponent("vad").appendingPathComponent(vad.name),
                    vadURL(root)))
        let total = all.reduce(Int64(0)) { $0 + $1.file.size }
        var done: Int64 = 0

        for entry in all {
            if isCancelled() { throw OpiVoiceError.cancelled }
            try fm.createDirectory(at: entry.dest.deletingLastPathComponent(),
                                   withIntermediateDirectories: true)
            // 之前下完的文件已经过 SHA-256；manifest 最后写，所以这里只查 size。
            if size(of: entry.dest) == entry.file.size {
                done += entry.file.size
                progress(Double(done) / Double(total))
                continue
            }
            try fetch(entry.file, from: entry.source, to: entry.dest,
                      isCancelled: isCancelled) { bytes in
                progress(Double(done + bytes) / Double(total))
            }
            done += entry.file.size
            progress(Double(done) / Double(total))
        }
        try writeManifest(engine, at: root)
        return root
    }

    // ---------- 内部 ----------

    private static func vadURL(_ root: URL) -> URL {
        root.appendingPathComponent("vad").appendingPathComponent(vad.name)
    }

    private static func size(of url: URL) -> Int64? {
        guard let attrs = try? FileManager.default.attributesOfItem(atPath: url.path),
              let number = attrs[.size] as? NSNumber else { return nil }
        return number.int64Value
    }

    private static func fetch(
        _ file: OpiVoiceEngine.ModelFile,
        from url: URL,
        to dest: URL,
        isCancelled: () -> Bool,
        progress: @escaping (Int64) -> Void
    ) throws {
        var last: Error = OpiVoiceError.message("未知错误")
        for attempt in 1...attempts {
            if isCancelled() { throw OpiVoiceError.cancelled }
            do {
                let temp = try OpiVoiceDownloadTask(expected: file.size, onProgress: progress)
                    .fetch(url)
                defer { try? FileManager.default.removeItem(at: temp) }
                guard isCancelled() == false else { throw OpiVoiceError.cancelled }
                guard size(of: temp) == file.size else {
                    throw OpiVoiceError.message("大小不符（应为 \(file.size) 字节）")
                }
                guard try sha256(temp) == file.sha256.lowercased() else {
                    throw OpiVoiceError.message("SHA-256 校验失败")
                }
                try? FileManager.default.removeItem(at: dest)
                try FileManager.default.moveItem(at: temp, to: dest)
                return
            } catch OpiVoiceError.cancelled {
                throw OpiVoiceError.cancelled
            } catch {
                last = error
                if attempt < attempts { Thread.sleep(forTimeInterval: Double(attempt)) }
            }
        }
        throw OpiVoiceError.message("下载 \(file.name) 失败：\(last.localizedDescription)")
    }

    private static func sha256(_ url: URL) throws -> String {
        let handle = try FileHandle(forReadingFrom: url)
        defer { try? handle.close() }
        var hasher = SHA256()
        while let chunk = try handle.read(upToCount: 1 << 20), !chunk.isEmpty {
            hasher.update(data: chunk)
        }
        return hasher.finalize().map { String(format: "%02x", $0) }.joined()
    }

    private static func writeManifest(_ engine: OpiVoiceEngine, at root: URL) throws {
        let files: [[String: Any]] = engine.files.map {
            ["path": $0.name, "size": $0.size, "sha256": $0.sha256, "role": $0.role]
        }
        let manifest: [String: Any] = ["version": 1, "files": files]
        let data = try JSONSerialization.data(withJSONObject: manifest, options: [.sortedKeys])
        try data.write(to: root.appendingPathComponent(engine.dirName)
            .appendingPathComponent("manifest.json"), options: .atomic)
    }
}

/// 同步下载一个文件，进度回调在 URLSession 的 delegate 队列上。
/// 成功后把临时文件移到一个独立路径返回（系统会在回调返回后删掉原临时文件）。
private final class OpiVoiceDownloadTask: NSObject, URLSessionDownloadDelegate {
    private let expected: Int64
    private let onProgress: (Int64) -> Void
    private let delegateQueue = OperationQueue()
    private let semaphore = DispatchSemaphore(value: 0)
    private var result: Result<URL, Error>?
    private var finished = false
    private var session: URLSession?

    init(expected: Int64, onProgress: @escaping (Int64) -> Void) {
        self.expected = expected
        self.onProgress = onProgress
        super.init()
        delegateQueue.maxConcurrentOperationCount = 1
    }

    func fetch(_ url: URL) throws -> URL {
        let config = URLSessionConfiguration.ephemeral
        config.timeoutIntervalForRequest = 30
        config.timeoutIntervalForResource = 900
        config.requestCachePolicy = .reloadIgnoringLocalCacheData
        let session = URLSession(configuration: config, delegate: self, delegateQueue: delegateQueue)
        self.session = session
        var request = URLRequest(url: url)
        request.setValue("opi-macos/1.0", forHTTPHeaderField: "User-Agent")
        session.downloadTask(with: request).resume()
        semaphore.wait()
        session.finishTasksAndInvalidate()
        guard let result else { throw URLError(.unknown) }
        switch result {
        case .success(let file): return file
        case .failure(let error): throw error
        }
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask,
                    didWriteData bytesWritten: Int64, totalBytesWritten: Int64,
                    totalBytesExpectedToWrite: Int64) {
        onProgress(min(totalBytesWritten, expected))
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask,
                    didFinishDownloadingTo location: URL) {
        let temp = FileManager.default.temporaryDirectory
            .appendingPathComponent("opi-asr-\(UUID().uuidString).part")
        do {
            try? FileManager.default.removeItem(at: temp)
            try FileManager.default.moveItem(at: location, to: temp)
            finish(.success(temp))
        } catch {
            finish(.failure(error))
        }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask,
                    didCompleteWithError error: Error?) {
        if let error { finish(.failure(error)) }
        else { finish(.failure(URLError(.cannotLoadFromNetwork))) }
    }

    private func finish(_ value: Result<URL, Error>) {
        guard !finished else { return }
        finished = true
        result = value
        semaphore.signal()
    }
}
