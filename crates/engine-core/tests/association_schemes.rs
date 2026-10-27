// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 联想与方案/词典路由的交界（Task 8）：五笔自动上屏、9 键留尾，以及
//! 繁体模式是否读 trad 的联想索引。

mod common;

use common::{AssocDict, no_symbols};
use engine_core::dictionary::InMemoryDictionary;
use engine_core::{Engine, Mode, Scheme};

fn texts(e: &Engine) -> Vec<String> {
    e.candidates(8).into_iter().map(|c| c.text).collect()
}

fn find(e: &Engine, word: &str) -> usize {
    e.candidates(usize::MAX)
        .iter()
        .position(|c| c.text == word)
        .unwrap_or_else(|| panic!("候选里没有 {word}：{:?}", texts(e)))
}

fn type_str(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

#[test]
fn wubi_auto_commit_builds_association() {
    let main = AssocDict::new()
        .word("nihao", "你好", 900)
        .prefix("你好", &[("你好吗", 500)]);
    let mut wubi = InMemoryDictionary::new();
    wubi.insert("abcd", "你好", 900);
    let mut e = Engine::new(Box::new(main), no_symbols(), true);
    e.set_wubi_dict(Some(Box::new(wubi)));
    e.set_scheme(Scheme::Wubi);

    type_str(&mut e, "abcd");
    assert_eq!(e.buffer(), "", "前置：四码唯一自动上屏");
    assert!(e.association_active(), "自动上屏也要建联想");
    assert!(texts(&e).contains(&"吗".to_string()));
}

fn nine_engine() -> Engine {
    let main = AssocDict::new()
        .word("ni", "你", 900)
        .word("nihao", "你好", 900)
        .prefix("你好", &[("你好吗", 500)]);
    let mut e = Engine::new(Box::new(main), no_symbols(), true);
    e.set_scheme(Scheme::Nine);
    e
}

#[test]
fn nine_tail_kept_does_not_associate_but_full_consumption_does() {
    let mut tail = nine_engine();
    type_str(&mut tail, "6442"); // ni 后面还留 42（1–2 键总会保留）
    let idx = find(&tail, "你");
    assert_eq!(tail.select(idx), "", "留尾 = 本轮不提交");
    assert!(!tail.buffer().is_empty(), "前置：尾巴保留");
    assert!(!tail.association_active(), "缓冲非空不得联想");

    let mut full = nine_engine();
    type_str(&mut full, "64426"); // 恰好 nihao
    let idx = find(&full, "你好");
    assert_eq!(full.select(idx), "你好");
    assert_eq!(full.buffer(), "");
    assert!(full.association_active(), "整段消耗后建联想");
}

fn trad_pair() -> (AssocDict, AssocDict) {
    let main = AssocDict::new()
        .word("jintian", "今天", 900)
        .suffix("今天", &[("吃饭", 90)]);
    let trad = AssocDict::new()
        .word("jintian", "今天", 900)
        .suffix("今天", &[("回家", 90)]);
    (main, trad)
}

#[test]
fn traditional_mode_uses_trad_dictionary_for_association() {
    let (main, trad) = trad_pair();
    let mut e = Engine::with_dictionaries(Box::new(main), Some(Box::new(trad)), no_symbols(), true);
    e.switch_mode(Mode::Traditional);
    type_str(&mut e, "jintian");
    let idx = find(&e, "今天");
    assert_eq!(e.select(idx), "今天");
    assert!(e.association_active());
    assert_eq!(texts(&e), vec!["回家"], "繁体模式必须读 trad 的 assoc");
}

#[test]
fn traditional_without_trad_dict_falls_back_to_main() {
    let (main, _) = trad_pair();
    let mut e = Engine::with_dictionaries(Box::new(main), None, no_symbols(), true);
    e.switch_mode(Mode::Traditional);
    type_str(&mut e, "jintian");
    let idx = find(&e, "今天");
    assert_eq!(e.select(idx), "今天");
    assert!(e.association_active());
    assert_eq!(texts(&e), vec!["吃饭"], "trad 缺失时回退主库");
}
