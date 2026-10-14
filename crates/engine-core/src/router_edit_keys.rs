// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **编辑键一族**（退格 / 空格 / 回车）—— 2026-10-14 从 `router.rs` 拆出（那边加回车
//! 判据接线后 512 行，撞 500 行门禁；拆分先例与 `candidates_*.rs` 同套，语义一字未动）。
//!
//! 三个键共同点：都先判「缓冲空不空」再分流（空 = 直通 / 出空格），且都要在改缓冲后
//! `reset_page_if_buffer_changed()` —— 与 ⇧ / 翻页 / 可打印键那几族分开写更好读。
//! 回车那条的判据来自 [`crate::Engine::enter_action`]（实现见 `engine_enter.rs`）。

use crate::EnterAction;
use crate::router::{KeyAction, KeyRouter, commit_or_handled};

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
            let out = self.engine.input_space();
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
}
