// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 空格/标点收尾的**当前页首条**回归（2026-10-24 用户报障「桌面端翻页后直接确认
//! 始终是第一页的 1」）。从 `candidate_tests.rs` 拆出：那边再加本用例越 500 行门禁。

use super::*;
use engine_core::dictionary::InMemoryDictionary;

fn state() -> CandidateState {
    let mut d = InMemoryDictionary::new();
    for i in 0..20 {
        d.insert("qwx", &format!("词{i:02}"), (5000 - i) as u32);
    }
    let symbols = engine_core::symbols::SymbolEngine::builtin();
    let mut s = CandidateState {
        engine: Engine::new(Box::new(d), symbols, true),
        page: 0,
        buffer_snapshot: String::new(),
        shift_state: ShiftState::Off,
        last_printable: None,
        dicts: Default::default(),
        english_pro: true,
        fetch_cache: Default::default(),
    };
    s.refresh_snapshot();
    s
}

#[test]
fn space_commits_current_page_first_candidate() {
    let mut s = state();
    for c in ['q', 'w', 'x'] {
        s.input_key(c);
    }
    s.next_page(); // 第 2 页（global 8..16）
    assert_eq!(s.candidates()[0], "词08", "前置：当前页首位");
    assert_eq!(
        s.input_space(),
        "词08",
        "空格 = 当前页首位，不是首页的 词00"
    );
    assert_eq!(s.buffer(), "");
    assert_eq!(s.page(), 0, "提交后页码归零");

    // 标点收尾同一条路（`input_punct` → 空格收尾）
    let mut s = state();
    for c in ['q', 'w', 'x'] {
        s.input_key(c);
    }
    s.next_page();
    assert_eq!(
        s.input_punct(','),
        Some("词08，".to_string()),
        "翻页后打 ',' = 当前页首位 + 标点"
    );
}
