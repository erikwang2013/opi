// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 联想状态的生命周期（Task 6 清空矩阵）：任何会改变输入上下文的入口都必须
//! 先收起联想 —— 否则一个按键之后候选行还停在上一次的联想上。

mod common;

use common::{AssocDict, type_str};
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Mode, Scheme};

fn fresh_engine() -> Engine {
    let d = AssocDict::new()
        .word("jintian", "今天", 900)
        .prefix("今天", &[("今天晚上", 700)])
        .suffix("今天", &[("吃饭", 90)]);
    Engine::new(Box::new(d), SymbolEngine::builtin(), true)
}

fn active_engine() -> Engine {
    let mut e = fresh_engine();
    type_str(&mut e, "jintian");
    assert_eq!(e.select(0), "今天");
    assert!(e.association_active(), "前置：提交后进入联想态");
    e
}

/// 已选「今天」；前缀链 今天 → 今天晚上 → 今天晚上好。
fn active_engine_with_chain() -> Engine {
    let d = AssocDict::new()
        .word("jintian", "今天", 900)
        .prefix("今天", &[("今天晚上", 700)])
        .prefix("今天晚上", &[("今天晚上好", 600)]);
    let mut e = Engine::new(Box::new(d), SymbolEngine::builtin(), true);
    type_str(&mut e, "jintian");
    assert_eq!(e.select(0), "今天");
    assert!(e.association_active(), "前置：提交后进入联想态");
    e
}

/// 已选「中国」；`with_tail` 决定「中国人」是否还有下一层（链的自然终点）。
fn chain_engine(with_tail: bool) -> Engine {
    let mut d = AssocDict::new()
        .word("zhongguo", "中国", 900)
        .prefix("中国", &[("中国人", 500)]);
    if with_tail {
        d = d.prefix("中国人", &[("中国人多", 400)]);
    }
    let mut e = Engine::new(Box::new(d), SymbolEngine::builtin(), true);
    type_str(&mut e, "zhongguo");
    assert_eq!(e.select(0), "中国");
    assert!(e.association_active(), "前置：提交后进入联想态");
    e
}

#[test]
fn every_input_entry_clears_association() {
    let mut e = active_engine();
    e.input_key('a');
    assert!(!e.association_active(), "input_key");

    let mut e = active_engine();
    e.backspace();
    assert!(!e.association_active(), "backspace");

    let mut e = active_engine();
    let _ = e.input_punct_at('，', 0);
    assert!(!e.association_active(), "input_punct_at");

    let mut e = active_engine();
    e.clear();
    assert!(!e.association_active(), "clear");

    let mut e = active_engine();
    e.switch_mode(Mode::English);
    assert!(!e.association_active(), "switch_mode");

    let mut e = active_engine();
    e.switch_language(Mode::English, Scheme::Full);
    assert!(!e.association_active(), "switch_language");

    let mut e = active_engine();
    e.set_scheme(Scheme::Nine);
    assert!(!e.association_active(), "set_scheme");

    let mut e = active_engine();
    e.toggle_symbol();
    assert!(!e.association_active(), "toggle_symbol");

    let mut e = active_engine();
    e.set_shift(true);
    assert!(!e.association_active(), "set_shift");
}

#[test]
fn dictionary_setters_clear_association() {
    let mut e = active_engine();
    e.set_extra_dicts(Vec::new());
    assert!(!e.association_active(), "set_extra_dicts");

    let mut e = active_engine();
    e.set_trad_dict(None);
    assert!(!e.association_active(), "set_trad_dict");

    let mut e = active_engine();
    e.set_stroke_dict(None);
    assert!(!e.association_active(), "set_stroke_dict");

    let mut e = active_engine();
    e.set_wubi_dict(None);
    assert!(!e.association_active(), "set_wubi_dict");
}

#[test]
fn clicking_continuation_commits_only_continuation_and_chains() {
    let mut e = active_engine_with_chain();
    let before = e.export_user_words(); // 「今天」本身的记账发生在点击之前
    let first = e.candidates(8);
    let idx = first.iter().position(|c| c.text == "晚上").unwrap();
    assert_eq!(e.select_from(&first, idx), "晚上");
    assert_eq!(e.buffer(), "");
    assert!(e.association_active());
    let second: Vec<String> = e.candidates(8).into_iter().map(|c| c.text).collect();
    assert!(second.iter().all(|s| s != "今天晚上"));
    assert!(second.iter().any(|s| s == "好"), "链式根 = 今天晚上");
    assert_eq!(e.export_user_words(), before, "联想点击不得记账");
}

#[test]
fn one_char_continuation_can_chain() {
    let mut e = chain_engine(true); // 中国 → 中国人 → 中国人多
    assert_eq!(e.select(0), "人");
    assert!(e.association_active());
}

#[test]
fn chain_ends_when_new_root_has_no_hits() {
    let mut e = chain_engine(false); // 无「中国人」的索引
    let _ = e.select(0);
    assert!(!e.association_active(), "中国人 无命中 → 联想结束");
}

#[test]
fn consecutive_association_clicks_do_not_change_learning() {
    let mut e = active_engine_with_chain();
    let before = e.export_user_words();
    let first = e.candidates(8);
    let idx = first.iter().position(|c| c.text == "晚上").unwrap();
    assert_eq!(e.select_from(&first, idx), "晚上");
    let second = e.candidates(8);
    let idx = second.iter().position(|c| c.text == "好").unwrap();
    assert_eq!(e.select_from(&second, idx), "好");
    assert_eq!(e.export_user_words(), before, "连续联想点击不得记账");
    assert!(!e.association_active(), "链到头 → 联想结束");
}

#[test]
fn space_at_entry_dismisses_and_never_selects_association() {
    let mut e = active_engine();
    assert_eq!(e.input_space_at(0), "");
    assert!(!e.association_active());
    assert_eq!(e.buffer(), "");
}

#[test]
fn space_commit_enters_association() {
    let mut e = fresh_engine();
    type_str(&mut e, "jintian");
    assert_eq!(e.input_space_at(0), "今天");
    assert!(e.association_active(), "空格提交后同样进入联想态");
}

#[test]
fn punctuation_flush_does_not_build_association() {
    let mut e = fresh_engine();
    type_str(&mut e, "jintian");
    assert_eq!(e.input_punct_at(',', 0).as_deref(), Some("今天，"));
    assert!(!e.association_active(), "标点冲刷路径不建联想");
}

#[test]
fn nine_scheme_tail_kept_does_not_build_association() {
    let d = AssocDict::new()
        .word("ni", "你", 900)
        .prefix("你", &[("你好", 500)]);
    let mut e = Engine::new(Box::new(d), SymbolEngine::builtin(), true);
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "6442"); // ni 后面还留 42 两键
    let cands = e.candidates(usize::MAX);
    let idx = cands
        .iter()
        .position(|c| c.text == "你")
        .expect("候选里没有 你");
    assert_eq!(e.select(idx), "", "留尾 = 本轮不提交");
    assert!(!e.association_active(), "留尾不得建链");
}
