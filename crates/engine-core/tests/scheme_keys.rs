// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 输入方案（9 键 / 14 键 / 笔画，2026-10-09）的端到端行为门禁。
//!
//! 判据与全键盘各层同源：**按键类前缀匹配**的结果集 + 「精确加成 ≥ 任何前缀
//! 扩展」那条既有不变式在方案路径下同样成立（打分规则逐字复用）。
use engine_core::symbols::SymbolEngine;
use engine_core::{Dictionary, Engine, InMemoryDictionary, Scheme};

fn dict(entries: &[(&str, &str, u32)]) -> Box<dyn Dictionary> {
    let mut d = InMemoryDictionary::new();
    for (py, w, f) in entries {
        d.insert(py, w, *f);
    }
    Box::new(d)
}

/// 符号表用空表：本文件测的是按键类匹配，符号数据不该参与断言。
fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

fn engine() -> Engine {
    Engine::new(
        dict(&[
            ("ni", "你", 5000),
            ("ni", "尼", 900),
            ("nihao", "你好", 6000),
            ("li", "里", 4000),
            ("hao", "好", 7000),
            ("wang", "王", 3000),
        ]),
        no_symbols(),
        false,
    )
}

fn type_str(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

fn texts(e: &Engine) -> Vec<String> {
    e.candidates(8).into_iter().map(|c| c.text).collect()
}

#[test]
fn nine_key_expands_key_classes() {
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64"); // 6∈{m,n,o} 4∈{g,h,i} ⇒ ni 命中
    let got = texts(&e);
    assert!(
        got.contains(&"你".to_string()),
        "64 应命中 ni⇒你，实际 {got:?}"
    );
    assert!(
        !got.contains(&"里".to_string()),
        "li 的第一个字母 l 不在 6 的类里，不该命中：{got:?}"
    );
}

#[test]
fn nine_key_prefix_matches_longer_keys() {
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64426"); // nihao
    assert_eq!(texts(&e)[0], "你好", "64426 应命中 nihao⇒你好");
    // 前缀语义：更短的输入也应命中更长的键
    let mut e2 = engine();
    e2.set_scheme(Scheme::Nine);
    type_str(&mut e2, "64");
    assert!(
        texts(&e2).contains(&"你好".to_string()),
        "64 前缀应命中 nihao"
    );
}

#[test]
fn nine_key_digits_enter_buffer_full_scheme_ignores_them() {
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64");
    assert_eq!(e.buffer(), "64", "9 键方案下数字进缓冲");
    // 0/1 不是 9 键的输入键
    e.input_key('0');
    assert_eq!(e.buffer(), "64", "0 不进缓冲");
    // 对照组：全键盘方案下数字照旧**不进**缓冲（行为一字不动）
    let mut full = engine();
    full.input_key('6');
    assert_eq!(full.buffer(), "", "全键盘下数字不该进拼音缓冲");
}

#[test]
fn fourteen_key_expands_letter_pairs() {
    let mut e = engine();
    e.set_scheme(Scheme::Fourteen);
    type_str(&mut e, "bu"); // b∈{b,n} u∈{u,i} ⇒ ni 命中
    let got = texts(&e);
    assert!(
        got.contains(&"你".to_string()),
        "14 键 bu 应命中 ni⇒你，实际 {got:?}"
    );
    // q∈{q,w}：q 应命中 wang（自己的类里的字母）
    let mut e2 = engine();
    e2.set_scheme(Scheme::Fourteen);
    type_str(&mut e2, "q");
    assert!(
        texts(&e2).contains(&"王".to_string()),
        "14 键 q 应命中 wang⇒王（q 键类含 w）"
    );
}

#[test]
fn scheme_exact_match_outranks_prefix_even_with_lower_freq() {
    // 与全键盘同一条不变式：精确（键长相等）> 任何前缀扩展。
    // ni（5000，精确）应压过 nihao（6000，前缀）。
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64");
    assert_eq!(texts(&e)[0], "你", "精确的 ni 应排在更长的 nihao 前");
}

#[test]
fn set_scheme_clears_buffer() {
    let mut e = engine();
    type_str(&mut e, "ni");
    assert_eq!(e.buffer(), "ni");
    e.set_scheme(Scheme::Nine);
    assert_eq!(
        e.buffer(),
        "",
        "换方案必须清缓冲（旧方案的串在新方案下无意义）"
    );
}

#[test]
fn stroke_scheme_queries_stroke_dict() {
    let mut e = engine();
    // 笔画词典：键是笔画串（h 横 s 竖 p 撇 n 捺 z 折），词是单字。
    // 主词典里**故意**没有这些键，用来分辨「换了词典」与「还是主词典」。
    e.set_stroke_dict(Some(dict(&[("hs", "十", 5000), ("hsz", "土", 4000)])));
    e.set_scheme(Scheme::Stroke);
    type_str(&mut e, "hs");
    assert_eq!(
        texts(&e)[0],
        "十",
        "笔画方案应查 stroke 词典（主词典里没有 hs）"
    );
    // 前缀语义：hs 也命中 hsz
    assert!(texts(&e).contains(&"土".to_string()));
}

#[test]
fn stroke_scheme_without_dict_is_empty_not_error() {
    let mut e = engine();
    e.set_scheme(Scheme::Stroke);
    type_str(&mut e, "hs");
    assert!(
        texts(&e).is_empty(),
        "没装笔画词典 = 候选取自普通词典（此例为空），不该报错"
    );
}

#[test]
fn full_scheme_behavior_unchanged() {
    // 回归守：默认方案下一切照旧（普通前缀查询 + 数字不入缓冲）。
    let mut e = engine();
    type_str(&mut e, "ni");
    assert_eq!(texts(&e)[0], "你");
    assert_eq!(e.scheme(), Scheme::Full);
}

#[test]
fn nine_scheme_digits_enter_buffer_through_router() {
    // 路由器层（Android/Apple 走的就是它）：9 键方案下数字必须进缓冲 ——
    // **有候选时也不许**被 digit_select 当页内选词吃掉（那正是首版的门没开在
    // 选词判定之前的形态：缓冲一有候选，9 键就再也打不进第二个数字）。
    let mut r = engine_core::KeyRouter::new(engine());
    r.engine_mut().set_scheme(Scheme::Nine);
    for c in ['6', '4'] {
        r.key_event(c as u32, 0);
    }
    assert_eq!(r.buffer(), "64", "9 键方案：数字经路由器进缓冲");
    assert!(
        r.candidates().contains(&"你".to_string()),
        "并且真的出候选（此时缓冲非空、有候选可被选）"
    );
    r.key_event('7' as u32, 0);
    assert_eq!(r.buffer(), "647", "有候选时数字仍是输入键（没被选词吃掉）");
    // 0/1 不是 9 键的输入键：直通（不进缓冲、也不选词）
    r.key_event('1' as u32, 0);
    assert_eq!(r.buffer(), "647", "0/1 既不进缓冲也不选词");
}

#[test]
fn full_scheme_digits_still_select_through_router() {
    // 对照组：默认方案下数字仍是选词键（行为一字不动）。
    let mut r = engine_core::KeyRouter::new(engine());
    for c in ['n', 'i'] {
        r.key_event(c as u32, 0);
    }
    assert_eq!(r.buffer(), "ni");
    let action = r.key_event('1' as u32, 0);
    assert!(
        matches!(action, engine_core::KeyAction::Input(_)),
        "全键盘：数字选词应产出提交，实际 {action:?}"
    );
    assert_eq!(r.buffer(), "", "选词后缓冲清空");
}
