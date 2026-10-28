// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! ⇧ 轻按判定：三拍与 fcitx5 轨的 `OpiShiftToggle`、TSF 轨的 `shift_tap.rs`
//! 逐条对应（按下武装 → 任何别的键按下撤销 → 抬起且按住 ≤ 300ms 才算轻按）。
//! 时间用 `systemUptime`（单调，不受系统时钟调整影响）。

import Foundation

struct ShiftTap {
    /// 与 `shift_tap.rs` 的 `TAP_MAX_HOLD` 同值（fcitx5 默认 250ms + 50ms 余量）。
    static let maxHold: TimeInterval = 0.3

    private var pressAt: TimeInterval?

    /// Shift 按下（重复按下刷新时刻，与 `on_key_down` 一致）。
    mutating func press(now: TimeInterval = ProcessInfo.processInfo.systemUptime) {
        pressAt = now
    }

    /// 任何别的键按下：撤销。
    mutating func cancel() {
        pressAt = nil
    }

    /// Shift 抬起：返回是否轻按（≤ `maxHold`），并复位。
    mutating func release(now: TimeInterval = ProcessInfo.processInfo.systemUptime) -> Bool {
        defer { pressAt = nil }
        guard let at = pressAt else { return false }
        return now - at <= Self.maxHold
    }

    /// 失焦：按住 Shift 切窗口后抬起可能落到别处，不撤销就是回来后的幽灵切换。
    mutating func disarm() {
        pressAt = nil
    }
}
