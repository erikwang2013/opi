// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// OpiUpdateCheck.swift —— 新版本检测（2026-10-22 用户裁决）。
//
// 数据源 = https://opi.erik.xyz/version.json：
//   {"version":"1.21.2","download":"https://opi.erik.xyz/download"}
// 4s 超时；语义化比较（数字段，缺段按 0）。打开设置时自动检测一次，失败**不显示**；
// 「关于」页的「检测更新」是手动入口，失败才显示原因。下载走 NSWorkspace.open。
// 网络只在后台线程，回调切回主线程。

import Foundation

final class OpiUpdateChecker {
    static let shared = OpiUpdateChecker()
    static let versionURL = URL(string: "https://opi.erik.xyz/version.json")!
    static let defaultDownloadURL = URL(string: "https://opi.erik.xyz/download")!
    private static let timeout: TimeInterval = 4

    enum State {
        case idle
        case checking
        case upToDate
        case available(latest: String, download: URL)
        case failed(reason: String)
    }

    private(set) var state: State = .idle
    /// 最近一次成功结论：自动检测失败时保留它，别把已知的新版本擦掉。
    private var lastResult: State = .idle
    private var task: URLSessionDataTask?
    /// 结果变化（主线程回调）。设置窗只更新「关于」页，别的页不重建。
    var onChange: (() -> Void)?

    private lazy var session: URLSession = {
        let config = URLSessionConfiguration.ephemeral
        config.timeoutIntervalForRequest = Self.timeout
        config.timeoutIntervalForResource = Self.timeout
        return URLSession(configuration: config)
    }()

    private init() {}

    var currentVersion: String {
        Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "未知"
    }

    /// 关于页那一行的文字（与 Linux `updateLabel` 同语义）。
    var statusText: String {
        switch state {
        case .idle:
            return "尚未检测"
        case .checking:
            return "正在检测更新…"
        case .upToDate:
            return "已是最新版本（v\(currentVersion)）"
        case .available(let latest, _):
            return "发现新版本 v\(latest)（当前 v\(currentVersion)）"
        case .failed(let reason):
            return "检测失败：\(reason)"
        }
    }

    /// 当前可用的下载地址（只有发现新版本时非空）。
    var downloadURL: URL? {
        if case .available(_, let url) = state { return url }
        return nil
    }

    /// 检测一次。[manual] = true 时失败原因显示在关于页；false（打开设置）时保持上次结论。
    func check(manual: Bool) {
        if case .checking = state { return }
        state = .checking
        onChange?()

        let current = currentVersion
        var request = URLRequest(url: Self.versionURL, timeoutInterval: Self.timeout)
        request.httpMethod = "GET"
        request.cachePolicy = .reloadIgnoringLocalCacheData
        request.setValue("opi/\(current)", forHTTPHeaderField: "User-Agent")
        request.setValue("application/json", forHTTPHeaderField: "Accept")

        task = session.dataTask(with: request) { [weak self] data, response, error in
            let result = Self.evaluate(data: data, response: response, error: error, current: current)
            DispatchQueue.main.async {
                guard let self else { return }
                self.task = nil
                if case .failed = result, !manual {
                    self.state = self.lastResult
                } else {
                    self.state = result
                    if case .failed = result {
                        // 手动失败照实显示，但不覆盖上一次成功结论。
                    } else {
                        self.lastResult = result
                    }
                }
                self.onChange?()
            }
        }
        task?.resume()
    }

    /// 版本端点 + 当前版本 → 结论。纯函数，便于人工核对（本机无 Swift 工具链，未单测）。
    static func evaluate(data: Data?, response: URLResponse?, error: Error?,
                         current: String) -> State {
        if let error {
            return .failed(reason: "网络错误：\(error.localizedDescription)")
        }
        guard let http = response as? HTTPURLResponse else {
            return .failed(reason: "服务器无响应")
        }
        guard http.statusCode == 200 else {
            return .failed(reason: "服务器返回 \(http.statusCode)")
        }
        guard let data,
              let obj = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else {
            return .failed(reason: "版本信息格式不正确")
        }
        let raw = (obj["version"] as? String) ?? (obj["latest"] as? String)
        guard let raw, let latest = normalizedVersion(raw) else {
            return .failed(reason: "版本信息缺少有效版本号")
        }
        guard let mine = normalizedVersion(current) else {
            return .failed(reason: "当前版本号未知")
        }
        guard compare(latest, mine) > 0 else { return .upToDate }
        let download = (obj["download"] as? String)
            .flatMap { URL(string: $0) }
            .flatMap { $0.scheme == "https" ? $0 : nil }
            ?? Self.defaultDownloadURL
        return .available(latest: displayVersion(raw), download: download)
    }

    /// `1.20.2` / `v1.20.2` → [1,20,2]；非数字点号 → nil。
    static func normalizedVersion(_ raw: String) -> [Int]? {
        var s = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if s.hasPrefix("v") || s.hasPrefix("V") { s.removeFirst() }
        let parts = s.split(separator: ".", omittingEmptySubsequences: false)
        guard !parts.isEmpty else { return nil }
        var out: [Int] = []
        for p in parts {
            guard !p.isEmpty, let n = Int(p), n >= 0 else { return nil }
            out.append(n)
        }
        return out
    }

    static func displayVersion(_ raw: String) -> String {
        var s = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if s.hasPrefix("v") || s.hasPrefix("V") { s.removeFirst() }
        return s
    }

    /// 数字段逐个比，缺段按 0（`1.20` == `1.20.0`）。
    static func compare(_ a: [Int], _ b: [Int]) -> Int {
        for i in 0..<max(a.count, b.count) {
            let x = i < a.count ? a[i] : 0
            let y = i < b.count ? b[i] : 0
            if x != y { return x < y ? -1 : 1 }
        }
        return 0
    }
}
