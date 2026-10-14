// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **候选三档 · 真库组**（2026-10-14 用户裁决；夹具组在 `engine-core/tests/candidate_tiers.rs`）。
//!
//! 为什么真库组落在 `engine-data` 而不是 `engine-core`：读 `.opid` 文件的能力在
//! `engine-data`（`engine-core` **不许有 IO**，也不依赖它 —— 依赖方向反着），
//! 而本组测的正是「真库数据下这三档长什么样」—— 夹具测不出生僻字海（那正是这次
//! 报障的成因：`ni` 一族在真库里有 580 个单字、1195 个词）。引擎侧的档位规则与
//! 全部理由见 `crates/engine-core/src/candidates_tiers.rs` 头注。
//!
//! 真库是**入库资产**（`android/app/src/main/assets/luna.opid`）—— 缺文件直接红，
//! 不静默跳过。

use engine_core::Engine;
use engine_core::composer::Mode;
use engine_core::symbols::SymbolEngine;

fn type_str(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

fn texts(e: &Engine, limit: usize) -> Vec<String> {
    e.candidates(limit).into_iter().map(|c| c.text).collect()
}

/// 真库（luna.opid）：用户报障的那条输入 —— `ni`。
///
/// 改动前的实测：前 40 位全是单字（`你 泥 拟 呢 腻 祢 铌 鲵 𢘝 …`），
/// 「你好 / 你们 / 你的」**永远出不来**（精确加成把几百个精确单字全压在词组之上）。
#[test]
fn real_dict_ni_letters_words_and_associations_are_all_reachable() {
    let mut e = real_engine();
    type_str(&mut e, "ni");
    let got = texts(&e, 200);
    let pos = |w: &str| {
        got.iter()
            .position(|t| t == w)
            .unwrap_or_else(|| panic!("候选里没有 {w}（前 200 位）：{:?}", &got[..20]))
    };

    // ① 常用单字档：前 5 位全是单字，首位是 你。
    assert_eq!(got[0], "你");
    for t in &got[..5] {
        assert_eq!(t.chars().count(), 1, "1 档应是单字：{t:?}");
    }
    // ② 联想档按拼音长度升序，同级先词后字（实测三条锚）。
    assert!(pos("您") < pos("年"), "3 字母（您）应在 4 字母（年）之前");
    assert!(
        pos("年") < pos("你好"),
        "4 字母（年）应在 5 字母（你好）之前"
    );
    assert!(
        pos("你哦") < pos("您"),
        "同为 3 字母时，词（你哦）应排在字（您）之前"
    );
    // ③ 生僻精确单字沉底：它们必须排在常用词组之后。
    assert!(
        pos("你好") < pos("祢"),
        "生僻精确单字（祢）应沉在联想档之后"
    );
    // ④ 可达性：用户报障的那几个词要真进得了候选栏（页 8 槽 ⇒ 这是前 5 页内）。
    assert!(pos("你好") < 40, "你好 在下标 {}，太深", pos("你好"));
    assert!(pos("你们") < 40);
}

/// 真库：单音节另一条最常见的输入 `wo`（词组一族 `我们 / 我的 / 我国`）。
#[test]
fn real_dict_wo_puts_common_words_in_reach() {
    let mut e = real_engine();
    type_str(&mut e, "wo");
    let got = texts(&e, 40);
    let pos = |w: &str| {
        got.iter()
            .position(|t| t == w)
            .unwrap_or_else(|| panic!("候选里没有 {w}（前 40 位）：{got:?}"))
    };
    assert_eq!(got[0], "我");
    for t in &got[..5] {
        assert_eq!(t.chars().count(), 1, "1 档应是单字：{t:?}");
    }
    assert!(pos("我们") < 20, "我们 在下标 {}", pos("我们"));
    assert!(pos("我的") < 20);
}

/// 真库：多音节输入**行为不变**（它们本来就以词为主，三档不该把它们改坏）。
#[test]
fn real_dict_multi_syllable_still_leads_with_the_word() {
    let mut e = real_engine();
    type_str(&mut e, "nihao");
    let got = texts(&e, 8);
    assert_eq!(got[0], "你好", "{got:?}");
    assert_eq!(got[1], "拟好", "精确同音词仍在第二位（按分）：{got:?}");
}

/// 真库引擎（入库资产 `android/app/src/main/assets/luna.opid`）。
/// 缺文件**直接红**：本文件后半段的价值全在真库数据上，静默跳过等于没有门禁。
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
