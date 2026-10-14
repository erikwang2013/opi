// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **多音字补读音 · 真库组**（2026-10-15 用户报障「为何单独打『么』候选词没有？」）。
//!
//! 机制与语义边界见 `engine-core/src/polyphone.rs` 头注；夹具组的力学门禁在
//! `crates/engine-core/tests/polyphone.rs`。本文件测的是**真库落位**（词频是词库量纲，
//! 只有真库才看得出来）与**用户那一单的复现**。
//!
//! 真库是入库资产（`android/app/src/main/assets/luna.opid`）—— 缺文件直接红。

use engine_core::composer::Mode;
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Scheme};

fn real_engine() -> Engine {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../android/app/src/main/assets/luna.opid");
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "读不到真库 {}（{e}）—— 它是入库资产，不该缺；缺了这条门禁就没在测东西",
            path.display()
        )
    });
    let dict = engine_data::load_bytes(bytes).expect("luna.opid 解析");
    let mut e = Engine::new(Box::new(dict), SymbolEngine::builtin(), true);
    e.switch_mode(Mode::Pinyin);
    e
}

fn texts(e: &Engine, limit: usize) -> Vec<String> {
    e.candidates(limit).into_iter().map(|c| c.text).collect()
}

fn type_str(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

fn pos(got: &[String], w: &str) -> Option<usize> {
    got.iter().position(|t| t == w)
}

/// 用户那一单：打 `mo` 要出 `么`（改前它一个字不出 —— 词库里 `么` 只挂了 `ma/me/yao`）。
#[test]
fn mo_yields_me_in_the_first_page() {
    let mut e = real_engine();
    type_str(&mut e, "mo");
    let got = texts(&e, 8);
    assert!(
        pos(&got, "么").is_some(),
        "打 mo 应出 么（第 0 页）：{got:?}"
    );
}

/// 9 键（手机默认）：`66` 同样是 `mo`，也要出 `么`。
#[test]
fn nine_key_66_also_yields_me() {
    let mut e = real_engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "66");
    let got = texts(&e, 8);
    assert!(
        pos(&got, "么").is_some(),
        "9 键 66 应出 么（第 0 页）：{got:?}"
    );
}

/// 同类缺口抽查：`绿` 的 `lu`（绿林）、`咯` 的 `ka`（咯血）。
#[test]
fn other_known_gaps_are_filled_too() {
    for (input, want) in [("lu", "绿"), ("ka", "咯")] {
        let mut e = real_engine();
        type_str(&mut e, input);
        let got = texts(&e, 8);
        assert!(
            pos(&got, want).is_some(),
            "打 {input} 应出 {want}（第 0 页）：{got:?}"
        );
    }
}

/// **过滤规则**的边界（2026-10-15 实测踩到过）：来源只收现代词典口径 + 变体孪生字 ——
/// `百(mò)`、`貌(mò)` 这类只有古读的**不得**被补进来（否则打 `mo` 会给一串不相干的字）。
#[test]
fn ancient_only_readings_are_not_patched_in() {
    let mut e = real_engine();
    type_str(&mut e, "mo");
    let got = texts(&e, 20);
    for bad in ["百", "貌", "伯"] {
        assert!(
            pos(&got, bad).is_none(),
            "`{bad}` 的 mo 只有古读（全量历史读音表口径）—— 不该补进来：{got:?}"
        );
    }
}

/// 非回归：`me`（`么` 的常打读音）一字不动 —— 补读音层只补**词库缺的**，
/// 已有的读音不该被它改序或去重掉。
#[test]
fn known_readings_are_untouched() {
    let mut e = real_engine();
    type_str(&mut e, "me");
    let got = texts(&e, 8);
    assert_eq!(got[0], "么", "me 的首位仍是 么：{got:?}");
}
