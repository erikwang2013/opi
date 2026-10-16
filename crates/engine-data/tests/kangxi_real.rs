// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **康熙字典词库 · 真库组**（2026-10-16 用户裁决「把 shunshi-kangxi-core 中康熙字典
//! 提取词库，作项目适配词库」）。
//!
//! 数据链：npm `shunshi-kangxi-core@0.1.1`（MIT）→ `scripts/gen_kangxi_dict.py`
//! 适配（读音规范化 + `pinyin.rs::SYLLABLES` 过滤 + 低位常数词频）→
//! `data/raw/kangxi.tsv` → `opi-tools compile` → 本文件加载的 `data/generated/kangxi.opid`
//! （入库资产，照 fallback/trad/wubi 的先例；缺文件直接红 —— 它同时是生成器的产物门禁）。
//!
//! 钉的是**适配后真吃得进引擎**：常见字、生僻字、多音字（上游空格分隔的副读音
//! 全部展开成多条）都能按读音查到。词库本身走四端共用的 TSV 导入契约分发
//! （见生成器头注），本文件不测那条通路。

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

/// 康熙库字节（入库资产，缺文件直接红 —— 它是生成器链的产物门禁）。
fn kangxi_bytes() -> Vec<u8> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/generated/kangxi.opid");
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "读不到 {}（{e}）—— 它是生成器（scripts/gen_kangxi_dict.py + opi-tools compile）\
             的入库产物，缺了本门禁就没在测东西",
            path.display()
        )
    })
}

/// 康熙库装进引擎（**独占**词典：本组测的是这份数据自己）。
fn kangxi_engine() -> Engine {
    let dict = engine_data::load_bytes(kangxi_bytes()).expect("kangxi.opid 解析");
    let mut e = Engine::new(Box::new(dict), SymbolEngine::builtin(), true);
    e.switch_mode(Mode::Pinyin);
    e
}

/// 常见字与**生僻字**都按读音可达（生僻字是这份词库存在的理由）。
#[test]
fn common_and_rare_chars_are_reachable_by_reading() {
    let mut e = kangxi_engine();
    for (py, ch) in [
        ("kao", "丂"),
        ("si", "亖"),
        ("da", "龘"),
        ("lve", "寽"), // 上游写 luè —— lue→lve 归一（与 gen_luna_dict 的 U_FIX 同一对）
    ] {
        type_str(&mut e, py);
        let got = texts(&e, 256);
        assert!(
            got.contains(&ch.to_string()),
            "打 {py} 应出「{ch}」（前 8：{:?}）",
            &got[..8.min(got.len())]
        );
        e.clear();
    }
}

/// 多音字：上游**全部**读音都展开（`乄` 在源数据里是 `shī  mē` 两个音）。
#[test]
fn polyphone_readings_are_all_expanded() {
    let mut e = kangxi_engine();
    for py in ["shi", "me"] {
        type_str(&mut e, py);
        let got = texts(&e, 256);
        assert!(
            got.contains(&"乄".to_string()),
            "打 {py} 应出「乄」：{:?}",
            &got[..8.min(got.len())]
        );
        e.clear();
    }
}

/// 条数下限：适配产物流失不许超过「无读音 + 读音不合法」两笔已计入的量
/// （生成器输出 20939 条；这条防的是**积累/编译**层悄悄吞条目）。
#[test]
fn entry_count_is_loaded_whole() {
    let dict = engine_data::load_bytes(kangxi_bytes()).expect("kangxi.opid 解析");
    assert!(
        engine_core::dictionary::Dictionary::len(&dict) >= 20_900,
        "词条数应 ≥ 20900（生成器实测 20939）"
    );
}
