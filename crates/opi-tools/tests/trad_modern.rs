// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 繁体现代词 + 康熙补充读音的**候选窗可达**门禁（2026-10-18 用户裁决）。
//!
//! 数据链：
//! - `scripts/gen_trad_modern.py` → `data/raw/trad_modern.tsv`（luna 前 10 万词组的
//!   OpenCC s2t 转换；产物入库）
//! - `scripts/gen_kangxi_extra.py` → `data/raw/kangxi_{trad,luna}.tsv`（只补差集）
//! - `cat 四份 TSV | opi-tools compile` → `data/generated/trad.opid`（部署副本在
//!   android assets）；luna 由部署副本 + `kangxi_luna.tsv` 重编。
//!
//! 判据是「手机抓取窗（前 64）可达」，不是「库里有」：4000 词频的康熙字实测在
//! 64 位之外，等于打不出。词频常数由 `gen_kangxi_extra.py` 头注记录取舍。

use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Mode};
use engine_data::load_mmap;
use std::path::Path;

const TRAD: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/generated/trad.opid"
);
const LUNA: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);

fn engine(path: &str, mode: Mode) -> Engine {
    let dict = load_mmap(Path::new(path)).expect("加载词库");
    let mut e = Engine::new(
        Box::new(dict),
        SymbolEngine::new(Vec::new(), Vec::new()),
        false,
    );
    e.switch_mode(mode);
    e
}

fn type_keys(e: &mut Engine, keys: &str) -> Vec<String> {
    e.clear();
    for c in keys.chars() {
        e.input_key(c);
    }
    e.candidates(64).into_iter().map(|c| c.text).collect()
}

#[test]
fn modern_traditional_words_are_in_the_first_window() {
    let mut e = engine(TRAD, Mode::Traditional);
    for (keys, word) in [
        ("jisuanji", "計算機"),
        ("liulanqi", "瀏覽器"),
        ("rengongzhineng", "人工智能"),
        ("shujuku", "數據庫"),
        ("caozuoxitong", "操作系統"),
        ("ruanjiankaifa", "軟件開發"),
        ("zhinengshouji", "智能手機"),
    ] {
        let got = type_keys(&mut e, keys);
        assert!(
            got.iter().any(|t| t == word),
            "打 {keys} 应在前 64 内出「{word}」—— 实得：{got:?}"
        );
    }
}

#[test]
fn kangxi_supplement_is_reachable_in_both_modes() {
    // (拼音, 目标字, 常用头)：康熙差集读音 + 常用字头必须仍在前面。
    for (py, target, head) in [("me", "乄", "麼"), ("o", "丆", "哦")] {
        for (path, mode) in [(TRAD, Mode::Traditional), (LUNA, Mode::Pinyin)] {
            let mut e = engine(path, mode);
            let got = type_keys(&mut e, py);
            assert!(
                got.iter().any(|t| t == target),
                "{path} 打 {py} 应在前 64 内出康熙字「{target}」—— 实得：{got:?}"
            );
            assert!(
                got.iter().take(16).any(|t| t == head),
                "{path} 打 {py} 常用字头「{head}」必须仍在前 16 —— 实得：{got:?}"
            );
        }
    }
}

#[test]
fn existing_traditional_ranking_is_unchanged() {
    // 既有 ranking_quality 的 STRICT 头三位：合并现代词不应把它们挤下去。
    let mut e = engine(TRAD, Mode::Traditional);
    for (py, want) in [("ni", "你"), ("hao", "好"), ("shi", "是"), ("zhong", "中")] {
        let got = type_keys(&mut e, py);
        assert_eq!(
            got.first().map(String::as_str),
            Some(want),
            "打 {py} 首位应仍是「{want}」：{got:?}"
        );
    }
}
