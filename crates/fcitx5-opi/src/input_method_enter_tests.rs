// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 回车一族（2026-10-14 接线）：五笔读「回车行为」设置位 —— **判据在引擎**
//! （`engine_core::Engine::enter_action`，实现与理由见 `engine_enter.rs`），
//! 本文件钉住宿主这一侧的**消费**（三种取值 → 三种 KeyAction）。
//!
//! 与 `logic_enter_tests.rs`（TSF 轨）逐条对称 —— 两轨同构是本仓纪律。

use super::*;
use crate::candidate::{CandidateState, ShiftState};
use engine_core::dictionary::InMemoryDictionary;
use engine_core::{Scheme, WubiReturnStyle};

/// 20 个 "hao" 词条 + 五笔方案（"hao" 也是合法码串；本组只关心回车怎么收尾）。
fn wubi_state() -> CandidateState {
    let mut d = InMemoryDictionary::new();
    for i in 0..20 {
        d.insert("hao", &format!("词{i:02}"), (5000 - i) as u32);
    }
    let symbols = engine_core::symbols::SymbolEngine::builtin();
    let mut s = CandidateState {
        engine: engine_core::Engine::new(Box::new(d), symbols, true),
        page: 0,
        buffer_snapshot: String::new(),
        shift_state: ShiftState::Off,
        last_printable: None,
        dicts: Default::default(),
        english_pro: true,
    };
    s.engine.set_scheme(Scheme::Wubi);
    s.buffer_snapshot = s.engine.buffer().to_string();
    s
}

fn type_str(s: &mut CandidateState, text: &str) {
    for c in text.chars() {
        handle_key(s, c as u32, 0);
    }
}

/// 五笔 + 默认档（`Clear`）：回车**清空缓冲、什么都不上屏**（旧行为是选首候选）。
#[test]
fn wubi_enter_clear_drops_the_buffer() {
    let mut s = wubi_state();
    type_str(&mut s, "hao");
    assert_eq!(handle_key(&mut s, KEY_RETURN, 0), KeyAction::EngineHandled);
    assert_eq!(s.buffer(), "");
}

/// 五笔 + `CommitRaw`：回车把**缓冲原文**（打的码）上屏。
#[test]
fn wubi_enter_commit_raw_commits_the_buffer_text() {
    let mut s = wubi_state();
    s.engine.set_wubi_return_style(WubiReturnStyle::CommitRaw);
    type_str(&mut s, "hao");
    assert_eq!(
        handle_key(&mut s, KEY_RETURN, 0),
        KeyAction::Input("hao".into())
    );
    assert_eq!(s.buffer(), "");
}

/// 非五笔：设置位不参与，回车仍是「选首候选」（既有行为一字不动）。
#[test]
fn pinyin_enter_still_selects_the_first_candidate() {
    let mut s = wubi_state();
    s.engine.set_scheme(Scheme::Full);
    s.engine.set_wubi_return_style(WubiReturnStyle::CommitRaw); // 故意设成只有五笔认的档
    type_str(&mut s, "hao");
    assert_eq!(
        handle_key(&mut s, KEY_RETURN, 0),
        KeyAction::Input("词00".into())
    );
}
