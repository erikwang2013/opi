// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 智能输入第一批：选词来源权重 + 误选撤销 + 英文学词频（2026-10-26）。
//!
//! 权重口径：空格接受首候选 = +1（`SelectionSource::AcceptFirst`），显式点选 /
//! 数字选词 = +2（`Explicit`）。误选撤销只认**同一 buffer + mode + scheme** 下
//! 的显式改选；`clear` / 切模式 / 换方案清链。英文候选按学习次数优先、静态词频次之。
//!
//! 频次断言走公开导出 `export_user_words()`（引擎不暴露内部 `Learner` 句柄）。

use engine_core::dictionary::InMemoryDictionary;
use engine_core::scheme::Scheme;
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Mode};

fn symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

/// 开启学习器的引擎；`entries` = (拼音, 词, 静态词频)。
fn engine(entries: &[(&str, &str, u32)]) -> Engine {
    let mut d = InMemoryDictionary::new();
    for (pinyin, word, freq) in entries {
        d.insert(pinyin, word, *freq);
    }
    Engine::new(Box::new(d), symbols(), true)
}

fn typed(e: &mut Engine, s: &str) {
    for ch in s.chars() {
        e.input_key(ch);
    }
}

/// 学习频次（无记录 = 0）。导出是公开 API，测试不碰私有字段。
fn freq(e: &Engine, word: &str) -> u32 {
    let v: serde_json::Value = serde_json::from_str(&e.export_user_words()).unwrap();
    v["words"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["text"].as_str() == Some(word))
        .map(|w| w["freq"].as_u64().unwrap() as u32)
        .unwrap_or(0)
}

/// 按文本找到候选的全局下标并选中（选中本身会重算同一张表）。
fn select_text(e: &mut Engine, text: &str) -> String {
    let cands = e.candidates(usize::MAX);
    let idx = cands
        .iter()
        .position(|c| c.text == text)
        .unwrap_or_else(|| panic!("候选里没有 {text}"));
    e.select(idx)
}

fn two_words() -> Engine {
    engine(&[("ni", "你", 9000), ("ni", "泥", 8000)])
}

// ---------------------------------------------------------------- 来源权重

/// 空格接受首候选 +1；同一词再显式点选 +2（累计 3）。
#[test]
fn space_accept_first_is_one_explicit_is_two() {
    let mut e = two_words();
    typed(&mut e, "ni");
    assert_eq!(e.input_space(), "你");
    assert_eq!(freq(&e, "你"), 1, "空格接受首候选 = +1");

    typed(&mut e, "ni");
    assert_eq!(select_text(&mut e, "你"), "你");
    assert_eq!(freq(&e, "你"), 3, "显式点选 = +2");
}

// ---------------------------------------------------------------- 误选撤销

/// 同一输入下先显式选 A、再显式改选 B：A 的那次学习被撤销，B 记 +2。
#[test]
fn explicit_reselection_under_same_input_undoes_previous() {
    let mut e = two_words();
    typed(&mut e, "ni");
    assert_eq!(select_text(&mut e, "你"), "你");
    assert_eq!(freq(&e, "你"), 2);

    typed(&mut e, "ni");
    assert_eq!(select_text(&mut e, "泥"), "泥");
    assert_eq!(freq(&e, "你"), 0, "改选即撤销 A（且退到 0 后不留条目）");
    assert_eq!(freq(&e, "泥"), 2);
}

/// 空格接受（+1）之后在同一输入下显式改选：按**当时记下的权重**退 1。
#[test]
fn explicit_reselection_undoes_prior_accept_first_by_its_weight() {
    let mut e = two_words();
    typed(&mut e, "ni");
    assert_eq!(e.input_space(), "你");
    assert_eq!(freq(&e, "你"), 1);

    typed(&mut e, "ni");
    assert_eq!(select_text(&mut e, "泥"), "泥");
    assert_eq!(freq(&e, "你"), 0, "AcceptFirst 的那一票退 1，不是退 2");
    assert_eq!(freq(&e, "泥"), 2);
}

/// 不同输入不是改选：两次显式选择各自保留。
#[test]
fn different_input_keeps_both_selections() {
    let mut e = engine(&[("ni", "你", 9000), ("hao", "好", 8000)]);
    typed(&mut e, "ni");
    assert_eq!(select_text(&mut e, "你"), "你");

    typed(&mut e, "hao");
    assert_eq!(select_text(&mut e, "好"), "好");
    assert_eq!(freq(&e, "你"), 2, "输入不同 ⇒ 不撤销");
    assert_eq!(freq(&e, "好"), 2);
}

/// 重复显式选同一个词不是改选：继续累加，不撤销。
#[test]
fn repeated_same_choice_accumulates() {
    let mut e = two_words();
    typed(&mut e, "ni");
    assert_eq!(select_text(&mut e, "你"), "你");
    typed(&mut e, "ni");
    assert_eq!(select_text(&mut e, "你"), "你");
    assert_eq!(freq(&e, "你"), 4);
}

/// `clear` 清链：清空会话后的下一次显式选择不再撤销上一次。
#[test]
fn clear_resets_undo_chain() {
    let mut e = two_words();
    typed(&mut e, "ni");
    assert_eq!(select_text(&mut e, "你"), "你");

    e.clear();
    typed(&mut e, "ni");
    assert_eq!(select_text(&mut e, "泥"), "泥");
    assert_eq!(freq(&e, "你"), 2, "清会话后不撤销");
    assert_eq!(freq(&e, "泥"), 2);
}

/// 切模式清链（含切回来）：跨模式的选择不是同一次输入的改选。
#[test]
fn mode_switch_resets_undo_chain() {
    let mut e = two_words();
    typed(&mut e, "ni");
    assert_eq!(select_text(&mut e, "你"), "你");

    e.switch_mode(Mode::English);
    e.switch_mode(Mode::Pinyin);
    typed(&mut e, "ni");
    assert_eq!(select_text(&mut e, "泥"), "泥");
    assert_eq!(freq(&e, "你"), 2, "跨模式不撤销");
    assert_eq!(freq(&e, "泥"), 2);
}

/// 换方案清链（含换回来）。
#[test]
fn scheme_switch_resets_undo_chain() {
    let mut e = two_words();
    typed(&mut e, "ni");
    assert_eq!(select_text(&mut e, "你"), "你");

    e.set_scheme(Scheme::Nine);
    e.set_scheme(Scheme::Full);
    typed(&mut e, "ni");
    assert_eq!(select_text(&mut e, "泥"), "泥");
    assert_eq!(freq(&e, "你"), 2, "跨方案不撤销");
    assert_eq!(freq(&e, "泥"), 2);
}

// ---------------------------------------------------------------- 英文学习词频

/// 学过一次的英文词排到所有未学词之前（静态词频更高的 `help` 也让位）。
#[test]
fn english_learned_word_ranks_first() {
    let mut e = engine(&[]);
    e.switch_mode(Mode::English);
    typed(&mut e, "hel");
    assert_eq!(
        e.candidates(1)[0].text,
        "help",
        "前置：未学习时静态词频最高"
    );

    assert_eq!(select_text(&mut e, "hello"), "hello");
    assert_eq!(freq(&e, "hello"), 2);

    typed(&mut e, "hel");
    assert_eq!(e.candidates(1)[0].text, "hello", "学习次数优先于静态词频");
}

/// 学习对大小写不敏感：大写缓冲下记的 `Hello`，小写缓冲查询也要算数。
#[test]
fn english_learning_is_case_insensitive_across_buffers() {
    let mut e = engine(&[]);
    e.switch_mode(Mode::English);
    typed(&mut e, "Hel");
    assert_eq!(select_text(&mut e, "Hello"), "Hello");
    assert_eq!(freq(&e, "Hello"), 2);

    typed(&mut e, "hel");
    assert_eq!(e.candidates(1)[0].text, "hello");
}
