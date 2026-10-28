// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **编辑键一族**（退格 / 空格 / 回车）—— 2026-10-14 从 `router.rs` 拆出（那边加回车
//! 判据接线后 512 行，撞 500 行门禁；拆分先例与 `candidates_*.rs` 同套，语义一字未动）。
//!
//! 三个键共同点：都先判「缓冲空不空」再分流（空 = 直通 / 出空格），且都要在改缓冲后
//! `reset_page_if_buffer_changed()` —— 与 ⇧ / 翻页 / 可打印键那几族分开写更好读。
//! 回车那条的判据来自 [`crate::Engine::enter_action`]（实现见 `engine_enter.rs`）。
//!
//! 另含三条**会话复位**出口（`clear` / `set_scheme` / `set_shuangpin_layout`）：
//! 它们与编辑键共用「按下记录作废」契约，故和 `clear_held_keys` 放在一起；
//! `opi-ffi` 的 raw 出口必须走这三条，别直接 `engine_mut()`。

use crate::EnterAction;
use crate::router::{KeyAction, KeyRouter, PAGE_SIZE, commit_or_handled};

/// 按住键集合上限：覆盖常见键盘 rollover（含 Ctrl+字母这类组合之外的单键），
/// 又不给「只发 keydown 不发 keyup」的客户端无限长的表。与 tsf `HOLD_LIMIT` 同值。
pub(crate) const HOLD_LIMIT: usize = 8;

impl KeyRouter {
    /// 退格：缓冲非空 → 引擎删（引擎按码点删）；空缓冲 → 直通。
    pub(crate) fn handle_backspace(&mut self) -> KeyAction {
        if self.buffer().is_empty() {
            KeyAction::PassThrough
        } else {
            self.engine.backspace();
            self.reset_page_if_buffer_changed();
            KeyAction::EngineHandled
        }
    }

    /// 空格：缓冲非空 → 引擎提交；空缓冲 → 出 " "（镜像 `handleSpace`；空格**不宽化**，
    /// 见 `Engine::fullwidthize` 头注）。
    pub(crate) fn handle_space(&mut self) -> KeyAction {
        if self.buffer().is_empty() {
            KeyAction::Input(" ".to_string())
        } else {
            // 翻页后空格 = **当前页**首条（页码在本层，引擎不知道）
            let out = self.engine.input_space_at(self.page() * PAGE_SIZE);
            self.reset_page_if_buffer_changed();
            commit_or_handled(out)
        }
    }

    /// 回车：缓冲非空 → **按引擎的判据**（`Engine::enter_action`；五笔读「回车行为」
    /// 设置位：选首候选 / 清空 / 上屏原文 —— 三处宿主的同一语义见 `engine_enter.rs`）；
    /// 空缓冲 → 直通（镜像 `handleEnter`）。
    pub(crate) fn handle_enter(&mut self) -> KeyAction {
        if self.buffer().is_empty() {
            KeyAction::PassThrough
        } else {
            match self.engine.enter_action() {
                EnterAction::SelectFirst => commit_or_handled(self.select(0)),
                EnterAction::Clear => {
                    self.engine.clear();
                    self.reset_page_if_buffer_changed();
                    KeyAction::EngineHandled
                }
                EnterAction::CommitRaw(raw) => {
                    self.engine.clear();
                    self.reset_page_if_buffer_changed();
                    KeyAction::Input(raw)
                }
            }
        }
    }

    /// 按下时记结论：**按下是权威**（直通插入、拦下移除），同一键的陈旧残留随之清掉。
    /// ⚠️ 有界（[`HOLD_LIMIT`]）：客户端丢 keyup（切窗/失焦）时集合不能无限长；
    /// 满了挤掉最老的 —— 那一键的抬起会回落 EngineHandled（宁可拦下不可误放）。
    pub(crate) fn record_key_outcome(&mut self, keyval: u32, pass: bool) {
        self.held_passthrough.retain(|&k| k != keyval);
        if !pass {
            return;
        }
        if self.held_passthrough.len() == HOLD_LIMIT {
            self.held_passthrough.pop_front();
        }
        self.held_passthrough.push_back(keyval);
    }

    /// 抬起：返回按下时是否直通。**集合**而非单槽 —— 键盘 rollover 下另一键按下
    /// 不再顶掉前一键的记录（旧单槽会让先按的直通键在抬起时被误拦，客户端卡键）。
    pub(crate) fn release_key(&mut self, keyval: u32) -> bool {
        match self.held_passthrough.iter().position(|&k| k == keyval) {
            Some(i) => self.held_passthrough.remove(i).is_some(),
            None => false,
        }
    }

    /// 模式/方案/清空/失焦时作废全部按下记录（与 tsf `clear_held_keys` 同契约）。
    pub(crate) fn clear_held_keys(&mut self) {
        self.held_passthrough.clear();
    }

    // ---- 会话复位（`opi-ffi` raw 出口的唯一入口）----
    // 这三条都改「按下时的结论」所依赖的语境，旧记录必须一起作废；`opi-ffi`
    // 跨 crate 调不到 `clear_held_keys`，直接 `engine_mut()` 会漏清 held。

    /// 清空会话（`opi_clear`）：引擎清零 + 作废按下记录 + 页码归零。
    pub fn clear(&mut self) {
        self.engine.clear();
        self.clear_held_keys();
        self.reset_page_if_buffer_changed();
    }

    /// 换方案（`opi_set_scheme`）：引擎清缓冲 + 作废按下记录 + 页码归零。
    pub fn set_scheme(&mut self, scheme: crate::Scheme) {
        self.engine.set_scheme(scheme);
        self.clear_held_keys();
        self.reset_page_if_buffer_changed();
    }

    /// 双拼布局（`opi_set_shuangpin_layout`）：引擎只换解码表、不清缓冲，
    /// 但按下记录仍作废（同一串按键的结论已随布局改变）。
    pub fn set_shuangpin_layout(&mut self, index: usize) {
        self.engine.set_shuangpin_layout(index);
        self.clear_held_keys();
    }
}
