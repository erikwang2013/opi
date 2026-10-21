// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Shift 轻按判定（平台中立，镜像 fcitx5 轨 C++ 的 `OpiShiftToggle`）。
//!
//! **为什么不在 `TsfLogic::input_key` 里判**：Linux 轨的同一个判定在 **C++ glue**
//! （`opi_mode.h` 的 `OpiShiftToggle`）里，Rust 逻辑层对 ⇧ 仍然只做大小写三态。
//! 两条轨的 Rust 逻辑必须逐事件同构（`crates/opi-ffi/tests/two_track_parity.rs`
//! 真压着这条）—— 把轻按判定塞进 TSF 的 Rust 逻辑，第 95 步就会分叉。TSF 没有
//! C++ glue 那一层，于是它在这里（`tsf.rs` 的 `OnTestKeyDown`/`OnKeyUp`）落脚，
//! 与 C++ 的位置对称。
//!
//! 三拍（与 `OpiShiftToggle::handle` 逐一对应）：
//! 1. 任何键**按下**：Shift → 武装并记按下时刻；别的键 → 撤销；
//! 2. Shift **抬起**：仍武装且按住不超过 [`TAP_MAX_HOLD`] → 轻按（返回 true 并复位）；
//! 3. 失焦/切走：`disarm`（否则按住 Shift 切窗口，抬起落到别处，回来时会幽灵切换）。
//!
//! **长按不算轻按**（2026-10-22 review）：设置页明写「长按不触发」，Linux 轨用
//! fcitx5 的 `ModifierOnlyKeyTimeout`（默认 250ms）挡住。TSF 没有同款配置面，这里
//! 用固定的 300ms（比 250ms 略宽，避免按键处理被系统卡顿时误判）。按住 Shift 拖选
//! 再松手不会切中英。
//!
//! 平台中立是为了**能在主机上单测**：真正接线的 `tsf.rs` 是 Windows 专属，本机
//! （Linux）门禁覆盖不到它；把判定摘出来，至少这半有回归。时间点由调用方传入
//! （`Instant`），测试不睡真实时间。

use std::time::{Duration, Instant};

/// 轻按的最长按住时长。对齐 fcitx5 默认 250ms，留 50ms 余量。
pub const TAP_MAX_HOLD: Duration = Duration::from_millis(300);

/// Shift 轻按状态机（见模块头）。
#[derive(Debug, Default, Clone, Copy)]
pub struct ShiftTap {
    armed: bool,
    press_at: Option<Instant>,
}

impl ShiftTap {
    pub const fn new() -> Self {
        Self {
            armed: false,
            press_at: None,
        }
    }

    /// 键**按下**事件：Shift 武装并记时刻；任何别的键撤销。重复事件同样走这里
    /// （按住 Shift 的系统重复会刷新时刻，抬起仍按最后一次重复算时长）。
    pub fn on_key_down(&mut self, is_shift: bool, now: Instant) {
        self.armed = is_shift;
        self.press_at = if is_shift { Some(now) } else { None };
    }

    /// Shift **抬起**：仍武装且按住时长 ≤ [`TAP_MAX_HOLD`] → 轻按（返回 `true`
    /// 并复位）；否则 `false`。非 Shift 的抬起不会调用本函数（`OnTestKeyUp` 只认领 Shift）。
    pub fn take_on_shift_up(&mut self, now: Instant) -> bool {
        let armed = self.armed;
        let press = self.press_at.take();
        self.armed = false;
        match (armed, press) {
            (true, Some(at)) => now.saturating_duration_since(at) <= TAP_MAX_HOLD,
            _ => false,
        }
    }

    /// 撤销（焦点离开/失活时调，见模块头第 3 拍）。
    pub fn disarm(&mut self) {
        self.armed = false;
        self.press_at = None;
    }

    /// 测试与排障用。
    pub fn is_armed(&self) -> bool {
        self.armed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 轻按 = 按下 Shift（只武装）+ 抬起（判定一次）。按下就切的话 Shift+字母
    /// 会先把模式切走。
    #[test]
    fn shift_press_then_release_is_one_tap() {
        let t = Instant::now();
        let mut s = ShiftTap::new();
        assert!(!s.take_on_shift_up(t), "从未按下就抬起：不是轻按");
        s.on_key_down(true, t);
        assert!(s.is_armed());
        assert!(
            s.take_on_shift_up(t + Duration::from_millis(100)),
            "按下后 100ms 抬起 = 轻按"
        );
        assert!(
            !s.take_on_shift_up(t + Duration::from_millis(150)),
            "一次按下只能消费一次"
        );
    }

    /// Shift+字母/方向/Ctrl 组合：中间任何别的键按下都撤销。
    #[test]
    fn any_other_key_down_cancels_the_tap() {
        let t = Instant::now();
        let mut s = ShiftTap::new();
        s.on_key_down(true, t);
        s.on_key_down(false, t + Duration::from_millis(10)); // 'a'
        assert!(
            !s.take_on_shift_up(t + Duration::from_millis(20)),
            "Shift+字母不是轻按"
        );
    }

    /// 按住超过阈值（拖选/犹豫）再松手：不切模式。设置页承诺「长按不触发」。
    #[test]
    fn long_press_does_not_toggle() {
        let t = Instant::now();
        let mut s = ShiftTap::new();
        s.on_key_down(true, t);
        assert!(
            !s.take_on_shift_up(t + Duration::from_millis(400)),
            "400ms > 300ms：长按不是轻按"
        );
        // 阈值本身（≤）仍算轻按，边界有定义、不是随机行为。
        s.on_key_down(true, t);
        assert!(s.take_on_shift_up(t + TAP_MAX_HOLD), "恰好 300ms 仍算轻按");
    }

    /// 按住 Shift 的系统重复：刷新时刻但不改判定语义。
    #[test]
    fn shift_repeats_keep_the_tap_armed() {
        let t = Instant::now();
        let mut s = ShiftTap::new();
        s.on_key_down(true, t);
        s.on_key_down(true, t + Duration::from_millis(50));
        s.on_key_down(true, t + Duration::from_millis(100));
        assert!(s.take_on_shift_up(t + Duration::from_millis(150)));
    }

    /// 失焦撤销：按住 Shift 切走后抬起，不得在回来时切模式。
    #[test]
    fn disarm_prevents_a_ghost_toggle() {
        let t = Instant::now();
        let mut s = ShiftTap::new();
        s.on_key_down(true, t);
        s.disarm();
        assert!(!s.take_on_shift_up(t + Duration::from_millis(50)));
    }
}
