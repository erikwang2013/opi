// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 新版本检测（2026-10-22 用户裁决）：只访问官网 `version.json`，不上传输入数据。
//
// 与 Android `settings/VersionCheck.kt` 同一契约：
//   · 4s 超时；语义化数字比较（1.21.10 > 1.21.2，缺段按 0，允许 v 前缀）；
//   · 下载落点固定 `https://opi.erik.xyz/download`，**不读** JSON 里的 download，
//     不让网络数据决定跳转目标；
//   · 自动检测失败静默；只有手动检测才把原因给用户。
//
// 键盘扩展没有完全访问时网络会失败：自动路径直接跳过，手动才提示需要完全访问。
//
// ⚠️ 本机无 UIKit/Foundation for iOS，只在 Apple CI 真编译；从未上机运行。

import UIKit

final class UpdateChecker {
    static let shared = UpdateChecker()

    /// 官网 `version.json` 的字段：{"version":"x.y.z","download":"..."}。
    private struct Payload: Decodable { let version: String }

    enum Phase {
        case idle
        case checking
        case upToDate
        case available(remote: String)
        case failed(String)
    }

    /// 状态变化通知：设置/关于面板的 `UpdateStatusView` 都监听它。
    static let didChange = Notification.Name("OpiUpdateCheckerDidChange")
    /// 「前往下载」固定落点；不跟随端点里的 download 字段。
    static let downloadURL = "https://opi.erik.xyz/download"

    private static let versionURL = URL(string: "https://opi.erik.xyz/version.json")!
    private static let timeout: TimeInterval = 4

    private(set) var phase: Phase = .idle
    /// 由 `KeyboardViewController` 在打开设置/关于面板时写入（`hasFullAccess`）。
    var hasFullAccess = true

    private var checking = false
    /// 自动检测在飞时用户又点了「检测更新」：本次结果按手动处理（手动才报错）。
    private var manualRequested = false
    /// 自动检测失败时回到上一次稳定状态，不把错误糊到用户脸上。
    private var lastStable: Phase = .idle

    private init() {}

    var currentVersion: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "未知"
    }

    func check(manual: Bool) {
        if checking {
            if manual {
                manualRequested = true
                setPhase(.checking)
            }
            return
        }
        guard hasFullAccess else {
            if manual { setPhase(.failed("需要开启完全访问才能联网检测更新")) }
            return
        }
        checking = true
        manualRequested = manual
        if manual { setPhase(.checking) }

        var request = URLRequest(url: Self.versionURL)
        request.timeoutInterval = Self.timeout
        request.cachePolicy = .reloadIgnoringLocalCacheData

        URLSession.shared.dataTask(with: request) { [weak self] data, response, error in
            DispatchQueue.main.async {
                self?.finish(data: data, response: response, error: error)
            }
        }.resume()
    }

    private func finish(data: Data?, response: URLResponse?, error: Error?) {
        checking = false
        let manual = manualRequested
        manualRequested = false
        if let error {
            fail(Self.reason(for: error), manual: manual)
            return
        }
        guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
            let code = (response as? HTTPURLResponse)?.statusCode ?? -1
            fail("服务器返回 HTTP \(code)", manual: manual)
            return
        }
        guard let data, let payload = try? JSONDecoder().decode(Payload.self, from: data) else {
            fail("版本信息格式不正确", manual: manual)
            return
        }
        let remote = payload.version.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let newer = Self.isNewer(remote, than: currentVersion) else {
            fail("版本号格式不正确", manual: manual)
            return
        }
        let next: Phase = newer ? .available(remote: remote) : .upToDate
        lastStable = next
        setPhase(next)
    }

    private func fail(_ reason: String, manual: Bool) {
        setPhase(manual ? .failed(reason) : lastStable)
    }

    private func setPhase(_ next: Phase) {
        phase = next
        NotificationCenter.default.post(name: Self.didChange, object: self)
    }

    private static func reason(for error: Error) -> String {
        guard let urlError = error as? URLError else { return error.localizedDescription }
        switch urlError.code {
        case .timedOut: return "请求超时"
        case .notConnectedToInternet: return "网络不可用"
        default: return urlError.localizedDescription
        }
    }

    /// `nil` = 版本号不是纯数字点分串（与 Android 同：不猜；手动检测会明确报格式错误）。
    static func isNewer(_ remote: String, than current: String) -> Bool? {
        guard let a = numericParts(remote), let b = numericParts(current) else { return nil }
        for i in 0..<max(a.count, b.count) {
            let x = i < a.count ? a[i] : 0
            let y = i < b.count ? b[i] : 0
            if x != y { return x > y }
        }
        return false
    }

    private static func numericParts(_ version: String) -> [Int]? {
        var raw = version.trimmingCharacters(in: .whitespacesAndNewlines)
        if raw.hasPrefix("v") || raw.hasPrefix("V") { raw.removeFirst() }
        guard !raw.isEmpty else { return nil }
        var parts: [Int] = []
        for piece in raw.split(separator: ".", omittingEmptySubsequences: false) {
            guard !piece.isEmpty, let n = Int(piece) else { return nil }
            parts.append(n)
        }
        return parts
    }
}

/// 设置/关于面板共用的更新状态行：状态文案 + 「检测更新」+「前往下载」。
final class UpdateStatusView: UIView {
    var onOpenDownload: ((String) -> Void)?

    private let label = UILabel()
    private let checkButton = UIButton(type: .system)
    private let downloadButton = UIButton(type: .system)
    private var observer: NSObjectProtocol?

    init() {
        super.init(frame: .zero)
        label.font = .systemFont(ofSize: 11)
        label.numberOfLines = 0
        label.textColor = .secondaryLabel
        for b in [checkButton, downloadButton] {
            b.titleLabel?.font = .systemFont(ofSize: 11)
            b.contentEdgeInsets = UIEdgeInsets(top: 4, left: 0, bottom: 4, right: 10)
        }
        checkButton.setTitle("检测更新", for: .normal)
        downloadButton.setTitle("前往下载", for: .normal)
        checkButton.accessibilityLabel = "检测更新"
        downloadButton.accessibilityLabel = "前往下载新版本"
        checkButton.addAction(UIAction { _ in
            UpdateChecker.shared.check(manual: true)
        }, for: .touchUpInside)
        downloadButton.addAction(UIAction { [weak self] _ in
            self?.onOpenDownload?(UpdateChecker.downloadURL)
        }, for: .touchUpInside)

        let buttons = UIStackView(arrangedSubviews: [checkButton, downloadButton, UIView()])
        buttons.axis = .horizontal
        buttons.spacing = 12
        let content = UIStackView(arrangedSubviews: [label, buttons])
        content.axis = .vertical
        content.spacing = 0
        content.translatesAutoresizingMaskIntoConstraints = false
        addSubview(content)
        NSLayoutConstraint.activate([
            content.leadingAnchor.constraint(equalTo: leadingAnchor),
            content.trailingAnchor.constraint(equalTo: trailingAnchor),
            content.topAnchor.constraint(equalTo: topAnchor),
            content.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
        observer = NotificationCenter.default.addObserver(
            forName: UpdateChecker.didChange, object: nil, queue: .main
        ) { [weak self] _ in self?.render() }
        applyTheme(.light)
        render()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    func applyTheme(_ p: KeyboardPalette) {
        label.textColor = p.barDimFg
        checkButton.setTitleColor(p.accent, for: .normal)
        downloadButton.setTitleColor(p.accent, for: .normal)
    }

    private func render() {
        let current = UpdateChecker.shared.currentVersion
        switch UpdateChecker.shared.phase {
        case .idle:
            label.text = "当前版本 v\(current)"
            checkButton.isHidden = false
            downloadButton.isHidden = true
        case .checking:
            label.text = "正在检测更新…"
            checkButton.isHidden = true
            downloadButton.isHidden = true
        case .upToDate:
            label.text = "已是最新版本 v\(current)"
            checkButton.isHidden = false
            downloadButton.isHidden = true
        case .available(let remote):
            label.text = "发现新版本 v\(remote)（当前 v\(current)）"
            checkButton.isHidden = false
            downloadButton.isHidden = false
        case .failed(let reason):
            label.text = "检测更新失败：\(reason)"
            checkButton.isHidden = false
            downloadButton.isHidden = true
        }
    }
}
