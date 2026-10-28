// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! ⇧ 键：大小写三态 + 中英切换的语义本体（`logic` 的子模块，从 `logic.rs` 与
//! `logic_input_method.rs` 拆出守 500 行；两处都是 Shift 相关，不另立第二主题）。
//!
//! 本模块**只放 Rust 逻辑层**（两轨必须逐事件同构，`two_track_parity` 压着）：
//! · 大小写三态 Off/Single/Lock（镜像 Android `EngineController`，屏上键盘语义）；
//! · [`TsfLogic::toggle_language`]：中 ⇄ 英的语义本体（W1 收尾 + 切模式），
//!   状态栏 `lang`、控制面 `lang` 与 Shift 轻按共用这一份。
//!
//! **物理 Shift 的「轻按」判定不在这里**：Linux 轨在 C++ glue（`OpiShiftToggle`），
//! TSF 轨在 `tsf.rs` 的 `OnTestKeyDown`/`OnKeyUp`（纯判定在 `crate::shift_tap`）。
//! 把 glue 语义塞进 `input_key` 会让两轨 Rust 逻辑分叉。

use super::input_method::{
    KEY_STATE_LONG_PRESSED, KEY_STATE_RELEASED, KEY_STATE_REPEAT, KeyOutcome,
};
use crate::logic::{ShiftState, TsfLogic};

impl TsfLogic {
    // ---- ⇧ 状态机（镜像 Android EngineController.shiftTap/ShiftLongPress/consumeSingleShift）----

    pub fn shift_state(&self) -> ShiftState {
        self.shift_state
    }

    /// 清空 ⇧ 残留（状态栏 `lang`/`mode` 动作进英文前用）：与用户按 ⇧ 的「单次大写」
    /// 不是一回事，切换动作不该把大写带进新档位。
    pub fn reset_shift(&mut self) {
        self.shift_state = ShiftState::Off;
        self.set_shift(false);
    }

    /// 单击：Off→Single（引擎 shift 开）；Single/Lock→Off（引擎 shift 关）。
    pub fn shift_tap(&mut self) {
        self.shift_state = if self.shift_state == ShiftState::Off {
            ShiftState::Single
        } else {
            ShiftState::Off
        };
        self.set_shift(self.shift_state != ShiftState::Off);
    }

    /// 长按：Lock（持续大写）。
    pub fn shift_long_press(&mut self) {
        self.shift_state = ShiftState::Lock;
        self.set_shift(true);
    }

    /// single 态消费后复位（lock 不受影响）。
    pub fn consume_single_shift(&mut self) {
        if self.shift_state == ShiftState::Single {
            self.shift_state = ShiftState::Off;
            self.set_shift(false);
        }
    }

    /// Shift 轻按开关（`settings.ini` 的 `ShiftToggle`）：**胶水**的 Shift 三拍读它
    /// （`tsf_shift.rs`）；`input_key` 不看 —— 两轨 Rust 键路由必须逐事件同构。
    pub fn shift_toggle(&self) -> bool {
        self.shift_toggle
    }

    /// 中 ⇄ 英（**Shift 轻按**与状态栏 `lang` 动作的唯一实现）：进英文先按 W1 收尾
    /// （待处理拼音上**原文**，而不是被 `switch_mode` 静默清掉），离开英文只切模式。
    /// 返回待上屏文本（空串 = 无）—— 调用方负责插入文档。
    ///
    /// 镜像 fcitx5 轨 `OpiShiftToggle::toggle`：切英文后 `reset_shift()` —— 轻按的
    /// 语义是切模式，不是「下个字母大写」；不清的话切过去第一个字母会莫名大写
    /// （Linux 轨实测过的同一形态）。`switch_mode` 只在**离开** English 时清 ⇧，
    /// 所以进英文这一步必须显式清。
    pub fn toggle_language(&mut self) -> String {
        if self.mode() == engine_core::composer::Mode::English {
            self.switch_mode(engine_core::composer::Mode::Pinyin);
            String::new()
        } else {
            let text = self.switch_mode_confirmed(engine_core::composer::Mode::English);
            self.reset_shift();
            text
        }
    }

    /// ⇧ 键：单击切换状态机（Off→Single→Off），长按 → Lock；释放/重复忽略。
    /// 镜像 `EngineController.shiftTap/shiftLongPress`（按住并释放 = 一次 tap）。
    ///
    /// **物理 Shift 的「轻按切中英」不在这里**：Linux 轨的同一判定在 C++ glue
    /// （`OpiShiftToggle`），TSF 轨在 `tsf.rs` 的 `OnTestKeyDown`/`OnKeyUp`
    /// （平台中立的判定在 `crate::shift_tap`）。两轨 Rust 逻辑必须逐事件同构
    /// （`two_track_parity` 压着），别把 glue 语义塞进 `input_key`。
    pub(super) fn handle_shift(&mut self, key_state: u32) -> KeyOutcome {
        let released = key_state & KEY_STATE_RELEASED != 0;
        let repeat = key_state & KEY_STATE_REPEAT != 0;
        if !released && !repeat {
            if key_state & KEY_STATE_LONG_PRESSED != 0 {
                self.shift_long_press();
            } else {
                self.shift_tap();
            }
        }
        KeyOutcome::Consumed
    }

    /// 引擎侧 shift 位。`pub(super)`：`logic.rs` 的 `switch_mode` 也要重推。
    pub(super) fn set_shift(&mut self, on: bool) {
        self.invalidate_fetch_cache();
        self.engine.set_shift(on);
        // 2026-10-26：Shift 不改变候选集（只影响后续英文字母大小写）。旧实现在这里
        // 走 set_page → page_count → candidates(MAX) 全量重排，Shift 按下白付 ~9ms。
    }
}
