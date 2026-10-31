// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 输入符号/标点/颜文字名称（2026-10-31 用户需求：中文模式打中文名、英文模式打
//! 英文名，匹配对应符号/标点/颜文字/表情）。
//!
//! 表情由 `emoji_match.rs` 覆盖；本文件只钉新加的 `name_keywords` 层：
//! - 拼音/繁体模式用拼音名，英文模式用英文名（不互串）；
//! - 追加在文本候选之后，绝不顶掉文本首候选；
//! - 颜文字同组最多 8 条；单键不触发；limit 生效。

use engine_core::candidates::CandidateKind;
use engine_core::dictionary::InMemoryDictionary;
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Mode};

fn empty_engine() -> Engine {
    Engine::new(
        Box::new(InMemoryDictionary::new()),
        SymbolEngine::new(Vec::new(), Vec::new()),
        false,
    )
}

fn typed(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

fn symbol_texts(e: &Engine, limit: usize) -> Vec<String> {
    e.candidates(limit)
        .into_iter()
        .filter(|c| c.kind == CandidateKind::Symbol)
        .map(|c| c.text)
        .collect()
}

#[test]
fn pinyin_symbol_names_match_punctuation_symbols_and_kaomoji() {
    let mut e = empty_engine();
    typed(&mut e, "zuokuohao");
    assert!(symbol_texts(&e, usize::MAX).iter().any(|t| t == "（"));

    let mut e = empty_engine();
    typed(&mut e, "jiantou");
    assert!(symbol_texts(&e, usize::MAX).iter().any(|t| t == "→"));

    let mut e = empty_engine();
    typed(&mut e, "kaixin");
    let got = symbol_texts(&e, usize::MAX);
    assert!(!got.is_empty(), "开心应命中颜文字");
    assert!(got.len() <= 8, "颜文字同组最多 8 条，实得 {}", got.len());
}

#[test]
fn english_symbol_names_match_in_english_mode() {
    let mut e = empty_engine();
    e.switch_mode(Mode::English);
    typed(&mut e, "leftbracket");
    let got = symbol_texts(&e, usize::MAX);
    assert!(got.iter().any(|t| t == "["));
    assert!(!got.iter().any(|t| t == "【"), "英文模式不该出中文方头括号");

    let mut e = empty_engine();
    e.switch_mode(Mode::English);
    typed(&mut e, "arrow");
    assert!(symbol_texts(&e, usize::MAX).iter().any(|t| t == "→"));
}

#[test]
fn name_languages_do_not_cross_modes() {
    let mut e = empty_engine();
    e.switch_mode(Mode::English);
    typed(&mut e, "zuokuohao");
    assert!(symbol_texts(&e, usize::MAX).is_empty());
}

#[test]
fn traditional_mode_matches_too() {
    let mut e = empty_engine();
    e.switch_mode(Mode::Traditional);
    typed(&mut e, "zuokuohao");
    assert!(symbol_texts(&e, usize::MAX).iter().any(|t| t == "（"));
}

#[test]
fn text_candidate_stays_first() {
    let mut d = InMemoryDictionary::new();
    d.insert("nihao", "你好", 999_999);
    let mut e = Engine::new(
        Box::new(d),
        SymbolEngine::new(Vec::new(), Vec::new()),
        false,
    );
    typed(&mut e, "nihao");
    let got = e.candidates(usize::MAX);
    assert_eq!(got[0].text, "你好", "符号/颜文字不得顶掉文本首候选");
}

#[test]
fn limit_is_respected_and_single_key_does_not_trigger() {
    let mut e = empty_engine();
    typed(&mut e, "kaixin");
    assert!(e.candidates(3).len() <= 3);

    let mut e = empty_engine();
    typed(&mut e, "k");
    assert!(symbol_texts(&e, usize::MAX).is_empty());
}
