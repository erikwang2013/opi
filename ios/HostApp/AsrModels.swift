// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 语音模型目录、清单与下载（iOS，2026-11-02）。
//
// full 档：package-ios.sh 把 `desktop/asr-bundle/asr` 复制进宿主 bundle 的 `asr/`，
// 本文件只读它，不重复下载。normal 档：首次使用由 `AsrModelDownloader` 逐文件下载
// 到 Application Support/opi/asr。落点与 Rust `AsrConfig` 的相对路径一一对应：
// `models_dir/sensevoice`、`models_dir/whisper-small.en`、`models_dir/vad/silero_vad.onnx`。
//
// 每个文件钉死 size + SHA-256（与 Android/桌面同一套真值）。下载写临时文件，校验
// 通过才移动到位；manifest.json 由本文件写，schema = crates/opi-asr/src/manifest.rs。
// 运行期 `verify_models=false`：下载/打包时已校验，设备上不再重复哈希 615MB。
// 无第三方依赖：URLSession + CryptoKit + Foundation。

import CryptoKit
import Foundation

struct AsrRemoteFile {
    /// 相对模型根目录，例如 `sensevoice/model.int8.onnx`。
    let path: String
    let bytes: Int64
    let sha256: String
    /// Rust manifest 的角色名；VAD 不写 manifest，故为 nil。
    let role: String?
}

enum AsrModels {
    static let siteBase = "https://opi.erik.xyz/download/components/asr/"
    static let sensevoiceDir = "sensevoice"
    static let whisperDir = "whisper-small.en"
    static let vadPath = "vad/silero_vad.onnx"

    static let sensevoiceFiles = [
        AsrRemoteFile(
            path: "sensevoice/model.int8.onnx",
            bytes: 239_233_841,
            sha256: "c71f0ce00bec95b07744e116345e33d8cbbe08cef896382cf907bf4b51a2cd51",
            role: "model"
        ),
        AsrRemoteFile(
            path: "sensevoice/tokens.txt",
            bytes: 315_894,
            sha256: "f449eb28dc567533d7fa59be34e2abca8784f771850c78a47fb731a31429a1dc",
            role: "tokens"
        ),
    ]

    static let whisperFiles = [
        AsrRemoteFile(
            path: "whisper-small.en/small.en-encoder.int8.onnx",
            bytes: 112_442_483,
            sha256: "8bdac288f369aa94ee2194059238c465ed82ea9d47ee8fa4a8c0a891873e462f",
            role: "encoder"
        ),
        AsrRemoteFile(
            path: "whisper-small.en/small.en-decoder.int8.onnx",
            bytes: 262_223_042,
            sha256: "710ccf890e10f3faa15f51ec346081a2723c9f3adb6e4da81c6573a5a6f877fb",
            role: "decoder"
        ),
        AsrRemoteFile(
            path: "whisper-small.en/small.en-tokens.txt",
            bytes: 835_554,
            sha256: "306cd27f03c1a714eca7108e03d66b7dc042abe8c258b44c199a7ed9838dd930",
            role: "tokens"
        ),
    ]

    static let vadFile = AsrRemoteFile(
        path: vadPath,
        bytes: 643_854,
        sha256: "9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6",
        role: nil
    )

    static let allFiles = sensevoiceFiles + whisperFiles + [vadFile]
    static let totalBytes = allFiles.reduce(Int64(0)) { $0 + $1.bytes }

    static func url(for file: AsrRemoteFile) -> URL? {
        URL(string: siteBase + file.path)
    }

    /// 某个模型目录的 manifest.json（只含该目录内的模型文件，不含 VAD）。
    static func manifestJSON(dir: String) -> Data? {
        let prefix = dir + "/"
        let entries: [[String: Any]] = allFiles.compactMap { file in
            guard file.path.hasPrefix(prefix), let role = file.role else { return nil }
            return [
                "path": String(file.path.dropFirst(prefix.count)),
                "size": file.bytes,
                "sha256": file.sha256,
                "role": role,
            ]
        }
        guard !entries.isEmpty else { return nil }
        let root: [String: Any] = ["version": 1, "files": entries]
        return try? JSONSerialization.data(withJSONObject: root, options: [.sortedKeys])
    }

    static func fileSize(_ url: URL) -> Int64 {
        let attrs = try? FileManager.default.attributesOfItem(atPath: url.path)
        return (attrs?[.size] as? NSNumber)?.int64Value ?? 0
    }

    /// 模型根是否完整：六个文件的大小都对，且两个模型目录都有 manifest.json。
    static func isComplete(root: URL) -> Bool {
        for file in allFiles where fileSize(root.appendingPathComponent(file.path)) != file.bytes {
            return false
        }
        for dir in [sensevoiceDir, whisperDir] {
            let manifest = root
                .appendingPathComponent(dir, isDirectory: true)
                .appendingPathComponent("manifest.json")
            guard FileManager.default.fileExists(atPath: manifest.path) else { return false }
        }
        return true
    }

    static func megabytes(_ bytes: Int64) -> String {
        ByteCountFormatter.string(fromByteCount: bytes, countStyle: .file)
    }
}

final class AsrModelManager {
    static let shared = AsrModelManager()
    private let fm = FileManager.default

    /// full 档内置模型。只有**完整**才返回，半套目录按“没有”处理并走下载。
    var bundledRoot: URL? {
        var candidates = [Bundle.main.bundleURL.appendingPathComponent("asr", isDirectory: true)]
        if let url = Bundle.main.url(forResource: "asr", withExtension: nil) {
            candidates.append(url)
        }
        return candidates.first { AsrModels.isComplete(root: $0) }
    }

    func downloadedRoot() -> URL {
        let base = fm.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        return base.appendingPathComponent("opi/asr", isDirectory: true)
    }

    /// 可用模型根：内置优先，其次已下载完整的用户目录。
    func readyRoot() -> URL? {
        if let bundled = bundledRoot { return bundled }
        let downloaded = downloadedRoot()
        return AsrModels.isComplete(root: downloaded) ? downloaded : nil
    }

    func prepareDownloadRoot() throws -> URL {
        let root = downloadedRoot()
        try fm.createDirectory(at: root, withIntermediateDirectories: true)
        // 615MB 不该进 iCloud 备份。
        var url = root
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        try? url.setResourceValues(values)
        return root
    }
}

enum AsrDownloadError: LocalizedError {
    case badURL(String)
    case badStatus(String, Int)
    case badSize(String, expected: Int64, actual: Int64)
    case badHash(String)
    case badManifest

    var errorDescription: String? {
        switch self {
        case .badURL(let path): return "下载地址无效：\(path)"
        case .badStatus(let path, let code): return "下载 \(path) 失败（HTTP \(code)）"
        case .badSize(let path, let expected, let actual):
            return "\(path) 大小不符：应为 \(expected)，实际 \(actual)"
        case .badHash(let path): return "\(path) 校验失败，请重试"
        case .badManifest: return "模型清单写入失败"
        }
    }
}

/// 串行逐文件下载；已存在且大小正确的文件直接跳过（写入时已校验过 SHA-256）。
final class AsrModelDownloader: NSObject, URLSessionDownloadDelegate {
    static let shared = AsrModelDownloader()
    private(set) var isDownloading = false

    private var session: URLSession?
    private var root: URL?
    private var files: [AsrRemoteFile] = []
    private var index = 0
    private var doneBytes: Int64 = 0
    private var currentBytes: Int64 = 0
    private var failed = false
    private var onProgress: ((Double, String) -> Void)?
    private var onFinish: ((Error?) -> Void)?

    func start(
        root: URL,
        files: [AsrRemoteFile],
        progress: @escaping (Double, String) -> Void,
        completion: @escaping (Error?) -> Void
    ) {
        guard !isDownloading else { return }
        self.root = root
        self.files = files
        index = 0
        doneBytes = 0
        currentBytes = 0
        failed = false
        onProgress = progress
        onFinish = completion
        isDownloading = true

        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 60
        config.timeoutIntervalForResource = 3600
        session = URLSession(configuration: config, delegate: self, delegateQueue: nil)
        nextFile()
    }

    private func nextFile() {
        guard !failed, let root else { return }
        while index < files.count {
            let file = files[index]
            let destination = root.appendingPathComponent(file.path)
            if AsrModels.fileSize(destination) == file.bytes {
                doneBytes += file.bytes
                index += 1
                report(name: "")
                continue
            }
            guard let url = AsrModels.url(for: file) else {
                fail(AsrDownloadError.badURL(file.path))
                return
            }
            try? FileManager.default.createDirectory(
                at: destination.deletingLastPathComponent(), withIntermediateDirectories: true
            )
            guard let task = session?.downloadTask(with: url) else {
                fail(AsrDownloadError.badURL(file.path))
                return
            }
            task.resume()
            return
        }
        do {
            try writeManifests(root: root)
            complete(nil)
        } catch {
            complete(error)
        }
    }

    private func writeManifests(root: URL) throws {
        for dir in [AsrModels.sensevoiceDir, AsrModels.whisperDir] {
            guard let data = AsrModels.manifestJSON(dir: dir) else {
                throw AsrDownloadError.badManifest
            }
            let target = root
                .appendingPathComponent(dir, isDirectory: true)
                .appendingPathComponent("manifest.json")
            try data.write(to: target, options: .atomic)
        }
    }

    private func report(name: String) {
        let total = Double(AsrModels.totalBytes)
        let done = min(total, Double(doneBytes + currentBytes))
        let fraction = total > 0 ? done / total : 0
        DispatchQueue.main.async { [weak self] in
            self?.onProgress?(fraction, name)
        }
    }

    private func complete(_ error: Error?) {
        guard isDownloading else { return }
        isDownloading = false
        session?.finishTasksAndInvalidate()
        session = nil
        let finish = onFinish
        onFinish = nil
        onProgress = nil
        DispatchQueue.main.async { finish?(error) }
    }

    private func fail(_ error: Error) {
        guard !failed else { return }
        failed = true
        session?.invalidateAndCancel()
        complete(error)
    }

    // MARK: - URLSessionDownloadDelegate

    func urlSession(
        _ session: URLSession,
        downloadTask: URLSessionDownloadTask,
        didWriteData bytesWritten: Int64,
        totalBytesWritten: Int64,
        totalBytesExpectedToWrite: Int64
    ) {
        currentBytes = totalBytesWritten
        report(name: downloadTask.originalRequest?.url?.lastPathComponent ?? "")
    }

    func urlSession(
        _ session: URLSession,
        downloadTask: URLSessionDownloadTask,
        didFinishDownloadingTo location: URL
    ) {
        guard !failed, index < files.count else { return }
        let file = files[index]
        if let http = downloadTask.response as? HTTPURLResponse, http.statusCode != 200 {
            fail(AsrDownloadError.badStatus(file.path, http.statusCode))
            return
        }
        do {
            let size = AsrModels.fileSize(location)
            guard size == file.bytes else {
                throw AsrDownloadError.badSize(file.path, expected: file.bytes, actual: size)
            }
            guard try Self.sha256(location) == file.sha256 else {
                throw AsrDownloadError.badHash(file.path)
            }
            guard let root else { return }
            let destination = root.appendingPathComponent(file.path)
            try? FileManager.default.removeItem(at: destination)
            try FileManager.default.moveItem(at: location, to: destination)
            doneBytes += file.bytes
            currentBytes = 0
            index += 1
        } catch {
            fail(error)
        }
    }

    func urlSession(
        _ session: URLSession,
        task: URLSessionTask,
        didCompleteWithError error: Error?
    ) {
        guard !failed else { return }
        if let error {
            fail(error)
            return
        }
        nextFile()
    }

    private static func sha256(_ url: URL) throws -> String {
        let handle = try FileHandle(forReadingFrom: url)
        defer { try? handle.close() }
        var hasher = SHA256()
        while true {
            guard let chunk = try handle.read(upToCount: 1 << 20), !chunk.isEmpty else { break }
            hasher.update(data: chunk)
        }
        return hasher.finalize().map { String(format: "%02x", $0) }.joined()
    }
}
