// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **真库（luna.opid）多字组合验收**（2026-10-23）。
//!
//! 两个用户报障串的整词在资产里**不存在**（`厌蠢症` / `样样通` 不是键），但组成段
//! 都在：`yanchun → 厌蠢`、`zheng → 症`、`yangyang → 样样`、`tong → 通`。本门禁钉住
//! 「引擎候选里真能选到它们」，并打印名次与逐键耗时（`--nocapture` 可见）。
//!
//! 资产取 android 部署副本（`data/generated/luna.opid` 是 gitignore 的构建中间物）。

use engine_core::Engine;
use engine_core::symbols::SymbolEngine;
use engine_data::{Dictionary, load_mmap};
use std::path::Path;
use std::time::Instant;

const LUNA_OPID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);

fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

fn engine() -> Engine {
    let dict =
        load_mmap(Path::new(LUNA_OPID)).unwrap_or_else(|e| panic!("加载 {LUNA_OPID} 失败：{e:?}"));
    Engine::new(Box::new(dict), no_symbols(), false)
}

fn type_str(engine: &mut Engine, input: &str) {
    engine.clear();
    for ch in input.chars() {
        engine.input_key(ch);
    }
}

fn candidates_of(engine: &mut Engine, input: &str) -> Vec<String> {
    type_str(engine, input);
    engine
        .candidates(usize::MAX)
        .into_iter()
        .map(|c| c.text)
        .collect()
}

/// 组成段必须真在资产里（缺哪段，报障串就永远拼不出来 —— 这正是要如实上报的边界）。
#[test]
fn composition_chunks_exist_in_luna() {
    let dict =
        load_mmap(Path::new(LUNA_OPID)).unwrap_or_else(|e| panic!("加载 {LUNA_OPID} 失败：{e:?}"));
    for (key, word) in [
        ("yanchun", "厌蠢"),
        ("zheng", "症"),
        ("yangyang", "样样"),
        ("tong", "通"),
        ("chunzheng", "纯正"),
        ("yangtong", "杨同"),
    ] {
        let entries = dict.query_prefix_top(key, 0);
        assert!(
            entries
                .iter()
                .any(|e| e.pinyin_len == key.len() && e.word == word),
            "luna 缺少组成段 {key} → {word}"
        );
    }
}

#[test]
fn user_reported_words_are_reachable() {
    let mut engine = engine();
    for (input, want) in [("yanchunzheng", "厌蠢症"), ("yangyangtong", "样样通")] {
        let got = candidates_of(&mut engine, input);
        let rank = got
            .iter()
            .position(|t| t == want)
            .map(|i| i + 1)
            .unwrap_or(0);
        eprintln!(
            "组合真库：{input} → {want} 第 {rank} 位（共 {} 条）",
            got.len()
        );
        assert!(
            rank > 0,
            "{input} 的候选里没有 {want}：{:?}",
            &got[..got.len().min(20)]
        );
        // 用户点名的新词由 `composition::PINNED` 直接置顶；移动端每页 8 条，
        // 必须落在第一页（2026-10-23：纯算法组合实测第 10/11 位）。
        assert!(rank <= 8, "{want} 排到第 {rank} 位，移动端第一页看不到");
    }
}

/// 半截音节 / 垃圾串不得靠组合层出候选（逃生口同既有语义）。
#[test]
fn incomplete_and_garbage_inputs_do_not_compose() {
    let mut engine = engine();
    for input in ["yanchunzhe", "yanchunzhen", "yangyangton", "nihaob", "qqq"] {
        let got = candidates_of(&mut engine, input);
        assert!(
            !got.iter().any(|t| t == "厌蠢症" || t == "样样通"),
            "{input} 不该组合出整词：{:?}",
            &got[..got.len().min(20)]
        );
    }
}

/// 逐键耗时（报告用；不断言具体毫秒 —— 共享机器上会抖，只打印）。用 release 跑：
/// `cargo test -p opi-tools --release --test composition_real -- --nocapture`。
#[test]
fn composition_latency_of_examples() {
    let mut engine = engine();
    for input in ["yanchunzheng", "yangyangtong", "nihaob"] {
        type_str(&mut engine, input);
        let _ = engine.candidates(8); // 预热
        let mut samples = Vec::new();
        for _ in 0..50 {
            let t = Instant::now();
            let _ = engine.candidates(8);
            samples.push(t.elapsed());
        }
        samples.sort();
        eprintln!(
            "组合耗时 {input}（limit 8）：中位 {:.3}ms，最慢 {:.3}ms",
            samples[samples.len() / 2].as_secs_f64() * 1000.0,
            samples[samples.len() - 1].as_secs_f64() * 1000.0
        );
    }
}
