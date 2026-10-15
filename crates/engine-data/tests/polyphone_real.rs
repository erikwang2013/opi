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

/// 用户那一单的**现状**（2026-10-15 落位规则改为「按该读音自己的语料份额」之后实测）：
/// `mo` 的 `么` **可达**（精确字组尾部，实测第 215 位）—— 它不再是「一个字不出」，
/// 但要翻页；`me` 的 `么` 仍是第 0 位（见下条）。
///
/// ⚠️ 为什么不在首屏：`么/mó`（幺麽）是**冷读音**，kHanyuPinlu 里没有它的语料证据 ⇒
/// 份额 0 ⇒ 按份额制沉到精确字组尾部。同一条规则修掉了反向的病：
/// `儿/ní`（同样冷）原先凭「儿 的整字常用度」排到 `ni` 候选第 2、压住 `你` —— 见下一条门禁。
#[test]
fn mo_yields_me_reachable_but_deep() {
    let mut e = real_engine();
    type_str(&mut e, "mo");
    let got = texts(&e, 400);
    let pos = got.iter().position(|t| t == "么");
    assert!(
        pos.is_some_and(|p| p < 300),
        "么 应在精确字组内可达（实测 215）：{:?}",
        pos
    );
    assert_eq!(got[0], "模", "首位仍是词库里词频最高的 模：{:?}", &got[..4]);
}

/// **冷读音不许凭整字常用度抢位**（2026-10-15 实测修掉的回归）：`儿/ní` 是 `儿(ér)` 的
/// 罕读，按「该字最大词频」落位时它排到 `ni` 的第 2 位、压住 `你`。份额制下它沉到精确
/// 字组尾部（实测 180/182）。本门禁钉住这个**方向**（不在前 100），不钉具体名次。
#[test]
fn rare_reading_does_not_outrank_the_common_char() {
    let mut e = real_engine();
    type_str(&mut e, "ni");
    let got = texts(&e, 200);
    assert_eq!(got[0], "你", "首位必须是 你");
    if let Some(p) = got.iter().position(|t| t == "儿") {
        assert!(p > 100, "儿/ní 是冷读音，不该排进前 100（实测 180）：{p}");
    }
}

/// 9 键（手机默认）：`66` 同样是 `mo`，同样可达（门禁只要求「翻页内可达」）。
#[test]
fn nine_key_66_also_reaches_me() {
    let mut e = real_engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "66");
    let got = texts(&e, 400);
    assert!(
        got.iter().any(|t| t == "么"),
        "9 键 66 应可达 么（前 400 位内）"
    );
}

/// 同类缺口抽查：`绿` 的 `lu`（绿林）、`咯` 的 `ka`（咯血）—— 同样是「可达但要翻页」。
#[test]
fn other_known_gaps_are_reachable_too() {
    for (input, want) in [("lu", "绿"), ("ka", "咯")] {
        let mut e = real_engine();
        type_str(&mut e, input);
        let got = texts(&e, 400);
        assert!(
            got.iter().any(|t| t == want),
            "打 {input} 应可达 {want}（前 400 位内）"
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
