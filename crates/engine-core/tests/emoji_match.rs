// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 输入可匹配表情（2026-10-18 用户需求「增加输入可匹配表情」）。
//!
//! 行为契约（数据 `data/raw/emoji_keywords.tsv`，项内 PS 见文件头）：
//! - 拼音/繁体（26 键）与英文模式下，缓冲是某个小写关键字的前缀 ⇒ 该 emoji 并入候选；
//! - emoji **追加在文本候选之后**：它们可以填满 limit 的空位，但绝不顶掉任何文本候选
//!   （`nihao` / `hello` 的首位仍分别是「你好」/ hello）；
//! - 同一 emoji 只出现一次；关键字/缓冲大小写不敏感；单键不触发（避免单字母噪声）；
//! - 提交走既有 `select` 通道，文本就是 emoji。
//!
//! 夹具用**空符号表 + 小词库**：断言只测本层，不受 `symbols.tsv` 数据牵连。

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

fn emojis(e: &Engine, limit: usize) -> Vec<String> {
    e.candidates(limit)
        .into_iter()
        .filter(|c| c.kind == CandidateKind::Emoji)
        .map(|c| c.text)
        .collect()
}

#[test]
fn pinyin_weixiao_yields_smiling_emoji() {
    let mut e = empty_engine();
    typed(&mut e, "weixiao");
    assert!(emojis(&e, usize::MAX).iter().any(|t| t == "😊"));
}

#[test]
fn pinyin_smile_yields_grinning_emoji() {
    let mut e = empty_engine();
    typed(&mut e, "smile");
    assert!(emojis(&e, usize::MAX).iter().any(|t| t == "😄"));
}

#[test]
fn short_pinyin_ku_yields_crying_emoji() {
    let mut e = empty_engine();
    typed(&mut e, "ku");
    assert!(emojis(&e, usize::MAX).iter().any(|t| t == "😭"));
}

#[test]
fn xiao_yields_multiple_smiley_choices() {
    let mut e = empty_engine();
    typed(&mut e, "xiao");
    let got = emojis(&e, usize::MAX);
    assert!(got.len() >= 2, "xiao 应命中多个笑脸，实得 {got:?}");
    assert!(got.iter().any(|t| t == "😄"));
    assert!(got.iter().any(|t| t == "😊"));
}

#[test]
fn traditional_mode_matches_too() {
    let mut e = empty_engine();
    e.switch_mode(Mode::Traditional);
    typed(&mut e, "weixiao");
    assert!(emojis(&e, usize::MAX).iter().any(|t| t == "😊"));
}

#[test]
fn english_mode_matches_annotations() {
    let mut e = empty_engine();
    e.switch_mode(Mode::English);
    typed(&mut e, "smile");
    assert!(emojis(&e, usize::MAX).iter().any(|t| t == "😄"));
}

#[test]
fn matching_is_case_insensitive() {
    let mut e = empty_engine();
    e.switch_mode(Mode::English);
    typed(&mut e, "SMILE");
    assert!(emojis(&e, usize::MAX).iter().any(|t| t == "😄"));

    let mut e = empty_engine();
    e.switch_mode(Mode::English);
    typed(&mut e, "Wave");
    assert!(emojis(&e, usize::MAX).iter().any(|t| t == "👋"));
}

#[test]
fn no_match_produces_no_emoji() {
    let mut e = empty_engine();
    typed(&mut e, "zzzzzz");
    assert!(emojis(&e, usize::MAX).is_empty());

    let mut e = empty_engine();
    e.switch_mode(Mode::English);
    typed(&mut e, "zzzzzz");
    assert!(emojis(&e, usize::MAX).is_empty());
}

#[test]
fn single_key_does_not_trigger_emoji_layer() {
    let mut e = empty_engine();
    typed(&mut e, "x");
    assert!(emojis(&e, usize::MAX).is_empty());

    let mut e = empty_engine();
    e.switch_mode(Mode::English);
    typed(&mut e, "s");
    assert!(
        emojis(&e, 64).is_empty(),
        "单字母的英文候选里不掺 emoji 噪声"
    );
}

#[test]
fn common_pinyin_word_keeps_first_candidate() {
    let mut d = InMemoryDictionary::new();
    d.insert("nihao", "你好", 999_999);
    d.insert("ni", "你", 100);
    d.insert("hao", "好", 100);
    let mut e = Engine::new(
        Box::new(d),
        SymbolEngine::new(Vec::new(), Vec::new()),
        false,
    );
    typed(&mut e, "nihao");
    let got = e.candidates(usize::MAX);
    assert_eq!(got[0].text, "你好", "emoji 不得顶掉文本首候选");
    assert!(
        got.iter()
            .any(|c| c.kind == CandidateKind::Emoji && c.text == "👋"),
        "nihao 应能匹配到挥手 emoji"
    );
}

#[test]
fn english_common_word_keeps_first_candidate() {
    let mut e = empty_engine();
    e.switch_mode(Mode::English);
    typed(&mut e, "hello");
    let got = e.candidates(usize::MAX);
    assert_eq!(got[0].text, "hello", "英文词候选必须排在 emoji 之前");
    assert!(
        got.iter()
            .any(|c| c.kind == CandidateKind::Emoji && c.text == "👋")
    );
}

#[test]
fn limit_is_respected_and_emoji_never_displaces_text() {
    // 文本候选填满 limit ⇒ 一个 emoji 也进不来。
    let mut d = InMemoryDictionary::new();
    d.insert("xiao", "笑", 3000);
    d.insert("xiao", "小", 2000);
    d.insert("xiao", "校", 1000);
    let mut e = Engine::new(
        Box::new(d),
        SymbolEngine::new(Vec::new(), Vec::new()),
        false,
    );
    typed(&mut e, "xiao");
    let got = e.candidates(3);
    assert_eq!(got.len(), 3);
    assert!(got.iter().all(|c| c.kind == CandidateKind::Hanzi));

    // 无文本候选时 emoji 只按 limit 出。
    let mut e = empty_engine();
    typed(&mut e, "xiao");
    let got = e.candidates(3);
    assert_eq!(got.len(), 3);
    assert!(got.iter().all(|c| c.kind == CandidateKind::Emoji));
}

#[test]
fn emoji_candidates_are_unique() {
    let mut e = empty_engine();
    typed(&mut e, "xiao");
    let got = emojis(&e, usize::MAX);
    let mut uniq = got.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(got.len(), uniq.len(), "同一 emoji 只能出现一次：{got:?}");
}

#[test]
fn emoji_commits_through_existing_select_channel() {
    let mut e = empty_engine();
    typed(&mut e, "weixiao");
    let got = e.candidates(usize::MAX);
    let idx = got
        .iter()
        .position(|c| c.text == "😊")
        .expect("😊 应在候选里");
    assert_eq!(e.select(idx), "😊");
    assert_eq!(e.buffer(), "");
}
