// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 联想构建的实测基准（2026-10-27，Spec §9.6：**只记录，不设门禁**）。
//!
//! `build_association` 是 `pub(crate)`，集成测试调不到；这里走公开路径：
//! `select`（内部含 `build_association`）+ `candidates(8)`（显示路径）。
//! 测到的是**上界**（含提交与整表排序），不是裸构建耗时。
//!
//! 跑法：`cargo test --release -p opi-tools --test association_perf -- --ignored --nocapture`

use engine_core::Engine;
use engine_core::symbols::SymbolEngine;

const LUNA: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);

#[test]
#[ignore = "真实 luna 33MB；cargo test --release -p opi-tools --test association_perf -- --ignored --nocapture"]
fn association_latency_on_real_luna() {
    let bytes = std::fs::read(LUNA).unwrap();
    let dict = engine_data::load_bytes(bytes).unwrap();
    let mut e = Engine::new(Box::new(dict), SymbolEngine::builtin(), false);
    for (root, pinyin) in [
        ("今天", "jintian"),
        ("什么", "shenme"),
        ("中国", "zhongguo"),
    ] {
        for ch in pinyin.chars() {
            let _ = e.input_key(ch);
        }
        let idx = e
            .candidates(512)
            .iter()
            .position(|c| c.text == root)
            .unwrap();
        let t0 = std::time::Instant::now();
        let out = e.select(idx); // select 内部含 build_association
        let select_time = t0.elapsed();
        let t1 = std::time::Instant::now();
        let shown = e.candidates(8).len();
        println!(
            "{root}: select={select_time:?} candidates={:?} shown={shown} out={out}",
            t1.elapsed()
        );
    }
}
