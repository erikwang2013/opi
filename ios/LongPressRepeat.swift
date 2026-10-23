// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 按住连续退格（2026-10-23）。此前长按 ⌫ 是**一次性**清组合（`clearComposingInput`），
// 用户要的是按住后持续执行**普通退格**：先退组合输入，组合空后退宿主正文，
// 抬手/取消立刻停。短按语义不变（仍然只删一格）。
//
// 手势与驱动分开：手势接线在 `KeyboardTouch.swift`（键面）与
// `CandidateTiledPanel.swift`（平铺右列），本类只管「起表 / 停表 / 每拍回调」。
// 每拍就是一次普通 `route(.backspace)` —— 引擎的退格本来就先消费组合、组合空时
// PassThrough 给宿主，所以这里不需要（也不允许）另写一套删除语义。
//
// Timer 放 `.common`：与表情搜索防抖同一条理由 —— `.default` 在滚动/触摸跟踪
// 时会停表，而按住期间正是触摸跟踪态。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译；0.5s 长按阈值与 0.1s 重复间隔是拍的，
// 真机上按手感校准（Android 侧同样待校准）。

import Foundation

final class RepeatBackspaceDriver {

    /// 每拍做什么（由手势层装：一次普通 `route(.backspace)`）。
    var onTick: (() -> Void)?

    /// 重复间隔。0.1s ≈ 每秒 10 格，接近系统键盘的按住退格。
    private static let interval: TimeInterval = 0.1

    private var timer: Timer?

    var isRunning: Bool { timer != nil }

    /// `.began` 时调：**立刻**删一格（长按已成立，不该再等一个 interval），
    /// 之后按 interval 重复。重复调用是幂等的（先停旧表）。
    func start() {
        stop()
        tick()
        let t = Timer(timeInterval: Self.interval, repeats: true) { [weak self] _ in
            self?.tick()
        }
        RunLoop.main.add(t, forMode: .common)
        timer = t
    }

    func stop() {
        timer?.invalidate()
        timer = nil
    }

    private func tick() {
        onTick?()
    }

    deinit { timer?.invalidate() }
}
