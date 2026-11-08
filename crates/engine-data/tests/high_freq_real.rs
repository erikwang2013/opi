// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **高频词库 · 真库组**（2026-11-08）：`data/raw/high_freq.tsv` 走**导入词库**
//! 通路（`engine_data::load_extra_dict`），不是编译进 .opid 的主库。
//!
//! 钉四件事：
//!   1. 六类合计 2800 条、注释头不进词条、3 列格式零坏行；
//!   2. 代表性条目按普通话拼音可达（含方言单字、网络 ASCII 字面键）；
//!   3. 词频列确实表达库内优先序（高频词语 > 方言），而不是同一个数；
//!   4. Android assets 与 fcitx5 部署副本都和 `data/raw/` 真源逐字节相同。
//!
//! 数据是项目自撰（词表 MIT）；高频字排序来自 Unihan kHanyuPinlu（Unicode
//! License），逐源见 `data/raw/LICENSES.md`。

use std::path::{Path, PathBuf};

use engine_data::Dictionary;

fn tsv_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/raw/high_freq.tsv")
}

/// Android 走 assets 副本，fcitx5 的 CI 容器编译吃 `crates/fcitx5-opi/data/` 副本
/// （其余端构建期从 `data/raw/` 原样拷）：三份必须逐字节相同，否则「入库数据」与
/// 「用户实际装载的数据」会静默漂开。
#[test]
fn deployment_copies_are_byte_identical() {
    let truth = std::fs::read(tsv_path()).expect("读 data/raw 真源");
    for rel in [
        "../../android/app/src/main/assets/high_freq.tsv",
        "../../crates/fcitx5-opi/data/high_freq.tsv",
    ] {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
        assert_eq!(
            std::fs::read(&p).expect("读部署副本"),
            truth,
            "{rel} 与 data/raw/high_freq.tsv 不一致"
        );
    }
}

#[test]
fn high_freq_tsv_loads_all_2800_entries() {
    let path = tsv_path();
    assert!(path.is_file(), "缺 {} —— 入库数据被误删？", path.display());
    let (dict, bad) = engine_data::load_extra_dict(&path).expect("high_freq.tsv 解析");
    assert_eq!(bad, 0, "注释头与 3 列格式不得产生坏行");
    assert_eq!(dict.len(), 2800, "六类合计 2800 条（同词跨类去重后）");
}

#[test]
fn representative_entries_are_reachable_by_pinyin() {
    let (dict, _) = engine_data::load_extra_dict(&tsv_path()).expect("high_freq.tsv 解析");
    for (py, word) in [
        ("nihao", "你好"),   // 高频用语
        ("bashi", "巴适"),   // 方言
        ("zazheng", "咋整"), // 方言 / 东北
        ("yyds", "yyds"),    // 网络词 ASCII 字面键
        ("de", "的"),        // 高频字
        ("women", "我们"),   // 高频词语
        ("chiguaqunzhong", "吃瓜群众"),
    ] {
        let hits = dict.query_all(py);
        assert!(
            hits.iter().any(|e| e.word == word),
            "{py} 应含「{word}」，实际 {:?}",
            hits.iter().map(|e| &e.word).collect::<Vec<_>>()
        );
    }
}

#[test]
fn freq_column_orders_categories() {
    let (dict, _) = engine_data::load_extra_dict(&tsv_path()).expect("high_freq.tsv 解析");
    let top = dict.query_all("women")[0].freq;
    let low = dict.query_all("zazheng")[0].freq;
    assert!(
        top > low,
        "高频词语（women={top}）应高于方言（zazheng={low}）"
    );
}
