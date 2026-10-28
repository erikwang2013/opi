// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Shift 轻按切中英的 **TSF 胶水三拍**（从 `tsf.rs` 拆出守 500 行）。
//!
//! 为什么在胶水层：Linux 轨的同一判定在 C++（`opi_mode.h` 的 `OpiShiftToggle`），
//! 两轨的 Rust 逻辑层必须逐事件同构（`crates/opi-ffi/tests/two_track_parity.rs`
//! 压着这条）—— 轻按判定不能进 `TsfLogic::input_key`。纯判定在 `crate::shift_tap`
//! （平台中立、主机可单测），本文件只是把它接到 TSF 的按键回调上。
//!
//! 三拍：
//! 1. [`TsfTextService::shift_tap_key_down`]：`OnTestKeyDown`（**每个**键按下都经过；
//!    非 Shift → 撤销，覆盖 Ctrl/Alt/Win 这些不会进 `OnKeyDown` 的组合键）；
//! 2. [`TsfTextService::take_shift_tap_on_release`]：Shift 抬起（`OnTestKeyUp` 只认领
//!    Shift，`OnKeyUp` 才到得了这里）且仍武装 → 一次轻按；
//! 3. [`TsfTextService::handle_shift_release_event`]：切中英 + W1 收尾 + 刷新/收起候选窗
//!    （锁内只改引擎+抓快照，COM 插入与管道写在锁外）。
//!
//! 失活时 [`TsfTextService::shift_tap_disarm`]：按住 Shift 切走后抬起可能到不了本
//! 服务，不撤销就是回来后的幽灵切换（镜像 `OpiShiftToggle::disarm`）。

use std::time::Instant;

use windows::Win32::UI::TextServices::ITfContext;
use windows::core::BOOL;

use crate::logic::{KEY_SHIFT, KEY_STATE_RELEASED};
use crate::sink::push_state;
use crate::tsf::TsfTextService;
use crate::vk::is_shift_vk;

impl TsfTextService {
    /// 第一拍：每个键按下事件调一次 —— Shift 武装，任何别的键撤销。
    /// `OnTestKeyDown` 是唯一能看见 Ctrl/Alt/Win 组合的位置（它们在那里就返回
    /// FALSE、不会进 `OnKeyDown`），所以撤销必须放在这里。
    ///
    /// `ShiftToggle=False` 时只撤销不武装：关掉开关后轻按不得切换（设置页的
    /// 「切换键」开关在 Windows 上的落点）。
    pub(super) fn shift_tap_key_down(&self, vk: u32) {
        let is_shift = is_shift_vk(vk);
        let armed = is_shift && self.state.shift_toggle();
        // 中毒锁 → 忽略这一次（吞掉判定，不把 panic 带过 COM 边界）。
        if let Ok(mut tap) = self.shift_tap.lock() {
            tap.on_key_down(armed, Instant::now());
        }
    }

    /// 失活复位：见模块头第 3 拍。
    pub(super) fn shift_tap_disarm(&self) {
        if let Ok(mut tap) = self.shift_tap.lock() {
            tap.disarm();
        }
    }

    /// 第二拍：Shift 抬起仍是轻按吗（是则消费掉这次判定）。**必须判在引擎之前**：
    /// 引擎对 Shift 抬起只回 `Consumed`，进去就没有切换点了。
    pub(super) fn take_shift_tap_on_release(&self, states: u32, keycode: u32) -> bool {
        if states & KEY_STATE_RELEASED == 0 || keycode != KEY_SHIFT {
            return false;
        }
        self.shift_tap
            .lock()
            .is_ok_and(|mut tap| tap.take_on_shift_up(Instant::now()))
    }

    /// Shift 抬起的**唯一处理点**（`handle_key_inner` 只调这一句）。返回 `Some` = 本次
    /// 抬起已处理（调用方直接回 `BOOL(1)`）；`None` = 不是 Shift 抬起，继续正常路由。
    ///
    /// 无论是否轻按都要收尾引擎里的 Single：物理 Shift 的大写由 `ToUnicodeEx` 给出，
    /// Shift+方向/组合键（轻按已被撤销）或 `ShiftToggle=False` 时残留的 Single 会让
    /// 下一个字母莫名大写（review 2026-10-22）。
    pub(super) fn handle_shift_release_event(
        &self,
        states: u32,
        keycode: u32,
        pic: Option<&ITfContext>,
    ) -> Option<BOOL> {
        if states & KEY_STATE_RELEASED == 0 || keycode != KEY_SHIFT {
            return None;
        }
        let tap = self.take_shift_tap_on_release(states, keycode);
        // 第三拍：执行中 ⇄ 英。锁内只切语言 + 抓快照，**放锁后**才写文档/管道
        // （insert_into 是同步 COM 编辑会话，持锁跨它会冻住宿主 UI，见 tsf.rs）。
        // W1：进英文前把待处理拼音的**原文**插进文档（`toggle_language` 的返回值），
        // 再让候选窗收起；没有焦点文档/插入被拒时文字丢，但仍推一帧让窗口按新语言
        // 收尾。切回拼音无待上屏文本同样推空帧。与 `toggle_mode` 同一条收尾。
        let (text, snapshot) = {
            let mut logic = match self.state.lock_logic() {
                Some(g) => g,
                None => return Some(BOOL(1)), // 中毒锁：吞键，与 handle_key_inner 同策略
            };
            if tap && logic.shift_toggle() {
                let text = logic.toggle_language();
                (text, logic.snapshot())
            } else {
                logic.consume_single_shift();
                return Some(BOOL(1));
            }
        };
        // 同 toggle_mode：插入成功才收窗口；空文本或插入失败 → 推切换后的空帧，
        // 免得候选窗停在旧语言/旧候选（原文在失败路径确实丢了）。
        let inserted =
            !text.is_empty() && pic.is_some_and(|pic| self.state.insert_into(pic, &text));
        if inserted {
            self.sink.on_commit(&text);
        } else {
            push_state(&*self.sink, &snapshot);
        }
        // 中 ⇄ 英已切：状态栏的「中/英」要立刻翻，不等 1s 轮询。
        self.sink.on_state_changed();
        Some(BOOL(1))
    }
}
