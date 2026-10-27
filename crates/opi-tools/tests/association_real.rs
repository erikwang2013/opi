// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 真词库上的联想门禁（Spec §9.3）：入库的 luna/trad 必须带词排序索引与语义小表，
//! 且提交后真的能给出续接项。断言的**内容不写死** —— 小表随生成参数漂，只钉
//! 「索引可用」「联想非空」「含当前 assoc 表里的至少一个已知项」。

use engine_core::Engine;
use engine_core::dictionary::Dictionary;
use engine_core::symbols::SymbolEngine;

const LUNA: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);
const TRAD: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/trad.opid"
);
const LUNA_ASSOC: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/generated/luna.assoc.tsv"
);

fn type_str(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

fn known_suffixes(root: &str) -> Vec<String> {
    opi_tools::compiler::parse_assoc(&std::fs::read_to_string(LUNA_ASSOC).unwrap())
        .unwrap()
        .into_iter()
        .filter(|r| r.root == root)
        .map(|r| r.suffix)
        .collect()
}

#[test]
fn luna_has_index_and_assoc_and_associates_after_commit() {
    let bytes = std::fs::read(LUNA).unwrap();
    let v = engine_data::validate(&bytes).unwrap();
    assert_eq!(v.flags, 0b11);
    assert!(v.word_index.is_some());
    assert!(v.assoc.expect("luna assoc").root_count > 0);
    let dict = engine_data::load_bytes(bytes).unwrap();
    // 不硬编码语义表内容：先钉词索引真的能按前缀取词（luna 有「今天晚上」）。
    let prefixed = dict.words_with_prefix("今天");
    assert!(
        prefixed.iter().any(|(w, _)| w == "今天晚上"),
        "索引缺 今天→今天晚上：{prefixed:?}"
    );
    let mut e = Engine::new(Box::new(dict), SymbolEngine::builtin(), false);
    type_str(&mut e, "jintian");
    let idx = e
        .candidates(512)
        .iter()
        .position(|c| c.text == "今天")
        .expect("候选缺 今天");
    assert_eq!(e.select(idx), "今天");
    assert!(e.association_active());
    let got: Vec<String> = e.candidates(8).into_iter().map(|c| c.text).collect();
    assert!(!got.is_empty());
    assert!(
        !got.iter().any(|s| s.contains("今天")),
        "续接项不得包含根：{got:?}"
    );
    let mut uniq = got.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(uniq.len(), got.len(), "联想候选不得重复：{got:?}");

    // Spec §9.3：候选里至少有一个当前 assoc 表里的已知项（随表动态取，不硬编码）。
    let known = known_suffixes("今天");
    assert!(
        !known.is_empty(),
        "luna.assoc.tsv 里没有「今天」根：检查生成参数（roots 太小或短语表换了）"
    );
    assert!(
        got.iter().any(|s| known.iter().any(|k| s == k)),
        "联想未包含 assoc 表里的已知后缀：known={known:?} got={got:?}"
    );
}

#[test]
fn trad_has_extras() {
    let bytes = std::fs::read(TRAD).unwrap();
    let v = engine_data::validate(&bytes).unwrap();
    assert_eq!(v.flags, 0b11);
    assert!(v.assoc.expect("trad assoc").root_count > 0);
    let dict = engine_data::load_bytes(bytes).unwrap();
    assert!(
        dict.words_with_prefix("中國").len() > 1,
        "trad 词索引应能按「中國」取词（trad 用繁体字形）"
    );
    // 2026-10-27 实测：trad 的 assoc 表没有「今天」根，词库也没有以「今天」开头的
    // 词 ⇒ 选它必然建不出联想。改用有 assoc 根、也有前缀续接的「中國」（zhongguo）。
    let mut e = Engine::new(Box::new(dict), SymbolEngine::builtin(), false);
    type_str(&mut e, "zhongguo");
    let idx = e
        .candidates(512)
        .iter()
        .position(|c| c.text == "中國")
        .expect("候选缺 中國");
    assert_eq!(e.select(idx), "中國");
    assert!(e.association_active());
    assert!(!e.candidates(8).is_empty(), "trad 空缓冲联想非空");
}
