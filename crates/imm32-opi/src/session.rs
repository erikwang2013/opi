// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! IMM32 的**平台中立**状态机：键事件 → 一组「要发给宿主/自己画的」动作。
//!
//! 这里不碰 HIMC/HWND/Win32；`win.rs` 只负责把 [`ImmAction`] 翻成
//! `WM_IME_*`、`ImmSetCompositionString` 和候选窗绘制。这样键路由、Shift 轻按、
//! 提交/组合串顺序都能在 Linux 主机上单测 —— Windows 真机跑不到的部分越薄越好。
//!
//! 组合串语义与 TSF 轨一致：`TsfLogic::input_key` 是唯一键路由；本层只镜像
//! `tsf_shift.rs` 的 Shift 三拍（IMM32 没有 `OnTestKeyDown`/`OnTestKeyUp`）。

use std::time::Instant;

use tsf_opi::logic::{
    KEY_SHIFT, KEY_STATE_ALT, KEY_STATE_CTRL, KEY_STATE_RELEASED, KeyOutcome, StateSnapshot,
    TsfLogic,
};
use tsf_opi::shift_tap::ShiftTap;
use tsf_opi::vk::{ModeHotkey, hotkey_target};

/// 一次按键后需要执行的动作。顺序就是发送顺序。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImmAction {
    StartComposition,
    SetComposition(String),
    EndComposition,
    /// 最终上屏文本：先写进 composition 的 result 段，再结束组合。
    Result(String),
    ShowCandidates {
        items: Vec<String>,
        page: usize,
        page_count: usize,
    },
    HideCandidates,
}

pub struct ImmSession {
    logic: TsfLogic,
    tap: ShiftTap,
    composing: bool,
}

impl ImmSession {
    pub fn new(logic: TsfLogic) -> Self {
        Self {
            logic,
            tap: ShiftTap::new(),
            composing: false,
        }
    }

    pub fn logic(&self) -> &TsfLogic {
        &self.logic
    }

    pub fn logic_mut(&mut self) -> &mut TsfLogic {
        &mut self.logic
    }

    /// `ImeProcessKey` 的只读认领：TRUE = `ImeToAsciiEx` 会消费。
    /// Ctrl/Alt 组合一律放行（与 `input_key` 的 Unhandled 分支一致）。
    pub fn claim(&self, keycode: u32, states: u32) -> bool {
        if states & (KEY_STATE_CTRL | KEY_STATE_ALT) != 0 {
            return false;
        }
        // IMM32 的惯例：非 Shift 抬起直接交给应用；Shift 抬起是轻按切中英的判定点。
        if states & KEY_STATE_RELEASED != 0 && keycode != KEY_SHIFT {
            return false;
        }
        keycode == KEY_SHIFT || self.logic.claim_on_test(keycode)
    }

    /// 切走/失焦/换输入上下文：丢掉未完成的组合，保留已加载的词库。
    pub fn reset(&mut self) {
        self.logic.clear();
        self.tap.disarm();
        self.composing = false;
    }

    /// 未认领的 keydown（Shift+方向键等）也必须撤销 Shift 轻按：IMM32 不会把这类
    /// 键送进 `ImeToAsciiEx`，但在 `ImeProcessKey` 里必须让 tap 重新计数。
    pub fn disarm_shift_tap(&mut self) {
        self.tap.disarm();
    }

    /// Ctrl+' / Ctrl+\ 的模式热键（TSF `toggle_mode` 的 IMM32 对应）。
    pub fn toggle_mode_hotkey(&mut self, hot: ModeHotkey) -> Vec<ImmAction> {
        let target = hotkey_target(hot, self.logic.mode());
        let text = self.logic.switch_mode_confirmed(target);
        let mut out = Vec::new();
        if !text.is_empty() {
            out.push(ImmAction::Result(text));
        }
        if self.composing {
            out.push(ImmAction::EndComposition);
            self.composing = false;
        }
        out.push(ImmAction::HideCandidates);
        out
    }

    /// Shift+Space 的全角开关（不改变缓冲/候选集）。
    pub fn toggle_fullwidth(&mut self) {
        self.logic.toggle_fullwidth();
    }

    /// 键事件 → 动作。`states` 用 `logic::KEY_STATE_*` 位；`now` 供 Shift 轻按判定
    /// 注入（测试不睡真实时间）。
    pub fn process_at(&mut self, keycode: u32, states: u32, now: Instant) -> Vec<ImmAction> {
        if keycode == KEY_SHIFT {
            if states & KEY_STATE_RELEASED != 0 {
                return self.shift_release(now);
            }
            self.tap.on_key_down(self.logic.shift_toggle(), now);
            let outcome = self.logic.input_key(keycode, states);
            return self.outcome(outcome);
        }
        if states & KEY_STATE_RELEASED == 0 {
            self.tap.on_key_down(false, now);
        }
        let outcome = self.logic.input_key(keycode, states);
        self.outcome(outcome)
    }

    pub fn process(&mut self, keycode: u32, states: u32) -> Vec<ImmAction> {
        self.process_at(keycode, states, Instant::now())
    }

    /// 候选窗点选：页内索引 → 提交文本。返回的动作与键盘选词一致。
    pub fn select(&mut self, index: usize) -> Vec<ImmAction> {
        let text = self.logic.select(index);
        let mut out = Vec::new();
        if !text.is_empty() {
            out.push(ImmAction::Result(text));
        }
        if self.composing {
            out.push(ImmAction::EndComposition);
            self.composing = false;
        }
        out.push(ImmAction::HideCandidates);
        out
    }

    fn shift_release(&mut self, now: Instant) -> Vec<ImmAction> {
        let tapped = self.tap.take_on_shift_up(now);
        if !(tapped && self.logic.shift_toggle()) {
            self.logic.consume_single_shift();
            return Vec::new();
        }
        // 与 TSF `handle_shift_release_event` 同序：先把待处理的拼音按原文交出去，
        // 再结束组合。IMM32 没有异步 insert，动作顺序就是宿主看到的顺序。
        let text = self.logic.toggle_language();
        let mut out = Vec::new();
        if !text.is_empty() {
            out.push(ImmAction::Result(text));
        }
        if self.composing {
            out.push(ImmAction::EndComposition);
            self.composing = false;
        }
        out.push(ImmAction::HideCandidates);
        out
    }

    fn outcome(&mut self, outcome: KeyOutcome) -> Vec<ImmAction> {
        match outcome {
            KeyOutcome::Commit(text) => {
                let mut out = Vec::new();
                if !text.is_empty() {
                    out.push(ImmAction::Result(text));
                }
                if self.composing {
                    out.push(ImmAction::EndComposition);
                    self.composing = false;
                }
                out.push(ImmAction::HideCandidates);
                out
            }
            KeyOutcome::CompositionChanged => {
                let snap = self.logic.snapshot();
                self.composition_actions(&snap)
            }
            KeyOutcome::Consumed | KeyOutcome::Unhandled => Vec::new(),
        }
    }

    fn composition_actions(&mut self, snap: &StateSnapshot) -> Vec<ImmAction> {
        if !snap.composing || snap.buffer.is_empty() {
            let mut out = Vec::new();
            if self.composing {
                out.push(ImmAction::EndComposition);
                self.composing = false;
            }
            out.push(ImmAction::HideCandidates);
            return out;
        }
        let mut out = Vec::new();
        if !self.composing {
            out.push(ImmAction::StartComposition);
            self.composing = true;
        }
        out.push(ImmAction::SetComposition(snap.buffer.clone()));
        if snap.candidates.is_empty() {
            out.push(ImmAction::HideCandidates);
        } else {
            out.push(ImmAction::ShowCandidates {
                items: snap.candidates.clone(),
                page: snap.page,
                page_count: snap.page_count,
            });
        }
        out
    }
}
