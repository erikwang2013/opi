// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **有限窗口 = 全量列表前缀**（2026-10-25）。移动端用 `candidates(8/64)` 显示，
//! 提交时 `Engine::select` 用 `candidates(MAX)` 的全局下标 —— 两条路必须同表。
//! 单键首击走了 `candidates_finite` 的快速选择，这里用真库把它钉回全量前缀。

use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Mode};
use std::path::Path;

fn engine(learner: bool) -> Engine {
    let p =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../android/app/src/main/assets/luna.opid");
    let d = engine_data::load_bytes(std::fs::read(p).unwrap()).unwrap();
    let mut e = Engine::new(Box::new(d), SymbolEngine::builtin(), true);
    e.switch_mode(Mode::Pinyin);
    if learner {
        e.import_user_words(
            r#"{"version":1,"words":[
                {"text":"你好","freq":9},{"text":"不饿","freq":3},{"text":"此次","freq":2},
                {"text":"差额","freq":2},{"text":"主啊","freq":2}
            ]}"#,
        )
        .unwrap();
    }
    e
}

fn texts(e: &mut Engine, input: &str, limit: usize) -> Vec<String> {
    e.clear();
    for ch in input.chars() {
        e.input_key(ch);
    }
    e.candidates(limit).into_iter().map(|c| c.text).collect()
}

#[test]
fn finite_candidates_are_exact_prefixes_of_unlimited() {
    let inputs = [
        "a", "b", "c", "d", "e", "f", "g", "h", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s",
        "t", "w", "x", "y", "z", "ni", "hao", "nihao", "xian", "niha", "zhongguo",
    ];
    for learner in [false, true] {
        let mut e = engine(learner);
        for input in inputs {
            let full = texts(&mut e, input, usize::MAX);
            for limit in [1usize, 8, 64, 512, 4096] {
                let got = texts(&mut e, input, limit);
                let want = &full[..got.len().min(full.len())];
                assert_eq!(got, want, "limit={limit} input={input} learner={learner}");
                assert_eq!(got.len(), limit.min(full.len()), "长度 input={input}");
            }
        }
    }
}

#[test]
fn select_commits_the_candidate_that_was_displayed() {
    let mut e = engine(true);
    for input in ["b", "n", "c", "zh", "g"] {
        e.clear();
        for ch in input.chars() {
            e.input_key(ch);
        }
        let shown = e.candidates(64);
        let first = shown[0].text.clone();
        assert_eq!(e.select(0), first, "input={input} 显示与上屏不一致");
    }
}

fn assert_prefixes(e: &mut Engine, inputs: &[&str]) {
    for input in inputs {
        let full = texts(e, input, usize::MAX);
        for limit in [1usize, 8, 64, 512, 4096] {
            let got = texts(e, input, limit);
            assert_eq!(
                got,
                &full[..got.len().min(full.len())],
                "limit={limit} input={input}"
            );
            assert_eq!(got.len(), limit.min(full.len()), "长度 input={input}");
        }
    }
}

#[test]
fn traditional_mode_and_custom_phrases_keep_the_prefix_contract() {
    let mut e = engine(true);
    e.switch_mode(Mode::Traditional);
    assert_prefixes(&mut e, &["b", "n", "c", "zh", "g"]);

    let mut e = engine(true);
    e.set_custom_phrases(r#"{"version":1,"phrases":[{"trigger":"b","text":"自定义"}]}"#)
        .unwrap();
    e.set_custom_phrases_enabled(true);
    assert_prefixes(&mut e, &["b", "n", "c"]);
}

#[test]
fn nine_key_finite_candidates_are_exact_prefixes_too() {
    let mut e = engine(true);
    e.set_scheme(engine_core::Scheme::Nine);
    assert_prefixes(&mut e, &["2", "6", "9", "64", "64426", "644269"]);
}
