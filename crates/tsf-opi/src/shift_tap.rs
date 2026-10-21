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
//! 1. 任何键**按下**：Shift → 武装；别的键 → 撤销（Shift+字母/方向/Ctrl 组合不是轻按）；
//! 2. Shift **抬起**：仍武装 → 这是一次轻按（`take_on_shift_up` 返回 true 并复位）；
//! 3. 失焦/切走：`disarm`（否则按住 Shift 切窗口，抬起落到别处，回来时会幽灵切换）。
//!
//! 平台中立是为了**能在主机上单测**：真正接线的 `tsf.rs` 是 Windows 专属，本机
//! （Linux）门禁覆盖不到它；把判定摘出来，至少这半有回归。

/// Shift 轻按状态机（见模块头）。
#[derive(Debug, Default, Clone, Copy)]
pub struct ShiftTap {
    armed: bool,
}

impl ShiftTap {
    pub const fn new() -> Self {
        Self { armed: false }
    }

    /// 键**按下**事件：Shift 武装；任何别的键撤销。重复事件同样走这里（按住 Shift
    /// 不放会持续武装，抬起仍算一次轻按 —— 与主流输入法一致）。
    pub fn on_key_down(&mut self, is_shift: bool) {
        self.armed = is_shift;
    }

    /// Shift **抬起**：仍武装 → 轻按（返回 `true` 并复位）；否则 `false`。
    /// 非 Shift 的抬起不会调用本函数（`tsf.rs` 的 `OnTestKeyUp` 只认领 Shift）。
    pub fn take_on_shift_up(&mut self) -> bool {
        let armed = self.armed;
        self.armed = false;
        armed
    }

    /// 撤销（焦点离开/失活时调，见模块头第 3 拍）。
    pub fn disarm(&mut self) {
        self.armed = false;
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
        let mut s = ShiftTap::new();
        assert!(!s.take_on_shift_up(), "从未按下就抬起：不是轻按");
        s.on_key_down(true);
        assert!(s.is_armed());
        assert!(s.take_on_shift_up(), "按下后抬起 = 轻按");
        assert!(!s.take_on_shift_up(), "一次按下只能消费一次");
    }

    /// Shift+字母/方向/Ctrl 组合：中间任何别的键按下都撤销。
    #[test]
    fn any_other_key_down_cancels_the_tap() {
        let mut s = ShiftTap::new();
        s.on_key_down(true);
        s.on_key_down(false); // 'a'
        assert!(!s.take_on_shift_up(), "Shift+字母不是轻按");
    }

    /// 按住 Shift 的系统重复：仍武装（抬起算一次轻按，不会来回切）。
    #[test]
    fn shift_repeats_keep_the_tap_armed() {
        let mut s = ShiftTap::new();
        s.on_key_down(true);
        s.on_key_down(true);
        s.on_key_down(true);
        assert!(s.take_on_shift_up());
    }

    /// 失焦撤销：按住 Shift 切走后抬起，不得在回来时切模式。
    #[test]
    fn disarm_prevents_a_ghost_toggle() {
        let mut s = ShiftTap::new();
        s.on_key_down(true);
        s.disarm();
        assert!(!s.take_on_shift_up());
    }
}
