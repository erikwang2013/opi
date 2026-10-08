// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 数字键分派（2026-10-09 从 `router.rs` 拆出 —— 加「输入方案」后那里越了 500 行，
//! 与 `engine_punct.rs` 同一套拆分先例）。
//!
//! 两部分，**顺序即语义**：
//!   1. [`KeyRouter::digit_dispatch`]：9 键方案下数字是**输入键**（`scheme.rs`），
//!      必须排在选词判定**之前** —— 否则缓冲一有候选，数字就被当页内选词吃掉，
//!      9 键一个拼音都打不出来（门只开在这一处，单源）；
//!   2. [`KeyRouter::digit_select`]：既有的页内选词（语义与判据见其文档）。
//!
//! 拆出的前提：两块都只经 `KeyRouter` 的 pub/pub(crate) 方法访问（`fetched` /
//! `mode` / `buffer` / `select_from`），不碰任何私有字段 —— 往这里加东西时守住这条，
//! 否则就要去动 `router.rs` 的结构体可见性。

use super::{KeyAction, KeyRouter};
use crate::scheme::Scheme;

impl KeyRouter {
    /// 数字键统一入口：方案门 + 页内选词。
    ///
    /// 9 键方案：2–9 进缓冲（输入键）；0/1 不是输入键、也不该落到选词（9 键布局上
    /// 它们另有用途）→ 直通交宿主。其余方案：照旧走选词（`digit_select`）。
    pub(crate) fn digit_dispatch(&mut self, c: char) -> KeyAction {
        if self.engine().scheme() == Scheme::Nine && c.is_ascii_digit() {
            if matches!(c, '2'..='9') {
                let out = self.engine.input_key(c);
                self.reset_page_if_buffer_changed();
                return super::commit_or_handled(out);
            }
            return KeyAction::PassThrough;
        }
        self.digit_select(c)
    }

    /// 拼音/繁体/符号模式有候选时按数字选词（页内索引：'1'→第 0 个候选）；否则直通。
    /// 无对应候选（本页没有第 9 项、'0'）不消费，交客户端处理 —— 本函数**不自己数**页内
    /// 条数：页内越界由 `select_from` 的页内判据兜住（→ 空串 → 直通）。两处各数一遍正是
    /// 缺陷的来源（旧 `select` 只判**全量**越界：数字键走对了、点击候选走错了；用例见
    /// router_invariants.rs::digit_beyond_page_... 与 select_index_bounds.rs::select_beyond_...）。
    /// 同一份抓取供选中用：旧路径 `candidates()` + `select()` 抓两次 ⇒ 每次数字选词排两遍。
    pub(crate) fn digit_select(&mut self, c: char) -> KeyAction {
        let fetched = self.fetched();
        if self.mode().digit_selects_candidates() && !self.buffer().is_empty() {
            let Some(idx) = c.to_digit(10).and_then(|d| d.checked_sub(1)) else {
                return KeyAction::PassThrough;
            };
            // 与 `select` 同一套换算与收尾，只是复用上面那次抓取
            let text = self.select_from(&fetched, idx as usize);
            // 空串 = 页内没这一项（或无候选）→ 不消费，交客户端处理
            if text.is_empty() {
                KeyAction::PassThrough
            } else {
                KeyAction::Input(text)
            }
        } else {
            KeyAction::PassThrough
        }
    }
}
