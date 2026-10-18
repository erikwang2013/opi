// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **输入即得表情 · 真库组**（2026-10-18 用户需求「增加输入可匹配表情」）。
//!
//! 引擎行为门禁在 `crates/engine-core/tests/emoji_match.rs`（夹具词库）；本文件只钉
//! 真 luna 上的两件事：
//! 1. **新数据层真的在跑** —— `huojian` 在 `symbols.tsv` 里没有拼音关键字
//!    （那里 🚀 只有 `rocket`），能出 🚀 只能是 `emoji_keywords.tsv` 这条新路；
//! 2. **常用词首位不被顶掉** —— 真库 `nihao` 的首候选仍是「你好」，👋 只在后面。
//!
//! 真库是入库资产（`android/app/src/main/assets/luna.opid`）—— 缺文件直接红。

use engine_core::candidates::CandidateKind;
use engine_core::composer::Mode;
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Scheme};

fn real_engine() -> Engine {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../android/app/src/main/assets/luna.opid");
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "读不到真库 {}（{e}）—— 它是入库资产，不该缺",
            path.display()
        )
    });
    let dict = engine_data::load_bytes(bytes).expect("luna.opid 解析");
    let mut e = Engine::new(Box::new(dict), SymbolEngine::builtin(), true);
    e.switch_mode(Mode::Pinyin);
    e
}

fn type_str(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

fn texts(e: &Engine) -> Vec<String> {
    e.candidates(usize::MAX)
        .into_iter()
        .map(|c| c.text)
        .collect()
}

#[test]
fn real_dict_pinyin_aliases_reach_emoji() {
    let mut e = real_engine();

    type_str(&mut e, "weixiao");
    assert!(texts(&e).iter().any(|t| t == "😊"), "weixiao → 😊");

    e.clear();
    type_str(&mut e, "huojian");
    assert!(
        texts(&e).iter().any(|t| t == "🚀"),
        "huojian → 🚀（symbols.tsv 没有这条拼音，只有新表能给出）"
    );

    e.clear();
    type_str(&mut e, "ku");
    assert!(texts(&e).iter().any(|t| t == "😭"), "ku → 😭");
}

#[test]
fn real_dict_common_word_keeps_first_slot() {
    let mut e = real_engine();
    type_str(&mut e, "nihao");
    let got = e.candidates(usize::MAX);
    assert_eq!(got[0].text, "你好", "emoji 不得顶掉真库的文本首候选");
    assert!(
        got.iter()
            .any(|c| c.kind == CandidateKind::Emoji && c.text == "👋"),
        "nihao 应能匹配到 👋"
    );
    // 9/14 键不覆盖（本轮边界）：数字串不是关键字前缀，不该凭空出 emoji。
    e.clear();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64426");
    assert!(
        !e.candidates(usize::MAX)
            .iter()
            .any(|c| c.kind == CandidateKind::Emoji)
    );
}
