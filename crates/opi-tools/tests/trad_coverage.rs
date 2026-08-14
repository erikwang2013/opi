//! 单字全覆盖门禁（spec 2026-08-15 测试节）：trad_hanzi.tsv 每行 (word, pinyin)
//! 逐一 query 断言该字出现在候选（GB2312 6763 字 ⊂ 期待表）；数据产物提交入库（Task 1）。
//! 本测试只读不联网；trad.opid 缺失时报错并引导执行 Task 1 数据构建。

use engine_data::{load_mmap, Dictionary};
use std::path::Path;

const RAW_TSV: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/raw/trad_hanzi.tsv");
const GENERATED_OPID: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/generated/trad.opid");

fn rows() -> Vec<(String, String)> {
    let text = std::fs::read_to_string(RAW_TSV)
        .unwrap_or_else(|e| panic!("读取 {RAW_TSV} 失败（先执行 Task 1 数据构建并提交）：{e}"));
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut it = l.split('\t');
            let word = it.next().unwrap_or("").to_string();
            let pinyin = it.next().unwrap_or("").to_string();
            (word, pinyin)
        })
        .collect()
}

#[test]
fn every_tsv_char_queryable() {
    let dict = load_mmap(Path::new(GENERATED_OPID))
        .unwrap_or_else(|e| panic!("加载 {GENERATED_OPID} 失败（先执行 Task 1 数据构建并提交）：{e:?}"));
    let rows = rows();
    assert!(rows.len() >= 6763, "期待表应覆盖 GB2312 全量 6763 字，实际 {} 行", rows.len());
    let missing: Vec<(&str, &str)> = rows
        .iter()
        .filter(|(w, py)| !dict.query(py, usize::MAX).iter().any(|e| &e.word == w))
        .map(|(w, py)| (w.as_str(), py.as_str()))
        .take(10)
        .collect();
    assert!(missing.is_empty(), "以下 (字, 拼音) 查询无候选（最多展示 10）：{missing:?}");
}

#[test]
fn trad_spot_checks() {
    let dict = load_mmap(Path::new(GENERATED_OPID)).expect("trad.opid 已由 Task 1 提交");
    let has = |pinyin: &str, word: &str| dict.query(pinyin, usize::MAX).iter().any(|e| e.word == word);
    assert!(has("fa", "發"));
    assert!(has("fa", "髮"));
    assert!(has("taiwan", "臺灣"));
    assert!(has("zhonghuaminguo", "中華民國"));
    assert!(has("hao", "好")); // 简繁同形字在 trad 库也可打
}
