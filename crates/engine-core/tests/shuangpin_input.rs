// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 双拼**端到端**门禁（2026-10-13 用户裁决「主流同款全套」）：经 `Engine::candidates`
//! 走完整链路（键序 → 解码 → 标准全拼管线 → 分档），不是只测解码函数。
//!
//! 每套布局打**那套自己的键**（键位表见 `shuangpin_tables.rs`，抽查的值与 libime
//! 的 C 表逐条对应）；同一断言在六套上跑，任何一套表错都会在本文件露头。

use engine_core::dictionary::InMemoryDictionary;
use engine_core::scheme::Scheme;
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Mode};

fn dict() -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    for (py, w, f) in [
        ("ni", "你", 3_999_128_899u32),
        ("hao", "好", 3_997_635_583),
        ("nihao", "你好", 47_594),
        ("zhong", "中", 3_990_000_000),
        ("guo", "国", 3_988_000_000),
        ("zhongguo", "中国", 3_996_000_000),
        ("an", "安", 3_995_000_000),
        ("dong", "东", 3_994_000_000),
    ] {
        d.insert(py, w, f);
    }
    d
}

fn engine(layout: usize) -> Engine {
    let mut e = Engine::new(Box::new(dict()), SymbolEngine::builtin(), false);
    e.set_scheme(Scheme::Shuangpin);
    e.set_shuangpin_layout(layout);
    e
}

fn type_and_cands(e: &mut Engine, keys: &str, limit: usize) -> Vec<String> {
    e.clear();
    for c in keys.chars() {
        e.input_key(c);
    }
    e.candidates(limit).into_iter().map(|c| c.text).collect()
}

/// 六套 nihao 的键序（各自 ao 的键不同；ni 都是 n+i）。
const NIHAO_KEYS: [(&str, usize); 6] = [
    ("nihk", 0), // 自然码
    ("nihk", 1), // 微软
    ("nihc", 2), // 小鹤（ao→c）
    ("nihk", 3), // 智能ABC
    ("nihq", 4), // 紫光（ao→q）
    ("nihd", 5), // 拼音加加（ao→d）
];

#[test]
fn every_layout_types_nihao_end_to_end() {
    for (keys, layout) in NIHAO_KEYS {
        let mut e = engine(layout);
        let got = type_and_cands(&mut e, keys, 9);
        assert!(
            got.contains(&"你好".to_string()),
            "布局 {layout}（{keys}）该出「你好」，实得 {got:?}"
        );
    }
}

/// zh/ch/sh 三套键位各异：zhong 在自然码/微软/小鹤 = vs / vs / vs?（都是 v+s），
/// 智能ABC = a+s、紫光 = u+s、拼音加加 = u+s？—— 逐套用生成表里的 zh 键拼。
#[test]
fn zh_initial_keys_per_layout() {
    // (layout, zhong 的键, guo 的键)：guo = g + uo 键。
    for (layout, zhong_keys) in [
        (0usize, "vs"), // 自然码 zh=v, ong=s
        (1, "vs"),      // 微软
        (2, "vs"),      // 小鹤
        (3, "as"),      // 智能ABC zh=a（ch=e, sh=v）
        (4, "uh"),      // 紫光 zh=u、ong=h（ch=a, sh=i）
        (5, "vy"),      // 拼音加加 zh=v、ong=y（ch=u, sh=i）
    ] {
        let mut e = engine(layout);
        let got = type_and_cands(&mut e, zhong_keys, 9);
        assert!(
            got.contains(&"中".to_string()),
            "布局 {layout}（{zhong_keys}）该出「中」，实得 {got:?}"
        );
    }
}

/// 零声母：`an` —— 自然码/拼音加加 `oj` 或裸 `j` 都行、微软/紫光/ABC 只认 `oj`、
/// 小鹤只认裸 `j`（`*`）。经候选层验证（不是解码函数的单测）。
#[test]
fn zero_initial_end_to_end() {
    // (layout, keys)：每套至少给一条合法写法。
    for (layout, keys) in [
        (0usize, "oj"),
        (1, "oj"),
        (2, "j"),
        (3, "oj"),
        (4, "or"),
        (5, "of"),
    ] {
        let mut e = engine(layout);
        let got = type_and_cands(&mut e, keys, 9);
        assert!(
            got.contains(&"安".to_string()),
            "布局 {layout}（{keys}）该出「安」，实得 {got:?}"
        );
    }
}

/// 展示读数：键序 → 解码后的**分词拼音**（与九键同一条读数纪律）。
#[test]
fn display_shows_decoded_pinyin() {
    let mut e = engine(0);
    for c in "nihk".chars() {
        e.input_key(c);
    }
    let _ = e.candidates(9);
    assert_eq!(
        e.display_buffer(),
        "ni'hao",
        "自然码 nihk 的读数该是 ni'hao"
    );
}

/// 精确档语义：解码命中真拼音 ⇒ 照吃精确加成（打整词 nihk 时「你好」在两字档之上）。
#[test]
fn decoded_exact_word_ranks_first() {
    let mut e = engine(0);
    let got = type_and_cands(&mut e, "nihk", 9);
    assert_eq!(
        got.first().map(String::as_str),
        Some("你好"),
        "实得 {got:?}"
    );
}

/// 繁体模式同样可打（模式管词典、方案管解码，两轴正交）。
#[test]
fn works_in_traditional_mode() {
    let mut e = engine(0);
    e.switch_mode(Mode::Traditional);
    let got = type_and_cands(&mut e, "nihk", 9);
    assert!(got.contains(&"你好".to_string()), "实得 {got:?}");
}
