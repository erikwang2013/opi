// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 高频词库的层序门禁（2026-11-08 用户裁决「先使用高频词库再结合其他词库一起
//! 识别，注意去重」）。
//!
//! 与 `dict_layers.rs` 同一套真实候选路径（`Engine::candidates`），单独成文件是
//! 为了守住 500 行门禁；夹具是内存词库，不读 luna。

use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, InMemoryDictionary};

fn dict(entries: &[(&str, &str, u32)]) -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    for (py, w, f) in entries {
        d.insert(py, w, *f);
    }
    d
}

fn engine(base: &[(&str, &str, u32)]) -> Engine {
    let mut e = Engine::with_dictionaries(
        Box::new(dict(base)),
        None,
        SymbolEngine::new(Vec::new(), Vec::new()),
        false,
    );
    e.set_import_scan_budget(None);
    e
}

fn type_keys(e: &mut Engine, keys: &str) {
    for ch in keys.chars() {
        e.input_key(ch);
    }
}

/// 同一键下 high_freq 的条目排在 other 导入层与普通内置之前；同一 (词, 键) 在
/// 候选表里只出现一次（合并取 max，不是叠成两条）。
#[test]
fn high_freq_layer_ranks_first_and_dedupes() {
    // 加一条与 hao 无关的「量纲顶」条目：`base_max/128` 不为 0，导入层的
    // raw freq 才能分出先后（只放 100 会让 unit=0，所有导入条目并列）。
    let mut e = engine(&[
        ("hao", "好", 100),
        ("hao", "号", 50),
        ("zzz", "某", 3_000_000_000),
    ]);
    e.set_extra_dicts(vec![
        (
            "high_freq".into(),
            dict(&[("hao", "好", 3_000_000), ("hao", "壕", 2_900_000)]),
        ),
        ("other".into(), dict(&[("hao", "豪", 2_800_000)])),
    ]);
    type_keys(&mut e, "hao");
    let cands = e.candidates(8);
    let texts: Vec<&str> = cands.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(
        texts.iter().filter(|t| **t == "好").count(),
        1,
        "跨层同词必须去重：{texts:?}"
    );
    let pos = |t: &str| texts.iter().position(|x| *x == t).unwrap();
    assert!(pos("好") < pos("壕"), "high_freq 内部按词频：{texts:?}");
    assert!(
        pos("壕") < pos("豪"),
        "high_freq 层压过 other 导入层：{texts:?}"
    );
    assert!(pos("豪") < pos("号"), "导入层压过普通内置：{texts:?}");
}
