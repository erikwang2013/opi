// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **回车一族**（2026-10-14 接线）—— 单独成文件：`engine_wubi.rs`（481 行）与
//! `engine.rs`（493 行）都贴着 500 行门禁，这族塞不进任何一边；拆分先例与
//! `engine_punct.rs` / `engine_readings.rs` / `engine_segment.rs` 同套。
//!
//! ## 为什么要有这一层（缺口 → 接线）
//!
//! M4 把「五笔特殊习惯 · 回车键（缓冲非空时）：清空 / 上屏原文」做成了**设置位**
//! （`WubiReturnStyle`，四端能存能读），但**消费点在四端路由**没接 —— 三端各自写着
//! 「回车 = 选首候选」（`router.rs::handle_enter`、`fcitx5-opi/input_method.rs::handle_enter`、
//! `tsf-opi/logic_input_method.rs::handle_enter`、Android `KeyRouter.handleEnter`），
//! 设置位等于摆设。
//!
//! 接线方式**不是**在三处各写一遍判据（本仓「同一语义抄三份」的教训见
//! `router.rs` 键路由表那条）：判据只此一份（[`Engine::enter_action`]），三个宿主
//! 各问一次、各按自己的通道执行（提交文本 / 清缓冲 / 直通）。
//!
//! ## 判据（唯一实现，改之前先读）
//!
//! - **只对五笔方案生效**：这是「五笔特殊习惯」里的一条，拼音 / 双拼 / 9 键 / 笔画的
//!   回车语义是「选首候选」，**一字不动**（`SelectFirst`）。混输（五笔拼音）同属
//!   `Scheme::Wubi` ⇒ 同样生效。
//! - **空缓冲不在此列**：宿主自己直通（回车交给应用/换行），本层不参与 —— 与
//!   `handle_enter` 三处的既有判据同一条边界（`buffer().is_empty()` 先判）。
//! - `Clear` = 清空缓冲、**什么都不上屏**（Sogou 的「清空」语义）；
//!   `CommitRaw` = 把缓冲里的**原文**（打的码/字母）上屏。

use crate::engine::Engine;
use crate::engine_wubi_impl::WubiReturnStyle;
use crate::scheme::Scheme;

/// 回车（缓冲非空时）该做什么 —— 由 [`Engine::enter_action`] 给出，宿主照做。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnterAction {
    /// 选首候选上屏（**非五笔方案恒此**，也是五笔的既有行为之前的语义）。
    SelectFirst,
    /// 清空缓冲、不上屏（`WubiReturnStyle::Clear`，引擎缺省值）。
    Clear,
    /// 上屏**原文**（缓冲原样，`WubiReturnStyle::CommitRaw`）。
    CommitRaw(String),
}

impl Engine {
    /// 回车键（缓冲非空时）该做什么 —— **判据的单一实现**，四端路由都问这一处。
    ///
    /// 空缓冲时返回 `SelectFirst`（= 既有行为，宿主那边照旧直通；本层不表达「直通」，
    /// 因为「有没有缓冲」本来就是宿主先判的）。
    pub fn enter_action(&self) -> EnterAction {
        if self.scheme() != Scheme::Wubi || self.buffer().is_empty() {
            return EnterAction::SelectFirst;
        }
        match self.wubi_return_style() {
            WubiReturnStyle::Clear => EnterAction::Clear,
            WubiReturnStyle::CommitRaw => EnterAction::CommitRaw(self.buffer().to_string()),
        }
    }
}
