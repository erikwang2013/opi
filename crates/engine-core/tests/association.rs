// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **确认后再输入 = 全新开始**门禁（2026-10-12 **晚**用户裁决）。
//!
//! 裁决原文：「输入确认后，再输入就是新的开始，不要和之前的拼接」。
//! 起因：当天白天的「联想」版把**上一次选中的词的拼音**拼进下一次输入的候选
//! （打「你好」选掉、再打 `ma` ⇒ 首位是「你好吗」），点它会把已确认的「你好」
//! **再上屏一遍**（字段里 你好+你好吗）—— 用户报障「把前的确认内容覆盖 / 拼接」。
//!
//! 撤除后钉两条：
//! 1. 确认后接着打，候选**只按本次输入**匹配（不含上文整串命中）；
//! 2. 落进字段的文本 = 本次输入对应的候选（不重复已确认部分）。
//!
//! ⚠️ 本文件白天钉的是**相反**的语义（联想档排最前、9 键 digits 拼接）。翻面是裁决，
//! 不是回归 —— 别按白天的断言「改回去」：那条路会让用户点到重复上屏的候选。

mod common;

use common::AssocDict;
use engine_core::dictionary::InMemoryDictionary;
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Scheme};

fn engine() -> Engine {
    let mut d = InMemoryDictionary::new();
    d.insert("nihao", "你好", 900);
    d.insert("nihao", "拟好", 100);
    d.insert("nihaoma", "你好吗", 500);
    d.insert("nihaoshi", "你好时", 500);
    d.insert("ni", "你", 900);
    d.insert("ma", "吗", 800);
    d.insert("ma", "妈", 700);
    Engine::new(Box::new(d), SymbolEngine::builtin(), true)
}

fn type_str(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

fn texts(e: &Engine) -> Vec<String> {
    e.candidates(8).into_iter().map(|c| c.text).collect()
}

/// 26 键：确认后接着打 ⇒ 候选只按新输入匹配，**不含**上文整串命中。
#[test]
fn full_scheme_starts_fresh_after_commit() {
    let mut e = engine();
    type_str(&mut e, "nihao");
    assert_eq!(texts(&e)[0], "你好", "前置：精确命中的词在首位");
    assert_eq!(e.select(0), "你好");

    type_str(&mut e, "ma");
    let got = texts(&e);
    assert_eq!(got[0], "吗", "新输入按自己的排名出候选（{got:?}）");
    assert!(
        !got.contains(&"你好吗".to_string()),
        "不得出现含上文的整串命中（点了会重复上屏已确认部分）：{got:?}"
    );
    // 落进字段的只有新的那个字 —— 「新的开始，不拼接」。
    assert_eq!(e.select(0), "吗");
}

/// 9 键：同一语义（缓冲是键位，也不做 `digits(上文)+缓冲` 的拼接）。
#[test]
fn nine_scheme_starts_fresh_after_commit() {
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64426"); // nihao
    assert_eq!(texts(&e)[0], "你好");
    assert_eq!(e.select(0), "你好");
    assert_eq!(e.buffer(), "", "前置：提交后缓冲清空");

    type_str(&mut e, "62"); // ma（6=mno 2=abc）
    let got = texts(&e);
    assert!(
        !got.contains(&"你好吗".to_string()),
        "9 键同样不拼接上文：{got:?}"
    );
    assert_eq!(e.select(0), "吗", "上屏的只有新输入对应的字");
}

fn assoc_engine() -> Engine {
    let d = AssocDict::new()
        .word("nihao", "你好", 900)
        .word("nihaoma", "你好吗", 500)
        .word("ma", "吗", 800)
        .word("ma", "妈", 700)
        .prefix("你好", &[("你好吗", 500)])
        .suffix("你好", &[("吗", 0)]);
    Engine::new(Box::new(d), SymbolEngine::builtin(), true)
}

/// 有索引/assoc 的真联想源也必须守住「确认后 = 全新开始」：联想只活在空缓冲，
/// 任何文本输入都从零开始（Spec §4.6 契约，正控见上面三条的夹具是 InMemoryDictionary）。
#[test]
fn association_does_not_leak_into_the_next_input() {
    let mut e = assoc_engine();
    type_str(&mut e, "nihao");
    assert_eq!(e.select(0), "你好");
    assert!(e.association_active(), "前置：提交后进入联想态");
    assert!(texts(&e).contains(&"吗".to_string()), "前置：联想候选可见");

    type_str(&mut e, "ma");
    let got = texts(&e);
    assert!(
        !got.contains(&"你好吗".to_string()),
        "联想不得拼进下文：{got:?}"
    );
    assert_eq!(e.select(0), "吗");
}

/// 确认**不改变**下一次输入的行为：无论中间隔多少动作，第二次输入都与「空引擎」
/// 打同一串的结果一致（这正是「新的开始」的可判定形式）。
#[test]
fn second_input_matches_a_cold_engine() {
    let mut warm = engine();
    type_str(&mut warm, "nihao");
    let _ = warm.select(0);
    // 中间再做几次无关动作（退格到空、再打）
    type_str(&mut warm, "x");
    warm.backspace();
    type_str(&mut warm, "ma");
    let warm_got = texts(&warm);

    let mut cold = engine();
    type_str(&mut cold, "ma");
    let cold_got = texts(&cold);

    assert_eq!(
        warm_got, cold_got,
        "确认过一轮之后的候选必须与冷引擎逐条相同（上文不参与）"
    );
}
