// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **逐字确认 / 延迟提交**（2026-10-17 用户裁决）：
//! 「我输入一个词组，匹配到的在最前面，到没有匹配，就要先匹配前面的第一个字，匹配到
//! 所有读音的字，确认一个字，依然在候选栏；然后依次匹配后面的字，直到最后的字匹配
//! 确认后，才是最后确认到输入框。可参考同类输入法。」
//!
//! 语义（四端同一条路，引擎单点）：
//! · 整串有词组匹配 ⇒ 词组候选在最前（既有档位秩序不动），选中 = **一次提交整词**；
//! · 整串无精确匹配 ⇒ 分词回退档出**第一个字**的整档（所有读音的字；更长的前缀词
//!   不算整串匹配）；
//!   确认一个字 ⇒ 它**留在候选区**（`pending_text()` / `display_buffer()` 可见），
//!   缓冲区只剩后缀接着编 —— **不提交到输入框**；
//! · 依次确认，直到最后一次选择把后缀吃完 ⇒ **整串一次性提交**；
//! · ⌫：缓冲非空先退缓冲；缓冲空再按一次 ⇒ **撤销上一个已确认的字**（它的拼音回到缓冲）。

use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, InMemoryDictionary};

fn dict(entries: &[(&str, &str, u32)]) -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    for (py, w, f) in entries {
        d.insert(py, w, *f);
    }
    d
}

/// 只有单字、没有词组：`zhongguo` 整串无匹 ⇒ 分词回退到「zhong」档。
fn chars_only() -> Engine {
    Engine::new(
        Box::new(dict(&[
            ("zhong", "中", 900),
            ("zhong", "钟", 500),
            ("zhong", "终", 300),
            ("guo", "国", 800),
            ("guo", "果", 400),
        ])),
        SymbolEngine::new(Vec::new(), Vec::new()),
        true,
    )
}

fn typed(e: &mut Engine, keys: &str) {
    for c in keys.chars() {
        e.input_key(c);
    }
}

fn pick(e: &mut Engine, word: &str) -> String {
    let cands = e.candidates(16);
    let idx = cands
        .iter()
        .position(|c| c.text == word)
        .unwrap_or_else(|| panic!("候选里没有 {word:?}：{cands:?}"));
    e.select(idx)
}

#[test]
fn confirmed_chars_stay_in_the_candidate_area_until_the_last_one() {
    let mut e = chars_only();
    typed(&mut e, "zhongguo");

    // 无词组匹配 ⇒ 回退档 = 第一个字（zhong）的整档：所有读音为 zhong 的字都在。
    let cands = e.candidates(16);
    for w in ["中", "钟", "终"] {
        assert!(
            cands.iter().any(|c| c.text == w),
            "回退档应含「{w}」（第一个字的所有读音的字）：{cands:?}"
        );
    }

    // 确认第一个字：**不提交到输入框**，留在候选区（display_buffer 可见），缓冲只剩后缀。
    let out = pick(&mut e, "中");
    assert_eq!(out, "", "中间确认不得提交到输入框");
    assert_eq!(e.pending_text(), "中", "确认的字留在候选区");
    assert_eq!(e.buffer(), "guo", "缓冲只剩后面的字接着编");
    assert!(
        e.display_buffer().starts_with('中'),
        "显示串必须带着已确认的字：{:?}",
        e.display_buffer()
    );

    // 确认最后一个字：整串一次性提交。
    let out = pick(&mut e, "国");
    assert_eq!(out, "中国", "最后一个字确认后才整串进输入框");
    assert_eq!(e.pending_text(), "", "提交后待确认区清空");
    assert_eq!(e.buffer(), "");
}

#[test]
fn phrase_candidate_commits_immediately() {
    // 整串有词组匹配：词组在最前（多音节输入下「精确单字」档为空），选中一次提交。
    let mut e = Engine::new(
        Box::new(dict(&[
            ("nihao", "你好", 900),
            ("ni", "你", 800),
            ("hao", "好", 700),
        ])),
        SymbolEngine::new(Vec::new(), Vec::new()),
        true,
    );
    typed(&mut e, "nihao");
    let cands = e.candidates(16);
    assert_eq!(cands[0].text, "你好", "词组匹配在最前面：{cands:?}");
    assert_eq!(pick(&mut e, "你好"), "你好", "整词一次提交");
    assert_eq!(e.pending_text(), "");
}

#[test]
fn backspace_unpicks_the_last_confirmed_char() {
    let mut e = chars_only();
    typed(&mut e, "zhongguo");
    assert_eq!(pick(&mut e, "中"), "");

    // 缓冲非空时 ⌫ 先退缓冲（guo → gu → g → 空）。
    for _ in 0..3 {
        e.backspace();
    }
    assert_eq!(e.buffer(), "");
    assert_eq!(e.pending_text(), "中", "退缓冲不动已确认的字");

    // 缓冲空时再 ⌫：撤销最后一个已确认的字，它的拼音回到缓冲。
    e.backspace();
    assert_eq!(e.pending_text(), "");
    assert_eq!(e.buffer(), "zhong", "撤销后拼音回到缓冲，可重新选字");
    assert!(e.display_buffer().contains("zhong"));
}

#[test]
fn space_flushes_pending_together_with_the_rest() {
    let mut e = chars_only();
    typed(&mut e, "zhongguo");
    assert_eq!(pick(&mut e, "中"), "");
    // 空格 = 收尾：后缀的首候选（国）+ 已确认的字一起提交。
    assert_eq!(e.input_space(), "中国");
    assert_eq!(e.pending_text(), "");
    assert_eq!(e.buffer(), "");
}

#[test]
fn clear_and_mode_switch_drop_pending() {
    let mut e = chars_only();
    typed(&mut e, "zhongguo");
    assert_eq!(pick(&mut e, "中"), "");
    e.clear();
    assert_eq!(e.pending_text(), "");

    let mut e = chars_only();
    typed(&mut e, "zhongguo");
    assert_eq!(pick(&mut e, "中"), "");
    e.switch_mode(engine_core::composer::Mode::English);
    assert_eq!(
        e.pending_text(),
        "",
        "切模式清待确认区（宿主会先 commitPendingBuffer）"
    );
}

/// 2026-10-18 用户报障「逐字确认没效果」的根因回归：整串没有**精确**词，但词库里有
/// 更长的词以它为前缀（前缀扩展命中）时，回退档曾被 `merged.is_empty()` 一类「有候选
/// 就不接管」判据挡掉 —— 用户看到的是「中国人」，不是第一个字「中/钟」的整档。
/// 判据必须是「没有精确整串命中」，不是「候选池为空」。
#[test]
fn prefix_extension_does_not_suppress_per_char_fallback() {
    let mut e = Engine::new(
        Box::new(dict(&[
            ("zhongguoren", "中国人", 900),
            ("zhong", "中", 900),
            ("zhong", "钟", 500),
        ])),
        SymbolEngine::new(Vec::new(), Vec::new()),
        true,
    );
    typed(&mut e, "zhongguo");
    let cands = e.candidates(16);
    assert!(
        cands.iter().any(|c| c.text == "中") && cands.iter().any(|c| c.text == "钟"),
        "应回退到第一个字 zhong 的整档（所有读音的字）：{cands:?}"
    );
    assert_eq!(
        cands[0].text, "中",
        "第一个字的精确单字应排在更长的前缀词之前：{cands:?}"
    );
    assert_eq!(pick(&mut e, "中"), "", "中间确认不得提交到输入框");
    assert_eq!(e.pending_text(), "中", "确认的字留在候选区");
    assert_eq!(e.buffer(), "guo", "缓冲只剩后缀接着编");
}
