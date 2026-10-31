// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 本机（Linux）没有 Swift 编译器，未编译；由 apple.yml 真编译。
//
// 平台中立的 opi-clip 宿主。macOS IMK 进程与 iOS 键盘扩展共用这一份：
// 两边只有「剪贴板读写 + UI」的薄壳（macos/OpiClip.swift、ios/KeyboardClipboard.swift）。
//
// 为什么是一条自建线程：`opi_clip_*` 的句柄**线程绑定**（cabi_clip.rs：create 记录创建
// 线程，其余调用在别的线程上一律失败；错线程 destroy 只泄漏）。`DispatchQueue` 是串行
// 但**不保证线程亲和**，所以这里用一条普通 Thread + 信号量命令循环 —— 句柄的
// create/start/poll/pair/stop/destroy 全在它上面，不阻塞主线程。
//
// 回调（onEvent/onPeers/onState）都在**剪贴板线程**上调用，平台侧自己切主线程。
// 内容不落盘、不打日志：事件 JSON 原样交给平台。
//
// 本文件同时编进 macOS 与 iOS 两个模块，因此**不能**用两个模块各自的
// `take` / `withUTF16`（名字不同），自带两个私有小工具。

import Foundation

final class OpiClipRunner {
    typealias EventHandler = ([String: Any]) -> Void
    typealias PeersHandler = ([[String: Any]]) -> Void
    typealias StateHandler = (_ running: Bool, _ error: String) -> Void

    /// 三个回调必须在本对象第一次 `start` 之前设置（平台侧在 init 里设）。
    var onEvent: EventHandler?
    var onPeers: PeersHandler?
    var onState: StateHandler?

    private let lock = NSLock()
    private var commands: [() -> Void] = []
    private let wake = DispatchSemaphore(value: 0)
    private var started = false
    private var running = false
    private var error = ""

    // ---- 以下只在剪贴板线程上读写（命令闭包与 poll 都在那条线程执行）----
    private var handle: OpaquePointer?
    private var nextPeersAt: TimeInterval = 0

    // MARK: - 公共入口（任意线程）

    /// 创建并启动内核；已启动/启动中则忽略（幂等）。
    func start(configJSON: String) {
        ensureThread()
        enqueue { [weak self] in self?.startCore(configJSON) }
    }

    /// 停止并释放内核句柄；线程留着等下一次 start（空闲时只在信号量上睡）。
    func stop() {
        enqueue { [weak self] in self?.stopCore() }
    }

    /// 0=已发布 1=防回环抑制 -1=失败；失败原因走 onState 的 error。
    func publish(_ text: String) {
        enqueue { [weak self] in self?.publishCore(text) }
    }

    func pair(_ peerId: String) {
        enqueue { [weak self] in
            guard let self, let h = self.handle else { return }
            let ok = clipWithUTF16(peerId) { opi_clip_pair_start(h, $0, $1) }
            self.finishCall(ok)
        }
    }

    func confirm(_ peerId: String, accept: Bool) {
        enqueue { [weak self] in
            guard let self, let h = self.handle else { return }
            let ok = clipWithUTF16(peerId) { opi_clip_pair_confirm(h, $0, $1, accept) }
            self.finishCall(ok)
        }
    }

    func forget(_ peerId: String) {
        enqueue { [weak self] in
            guard let self, let h = self.handle else { return }
            let ok = clipWithUTF16(peerId) { opi_clip_forget_peer(h, $0, $1) }
            self.finishCall(ok)
        }
    }

    /// 立即拉一次对端快照（内核的 1s 节流之外的手动刷新）。
    func refreshPeers() {
        enqueue { [weak self] in self?.refreshPeersIfDue(force: true) }
    }

    var isRunning: Bool {
        lock.lock()
        defer { lock.unlock() }
        return running
    }

    var lastError: String {
        lock.lock()
        defer { lock.unlock() }
        return error
    }

    // MARK: - 线程

    private func ensureThread() {
        lock.lock()
        defer { lock.unlock() }
        guard !started else { return }
        started = true
        let thread = Thread { [weak self] in self?.loop() }
        thread.name = "opi-clip"
        thread.qualityOfService = .utility
        thread.start()
    }

    private func enqueue(_ block: @escaping () -> Void) {
        lock.lock()
        // 没 start 过就没有线程会来取；直接丢掉，避免键盘反复收起时命令越积越多。
        guard started else {
            lock.unlock()
            return
        }
        commands.append(block)
        lock.unlock()
        wake.signal()
    }

    private func loop() {
        while true {
            // 空闲时睡久一点；运行中 100ms 一拍（事件轮询与对端快照节流都在 tick 里）。
            let timeout: DispatchTimeInterval = handle == nil ? .seconds(30) : .milliseconds(100)
            _ = wake.wait(timeout: .now() + timeout)
            lock.lock()
            let batch = commands
            commands.removeAll()
            lock.unlock()
            for run in batch { run() }
            guard handle != nil else { continue }
            drainEvents()
            refreshPeersIfDue()
        }
    }

    private func startCore(_ configJSON: String) {
        guard handle == nil else { return }
        guard let created = clipWithUTF16(configJSON, { opi_clip_create($0, $1) }) else {
            setState(running: false, error: readLastError())
            return
        }
        guard opi_clip_start(created) else {
            let why = readLastError()
            opi_clip_destroy(created)
            setState(running: false, error: why)
            return
        }
        handle = created
        nextPeersAt = 0
        setState(running: true, error: "")
        refreshPeersIfDue(force: true)
    }

    private func stopCore() {
        if let h = handle {
            handle = nil
            opi_clip_stop(h)
            opi_clip_destroy(h)
        }
        nextPeersAt = 0
        setState(running: false, error: "")
        onPeers?([])
    }

    private func publishCore(_ text: String) {
        guard let h = handle else {
            setState(running: false, error: "clip node is not started")
            return
        }
        let rc = clipWithUTF16(text) { opi_clip_local_text(h, $0, $1) }
        setState(running: true, error: rc < 0 ? readLastError() : "")
    }

    private func finishCall(_ ok: Bool) {
        setState(running: handle != nil, error: ok ? "" : readLastError())
    }

    // MARK: - 事件 / 对端

    private func drainEvents() {
        guard let h = handle else { return }
        // 一拍最多 64 条：极端情况下也不让 UI 命令饿死。
        for _ in 0..<64 {
            let json = clipTake(opi_clip_poll_event(h, 0))
            if json.isEmpty { return }
            guard let data = json.data(using: .utf8),
                  let raw = try? JSONSerialization.jsonObject(with: data),
                  let obj = raw as? [String: Any] else { continue }
            switch obj["type"] as? String {
            case "peer_discovered", "peer_offline", "pairing_finished":
                nextPeersAt = 0 // 下一拍立刻刷新快照
            default:
                break
            }
            onEvent?(obj)
        }
    }

    private func refreshPeersIfDue(force: Bool = false) {
        guard let h = handle else { return }
        let now = Date().timeIntervalSince1970
        if !force && now < nextPeersAt { return }
        nextPeersAt = now + 1
        let json = clipTake(opi_clip_peers(h))
        guard let data = json.data(using: .utf8),
              let raw = try? JSONSerialization.jsonObject(with: data),
              let list = raw as? [[String: Any]] else { return }
        onPeers?(list)
    }

    private func setState(running nowRunning: Bool, error message: String) {
        lock.lock()
        let changed = running != nowRunning || error != message
        running = nowRunning
        error = message
        lock.unlock()
        guard changed else { return }
        onState?(nowRunning, message)
    }

    private func readLastError() -> String {
        let message = clipTake(opi_clip_last_error())
        return message.isEmpty ? "clip core call failed" : message
    }

    // MARK: - 私有 UTF-16 桥（两模块共用，见文件头）

    private func clipTake(_ s: OpiString) -> String {
        defer { opi_ffi_free_string(s) } // 每个返回句柄恰好释放一次
        guard let ptr = s.ptr, s.len > 0 else { return "" }
        return String(decoding: UnsafeBufferPointer(start: ptr, count: s.len), as: UTF16.self)
    }

    private func clipWithUTF16<R>(_ s: String, _ body: (UnsafePointer<UInt16>?, Int) -> R) -> R {
        let units = Array(s.utf16)
        guard !units.isEmpty else { return body(nil, 0) }
        return units.withUnsafeBufferPointer { body($0.baseAddress, $0.count) }
    }
}
