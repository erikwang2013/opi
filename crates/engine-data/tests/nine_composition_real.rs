// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **9 键多字组合 · 真库组**（2026-10-23）—— 与 `opi-tools/tests/composition_real.rs`
//! 的全键盘验收同一个报障：整词不在词库里（`厌蠢症` / `样样通` 不是键），但
//! `yanchun + zheng`、`yangyang + tong` 两段都在，9 键按键串要能把它们组合出来。
//!
//! 为什么在 engine-data：读 `.opid` 的能力在这里（engine-core 不许有 IO）。真库是
//! 入库资产（`android/app/src/main/assets/luna.opid`）—— 缺文件直接红，不静默跳过。

use engine_core::router::KeyRouter;
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Scheme};
use engine_data::load_mmap;
use std::path::Path;

const LUNA_OPID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);

fn real_engine() -> Engine {
    let dict =
        load_mmap(Path::new(LUNA_OPID)).unwrap_or_else(|e| panic!("加载 {LUNA_OPID} 失败：{e:?}"));
    let mut e = Engine::new(
        Box::new(dict),
        SymbolEngine::new(Vec::new(), Vec::new()),
        false,
    );
    e.set_scheme(Scheme::Nine);
    e
}

fn type_str(e: &mut Engine, input: &str) {
    e.clear();
    for ch in input.chars() {
        e.input_key(ch);
    }
}

fn texts(e: &mut Engine, digits: &str, limit: usize) -> Vec<String> {
    type_str(e, digits);
    e.candidates(limit).into_iter().map(|c| c.text).collect()
}

#[test]
fn nine_key_user_reported_words_are_reachable_and_selectable() {
    for (digits, want) in [("926248694364", "厌蠢症"), ("926492648664", "样样通")] {
        let mut r = KeyRouter::new(real_engine());
        type_str(r.engine_mut(), digits);
        let cands = r.engine().candidates(usize::MAX);
        let rank = cands
            .iter()
            .position(|c| c.text == want)
            .map(|i| i + 1)
            .unwrap_or(0);
        eprintln!(
            "9 键组合真库：{digits} → {want} 第 {rank} 位（共 {} 条）",
            cands.len()
        );
        assert!(
            rank > 0,
            "{digits} 的候选里没有 {want}：{:?}",
            &cands[..cands.len().min(20)]
        );
        assert!(rank <= 8, "{want} 排到第 {rank} 位，移动端第一页看不到");
        // 9 键拼音行由候选读音驱动：整串可读时显示完整拼音（yan'chun'zheng），
        // 而不是把按键数字端上去。`'` 不占键位。
        let shown = r.engine().display_buffer();
        assert_eq!(
            shown.chars().filter(|c| *c != '\'').count(),
            digits.len(),
            "拼音行应覆盖全部 {} 键：{shown:?}",
            digits.len()
        );
        // 选它 = 整段消耗（组合词覆盖整串），不是「先确认第一个字」。
        let idx = cands.iter().position(|c| c.text == want).unwrap();
        assert_eq!(r.select(idx), want);
        assert_eq!(r.engine().buffer(), "", "整段消耗（不留下半截按键）");
        assert_eq!(r.engine().pending_text(), "", "组合不是逐字确认");
    }
}

/// 半截按键串 / 乱按不得组合出点名的新词（整串必须全覆盖，与全键盘组的负控制同源）。
#[test]
fn incomplete_and_garbage_digit_strings_do_not_compose() {
    let mut e = real_engine();
    for digits in [
        "92624869436",  // yanchunzhen（少最后一键）
        "92649264866",  // yangyangton
        "222222222222", // 乱按
        "62626262",
    ] {
        let got = texts(&mut e, digits, usize::MAX);
        assert!(
            !got.iter().any(|t| t == "厌蠢症" || t == "样样通"),
            "{digits} 不该组合出点名的新词：{:?}",
            &got[..got.len().min(20)]
        );
    }
}

/// 同一输入两次结果逐条相同（排序/去重无隐藏状态）。
#[test]
fn nine_key_composition_is_deterministic() {
    let mut e = real_engine();
    for digits in ["926248694364", "926492648664"] {
        let first = texts(&mut e, digits, usize::MAX);
        let second = texts(&mut e, digits, usize::MAX);
        assert_eq!(first, second, "{digits} 两次结果必须逐条相同");
    }
}
