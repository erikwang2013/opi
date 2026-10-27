// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 联想态的 TSF 逻辑层语义（2026-10-27，Spec §5，镜像 fcitx5 轨
//! `input_method_association_tests.rs` 的前提）：快照带 `association` 位、
//! 收起清缓存、点联想项走链式。
//!
//! 挂在 `logic_snapshot.rs`（`logic.rs` 已 498 行，加不了模块声明）。

use super::candidates_visible;
use crate::logic::TsfLogic;
use engine_core::composer::Mode;
use engine_data::{AssocRow, OpDict, RawEntry, load_bytes, serialize_with_extras};

/// 内存词库：今天 / 今天晚上 + assoc「今天→回家」「今天回家→吃饭」。
/// 第二条是为了让「点联想项」之后仍有链（提交后根 = 今天 + 回家）。
fn association_logic() -> TsfLogic {
    let entries = vec![
        RawEntry {
            pinyin: "jintian".into(),
            word: "今天".into(),
            freq: 900,
        },
        RawEntry {
            pinyin: "jintianwanshang".into(),
            word: "今天晚上".into(),
            freq: 700,
        },
    ];
    let bytes = serialize_with_extras(
        &OpDict {
            pinyin_total: entries.iter().map(|e| e.pinyin.len()).sum(),
            entries,
        },
        true,
        &[
            AssocRow {
                root: "今天".into(),
                suffix: "回家".into(),
                rank: 90,
            },
            AssocRow {
                root: "今天回家".into(),
                suffix: "吃饭".into(),
                rank: 80,
            },
        ],
    );
    let dict = Box::new(load_bytes(bytes).expect("测试词库应可装载"));
    let mut logic = TsfLogic::with_dict(dict);
    logic.switch_mode(Mode::Pinyin);
    for ch in "jintian".chars() {
        logic.input_key(ch as u32, 0);
    }
    assert_eq!(logic.select(0), "今天", "测试前提：jintian 首候选是今天");
    assert!(
        logic.engine.association_active(),
        "测试前提：提交后进入联想态"
    );
    logic
}

#[test]
fn snapshot_association_flag_and_visibility() {
    let logic = association_logic();
    let s = logic.snapshot();
    assert!(s.association);
    assert!(s.buffer.is_empty());
    assert!(!s.candidates.is_empty());
    assert!(
        candidates_visible(&s.buffer, &s.candidates),
        "联想态候选窗必须可见（缓冲空但候选是续接项）"
    );
}

#[test]
fn dismiss_association_clears_state_and_fetch_cache() {
    let mut logic = association_logic();
    // 造一次抓取缓存（page_count 会留下全量表）。
    let _ = logic.page_count();
    assert!(logic.fetch_cache.borrow().is_some(), "测试前提：缓存已填");
    assert!(logic.dismiss_association(), "有联想时应返回真");
    assert!(!logic.engine.association_active());
    assert!(
        logic.fetch_cache.borrow().is_none(),
        "缓存键不含联想开关，必须一并作废"
    );
    assert!(!logic.dismiss_association(), "已无联想时返回假");
}

#[test]
fn select_from_association_item_commits_suffix_and_keeps_chain() {
    let mut logic = association_logic();
    let fetched = logic.fetched();
    let index = fetched
        .iter()
        .position(|c| c.text == "回家")
        .expect("联想项里有回家");
    assert_eq!(logic.select_from(&fetched, index), "回家");
    assert_eq!(logic.engine.buffer(), "");
    assert!(
        logic.engine.association_active(),
        "链式：提交回家后文档尾是 今天回家，应有续接项吃饭"
    );
    let s = logic.snapshot();
    assert_eq!(s.candidates.first().map(String::as_str), Some("吃饭"));
}

#[test]
fn candidate_visibility_hides_only_when_both_empty() {
    let none: Vec<String> = Vec::new();
    assert!(!candidates_visible("", &none));
    assert!(candidates_visible("ni", &none));
    assert!(candidates_visible("", &["回家".to_string()]));
}
