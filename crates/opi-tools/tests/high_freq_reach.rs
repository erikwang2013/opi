// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 高频词库并入**真实 luna** 的语义门禁（2026-11-08 用户裁决「先使用高频词库再
//! 结合其他词库一起识别，注意去重」）。
//!
//! 与 `surnames_slang.rs` 同一条路：走 `Engine::candidates`（rank_and_pick），
//! 不走 `dict.query` —— 导入层的抢位、跨层去重、截断只在候选路径可见。
//!
//! 钉三件事：
//!   1. 词库里的方言/网络条目在真实 luna 之上可达（不是只在自建夹具里）；
//!   2. 同键竞争时高频词库条目排在普通内置词之前（巴适/咋整 首位）；
//!   3. 同一词跨层只出现一次（去重）。

use engine_core::Engine;
use engine_core::symbols::SymbolEngine;
use engine_data::load_mmap;
use std::path::Path;

const LUNA_OPID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);
const HIGH_FREQ: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/raw/high_freq.tsv");

fn engine() -> Engine {
    let dict = load_mmap(Path::new(LUNA_OPID)).unwrap_or_else(|e| panic!("加载 luna 失败：{e:?}"));
    let mut e = Engine::new(
        Box::new(dict),
        SymbolEngine::new(Vec::new(), Vec::new()),
        false,
    );
    let (hf, bad) = engine_data::load_extra_dict(Path::new(HIGH_FREQ))
        .unwrap_or_else(|e| panic!("加载 high_freq 失败：{e}"));
    assert_eq!(bad, 0, "high_freq.tsv 不应有坏行");
    e.set_extra_dicts(vec![("high_freq".to_string(), hf)]);
    // 预算门禁是时间量；本文件只断词典层语义，别被并发构建读成假红。
    e.set_import_scan_budget(None);
    e
}

fn candidates_of(engine: &mut Engine, pinyin: &str, limit: usize) -> Vec<String> {
    engine.clear();
    for ch in pinyin.chars() {
        engine.input_key(ch);
    }
    engine
        .candidates(limit)
        .into_iter()
        .map(|c| c.text)
        .collect()
}

#[test]
fn curated_entries_are_reachable_on_real_luna() {
    let mut e = engine();
    for (py, want) in [
        ("bashi", "巴适"),
        ("zazheng", "咋整"),
        ("yyds", "yyds"),
        ("chiguaqunzhong", "吃瓜群众"),
        ("nihao", "你好"),
    ] {
        let got = candidates_of(&mut e, py, 24);
        assert!(
            got.iter().any(|c| c == want),
            "{py} 应含「{want}」，实际 {got:?}"
        );
    }
}

#[test]
fn high_freq_layer_wins_same_key_and_dedupes() {
    let mut e = engine();
    // 方言/网络词在真实 luna 的同键竞争里拿到首位（导入层抢位）。
    assert_eq!(candidates_of(&mut e, "bashi", 8)[0], "巴适");
    assert_eq!(candidates_of(&mut e, "zazheng", 8)[0], "咋整");
    // 跨层同词只出现一次（合并取 max，不是叠成两条）。
    let got = candidates_of(&mut e, "nihao", 64);
    assert_eq!(
        got.iter().filter(|c| *c == "你好").count(),
        1,
        "跨层同词必须去重：{got:?}"
    );
}
