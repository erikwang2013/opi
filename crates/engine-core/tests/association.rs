// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **联想（前后文连续输入）**门禁（用户裁决 2026-10-12「选词后接着打要能接着匹配」）：
//! 提交一个词之后，接着打的候选里，`上文拼音 + 当前输入` 的整串命中排在最前 ——
//! 打「你好」选掉、再打 `ma`，要有「你好吗」（键 `nihaoma`，没有上文时永远轮不到它）。
//!
//! 两条路各钉一次：26 键（字母拼接）与 9 键（按键类拼接 `digits(上文)+缓冲`）。
//! 上下文语义（替换而非累计、各清空点）见 `Engine::assoc_pinyin` 的字段注。

use engine_core::dictionary::InMemoryDictionary;
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Scheme};

fn engine() -> Engine {
    let mut d = InMemoryDictionary::new();
    // 词频：把「你好吗」的整串键造出来（0 频也行 —— 联想不看分数也进档）
    d.insert("nihao", "你好", 900);
    d.insert("nihao", "拟好", 100);
    d.insert("nihaoma", "你好吗", 500);
    d.insert("nihaoshi", "你好时", 500);
    d.insert("nihaomashi", "你好吗时", 500);
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

/// 26 键：选词后接着打 ⇒ 整串命中排最前。
#[test]
fn full_scheme_associates_with_the_last_committed_word() {
    let mut e = engine();
    type_str(&mut e, "nihao");
    assert_eq!(texts(&e)[0], "你好", "前置：精确命中的词在首位");
    assert_eq!(e.select(0), "你好");

    type_str(&mut e, "ma");
    let got = texts(&e);
    assert_eq!(
        got[0], "你好吗",
        "上文「你好」+ 输入 ma ⇒ 整串命中排最前（{got:?}）"
    );
    assert!(
        got.contains(&"吗".to_string()),
        "单字候选仍在其后（不被挤掉）"
    );

    // 「替换而非累计」：再选一次「你好吗」之后，上文换成**它自己**（不是「你好」）
    // —— 夹具里 `nihaomashi`→你好吗时 与 `nihaoshi`→你好时 都在：拼接的是前者才说明
    // 上文是「你好吗」；若还停在「你好」，出来的会是「你好时」。
    assert_eq!(e.select(0), "你好吗");
    type_str(&mut e, "shi");
    let got = texts(&e);
    assert_eq!(got[0], "你好吗时", "上文已替换为「你好吗」（{got:?}）");
    assert!(
        !got.contains(&"你好时".to_string()),
        "旧上文不许残留：{got:?}"
    );
}

/// 9 键：缓冲是键位 ⇒ 联想走 `digits(上文) + 缓冲` 的按键类拼接（读数取的是
/// **选中候选的读音**，不是数字串）。
#[test]
fn nine_scheme_associates_through_key_classes() {
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64426"); // nihao
    assert_eq!(texts(&e)[0], "你好");
    assert_eq!(e.select(0), "你好");
    assert_eq!(e.buffer(), "", "前置：提交后缓冲清空");

    type_str(&mut e, "62"); // ma（6=mno 2=abc）
    let got = texts(&e);
    assert_eq!(
        got[0], "你好吗",
        "9 键：digits(上文)+缓冲 的整串命中排最前（{got:?}）"
    );
}

/// 清空点：切模式 / 换方案 / 清缓冲 / 「打英文」逃生口 —— 上文作废，联想不再发生。
#[test]
fn context_dies_at_the_documented_points() {
    // 切模式
    let mut e = engine();
    type_str(&mut e, "nihao");
    let _ = e.select(0);
    e.switch_mode(engine_core::Mode::English);
    e.switch_mode(engine_core::Mode::Pinyin);
    type_str(&mut e, "ma");
    assert_eq!(texts(&e)[0], "吗", "切模式后无联想（首位是单字）");

    // 换方案
    let mut e = engine();
    type_str(&mut e, "nihao");
    let _ = e.select(0);
    e.set_scheme(Scheme::Nine);
    e.set_scheme(Scheme::Full);
    type_str(&mut e, "ma");
    assert_eq!(texts(&e)[0], "吗", "换方案后无联想");

    // 清缓冲
    let mut e = engine();
    type_str(&mut e, "nihao");
    let _ = e.select(0);
    e.clear();
    type_str(&mut e, "ma");
    assert_eq!(texts(&e)[0], "吗", "清缓冲后无联想");

    // 逃生口：无候选时空格提交原文（「打英文」）⇒ 上文作废
    let mut e = engine();
    type_str(&mut e, "nihao");
    let _ = e.select(0);
    type_str(&mut e, "zxcvb"); // 文档里的死串（非完整音节、简拼也救不活）
    assert!(
        texts(&e).is_empty(),
        "前置：zxcvb 无候选（实得 {:?}）",
        texts(&e)
    );
    assert_eq!(e.input_space(), "zxcvb", "逃生口：空格提交原文");
    type_str(&mut e, "ma");
    assert_eq!(texts(&e)[0], "吗", "逃生口提交后无联想");
}

/// 上限与去重：联想档最多 [engine_core::candidates::ASSOC_TOP_N] 条，且不与主档重复。
#[test]
fn association_is_capped_and_deduped() {
    let mut e = engine();
    type_str(&mut e, "nihao");
    let _ = e.select(0);
    type_str(&mut e, "ma");
    let got = texts(&e);
    let assoc_like = got.iter().filter(|t| t.starts_with("你好")).count();
    assert!(
        assoc_like <= 2,
        "联想档最多 2 条（实得 {assoc_like}：{got:?}）"
    );
    let uniq: std::collections::HashSet<&String> = got.iter().collect();
    assert_eq!(uniq.len(), got.len(), "候选不得重复：{got:?}");
}
