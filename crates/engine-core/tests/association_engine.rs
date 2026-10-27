// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 提交后联想（Task 5）：状态、生成配比与候选注入。
//! 夹具 `AssocDict` 的 `prefix` / `suffix` 按 root 精确查表，故意不做字符串扫描 ——
//! 这样「链根无命中」与「assoc 与前缀项的前缀关系」都是可控输入。

mod common;

use common::{AssocDict, type_str};
use engine_core::dictionary::InMemoryDictionary;
use engine_core::symbols::SymbolEngine;
use engine_core::{Dictionary, Engine, Mode};

fn engine() -> Engine {
    let d = AssocDict::new()
        .word("jintian", "今天", 900)
        .word("tian", "天", 400)
        .prefix(
            "今天",
            &[
                ("今天", 900),     // 与根相同：跳过
                ("今天晚上", 700), // 续接「晚上」
                ("今天下午", 600), // 「下午」
                ("今天天气", 550), // 「天气」
                ("今天早上", 540), // 「早上」
                ("今天上午", 530), // 「上午」
                ("今天回家", 520), // 「回家」
                ("今天气", 500),   // 「气」1 字：只够填 b 的 2–6 字约束，不入选
            ],
        )
        .suffix("今天", &[("吃饭", 90), ("真好", 80), ("晚上好", 70)]);
    Engine::new(Box::new(d), SymbolEngine::builtin(), true)
}

fn find(e: &Engine, word: &str) -> usize {
    e.candidates(usize::MAX)
        .iter()
        .position(|c| c.text == word)
        .unwrap_or_else(|| panic!("候选里没有 {word}"))
}

#[test]
fn two_char_commit_shows_continuations_without_root() {
    let mut e = engine();
    type_str(&mut e, "jintian");
    let idx = find(&e, "今天");
    assert_eq!(e.select(idx), "今天");
    assert!(e.association_active());
    let got: Vec<String> = e.candidates(8).into_iter().map(|c| c.text).collect();
    // 3 前缀（晚上/下午/天气）+ 3 长前缀（早上/上午/回家）+ 2 assoc（吃饭/真好）；
    // 「晚上好」与已选「晚上」有前缀关系，跳过。
    assert_eq!(
        got,
        vec![
            "晚上", "下午", "天气", "早上", "上午", "回家", "吃饭", "真好"
        ]
    );
    assert!(!got.iter().any(|s| s.contains("今天")));
}

#[test]
fn single_char_commit_does_not_associate() {
    let mut e = engine();
    type_str(&mut e, "tian");
    let idx = find(&e, "天");
    assert_eq!(e.select(idx), "天");
    assert!(!e.association_active());
}

#[test]
fn dictionary_default_methods_return_empty() {
    // `Dictionary` 的两个默认方法（Spec §3.7）：`InMemoryDictionary` 不覆写 ⇒ 一律空。
    let mut d = InMemoryDictionary::new();
    d.insert("nihao", "你好", 900);
    assert!(d.words_with_prefix("").is_empty());
    assert!(d.words_with_prefix("你好").is_empty());
    assert!(d.association_suffixes("你好").is_empty());
}

#[test]
fn in_memory_dictionary_does_not_associate() {
    let mut d = InMemoryDictionary::new();
    d.insert("nihao", "你好", 900);
    let mut e = Engine::new(Box::new(d), SymbolEngine::builtin(), true);
    type_str(&mut e, "nihao");
    let idx = find(&e, "你好");
    assert_eq!(e.select(idx), "你好");
    assert!(!e.association_active(), "无词索引/assoc 的词典不得产生联想");
}

#[test]
fn non_chinese_modes_do_not_associate() {
    for mode in [Mode::English, Mode::Number, Mode::Symbol] {
        let mut e = engine();
        e.switch_mode(mode);
        type_str(&mut e, "nihao");
        let _ = e.select(0); // 有候选就提交；没有就是空串，两者都不得建联想
        assert!(!e.association_active(), "{mode:?} 提交后不得联想");
    }
}

#[test]
fn limit_truncates_association() {
    let mut e = engine();
    type_str(&mut e, "jintian");
    let idx = find(&e, "今天");
    e.select(idx);
    assert_eq!(e.candidates(3).len(), 3);
    assert_eq!(e.candidates(0).len(), 0);
}

#[test]
fn prefix_continuations_are_deduped_by_text() {
    let d = AssocDict::new()
        .word("jintian", "今天", 900)
        // 同词多读音在词索引里是两条：续接相同 ⇒ 只出现一次，取最大权重。
        .prefix("今天", &[("今天晚上", 700), ("今天晚上", 650)]);
    let mut e = Engine::new(Box::new(d), SymbolEngine::builtin(), true);
    type_str(&mut e, "jintian");
    assert_eq!(e.select(0), "今天");
    let got: Vec<String> = e.candidates(8).into_iter().map(|c| c.text).collect();
    assert_eq!(got, vec!["晚上"]);
}

#[test]
fn assoc_suffix_with_prefix_relation_is_skipped() {
    let d = AssocDict::new()
        .word("jintian", "今天", 900)
        .prefix("今天", &[("今天晚上", 700)])
        .suffix("今天", &[("晚上好", 90), ("回家", 80)]);
    let mut e = Engine::new(Box::new(d), SymbolEngine::builtin(), true);
    type_str(&mut e, "jintian");
    assert_eq!(e.select(0), "今天");
    let got: Vec<String> = e.candidates(8).into_iter().map(|c| c.text).collect();
    assert_eq!(
        got,
        vec!["晚上", "回家"],
        "「晚上好」与「晚上」前缀相关，跳过"
    );
}
