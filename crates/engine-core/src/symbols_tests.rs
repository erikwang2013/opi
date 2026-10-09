// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! symbols.rs 的内联测试 —— 2026-10-09 拆出（加 builtin_emoji_texts 后源文件
//! 517 行越了 500 行门禁；与 `candidates_tests.rs` / `router_tests.rs` 同惯例，
//! 拆分是机械搬移，判据与注释一条未改）。

use super::*;

/// 查询路径的通用断言一律走**内联样例**，不碰 `builtin()`/数据文件：
/// 那两份表由 scripts 生成，内容（区块编号、关键字、emoji 标记）会随生成器变，
/// 钉死内容等于把测试绑在别人的数据上。
fn sample() -> SymbolEngine {
    SymbolEngine::new(
        vec![
            Block {
                id: BlockId(1),
                start: 0x3000,
                end: 0x303F,
                name: "CJK 符号".into(),
                common: true,
            },
            Block {
                id: BlockId(3),
                start: 0x2600,
                end: 0x26FF,
                name: "杂项符号".into(),
                common: true,
            },
            Block {
                id: BlockId(4),
                start: 0x1F600,
                end: 0x1F64F,
                name: "表情符号".into(),
                common: false,
            },
        ],
        vec![
            SymbolEntry {
                text: "♥".into(),
                name: "心形".into(),
                keywords: vec!["heart".into(), "xin".into()],
                block: BlockId(3),
                emoji: false,
            },
            SymbolEntry {
                text: "😄".into(),
                name: "微笑".into(),
                keywords: vec!["xiao".into(), "smile".into()],
                block: BlockId(4),
                emoji: true,
            },
        ],
    )
}

/// 接线守卫：`builtin()` 走编译期嵌入的数据表，表空/路径错会让面板整个空掉。
/// 只断言「非空」这类结构性质，不钉具体内容（内容归生成器）。
#[test]
fn builtin_panel_is_not_empty() {
    let e = SymbolEngine::builtin();
    assert!(
        !e.common_blocks().is_empty(),
        "常用区块为空：data/raw/symbol_blocks.tsv 内容可疑"
    );
    assert!(
        e.block_of('。').is_some() || e.block_of('♥').is_some(),
        "区块区间与常见符号对不上"
    );
}

#[test]
fn block_lookup_returns_owning_block() {
    let e = sample();
    assert_eq!(e.block_of('♥').unwrap().id, BlockId(3));
    assert_eq!(e.block_of('😄').unwrap().id, BlockId(4));
    assert_eq!(e.block_of('中'), None, "区间外的字符没有所属区块");
}

#[test]
fn common_blocks_sorted_by_start() {
    let e = sample();
    let blocks = e.common_blocks();
    assert_eq!(blocks.len(), 2);
    assert!(blocks.windows(2).all(|w| w[0].start <= w[1].start));
}

#[test]
fn entries_in_block_returns_members() {
    let e = sample();
    let texts: Vec<String> = e
        .entries_in_block(BlockId(3))
        .iter()
        .map(|s| s.text.clone())
        .collect();
    assert_eq!(texts, vec!["♥".to_string()]);
    assert!(e.entries_in_block(BlockId(99)).is_empty());
}

#[test]
fn search_by_pinyin_and_english_keyword() {
    let e = sample();
    assert!(e.search("xiao").iter().any(|s| s.text == "😄"));
    assert!(e.search("heart").iter().any(|s| s.name == "心形"));
}

#[test]
fn search_matches_keyword_prefix() {
    let e = sample();
    assert!(e.search("x").iter().any(|s| s.text == "😄")); // xiao
    assert!(e.search("sm").iter().any(|s| s.text == "😄")); // smile
    assert!(e.search("he").iter().any(|s| s.text == "♥")); // heart
}

#[test]
fn search_no_match_empty() {
    let e = sample();
    assert!(e.search("zzz").is_empty());
}

/// 数据文件格式（冻结，scripts 生成）：
/// symbol_blocks.tsv: `id \t start(hex) \t end(hex) \t name \t common(0/1)`
/// symbols.tsv:       `text \t name \t keywords(逗号分隔) \t block_id \t emoji(0/1)`
const BLOCKS_TSV: &str = "\
# id\tstart\tend\tname\tcommon
1\t3000\t303F\tCJK 符号\t1
2\t1F600\t1F64F\t表情符号\t1
3\t0x25A0\t25FF\t几何图形\t0
";
const SYMBOLS_TSV: &str = "\
。\t句号\tju,period\t1\t0
😄\t微笑\txiao,smile\t2\t1
▲\t上三角\tsjx，triangle\t3\t0
∅\t空集\t\t3\t0
";

#[test]
fn from_tsv_parses_blocks_and_entries() {
    let e = SymbolEngine::from_tsv(BLOCKS_TSV, SYMBOLS_TSV).unwrap();
    // 区块：解析、排序、common 标记
    assert_eq!(e.common_blocks().len(), 2);
    assert_eq!(e.block_of('。').unwrap().id, BlockId(1));
    assert_eq!(e.block_of('。').unwrap().name, "CJK 符号");
    assert_eq!(e.block_of('😄').unwrap().id, BlockId(2));
    assert_eq!(e.block_of('▲').unwrap().end, 0x25FF);
    // 条目：区块归属、emoji 标记、空 keywords 列
    let emoji = e.entries_in_block(BlockId(2));
    assert_eq!(emoji.len(), 1);
    assert_eq!(emoji[0].text, "😄");
    assert!(emoji[0].emoji);
    assert_eq!(
        emoji[0].keywords,
        vec!["xiao".to_string(), "smile".to_string()]
    );
    let geom = e.entries_in_block(BlockId(3));
    assert_eq!(geom.len(), 2);
    assert!(
        geom.iter()
            .find(|s| s.text == "∅")
            .unwrap()
            .keywords
            .is_empty()
    );
    // 全角逗号也当分隔符（▲ 的 keywords 用的是 "sjx，triangle"）
    assert_eq!(
        geom.iter().find(|s| s.text == "▲").unwrap().keywords,
        vec!["sjx".to_string(), "triangle".to_string()]
    );
    // 查询路径与 new() 构造的完全一致
    assert!(e.search("xi").iter().any(|s| s.text == "😄"));
    assert!(e.search("smile").iter().any(|s| s.text == "😄"));
    assert!(e.search("tri").iter().any(|s| s.text == "▲"));
    assert_eq!(e.search("heart"), Vec::new());
}

#[test]
fn from_tsv_error_message_carries_line_number() {
    let err = SymbolEngine::from_tsv("1\t3000\tZ\tCJK\t1\n", "").unwrap_err();
    assert!(err.contains("第 1 行"), "{err}");
    let err = SymbolEngine::from_tsv(BLOCKS_TSV, "。\t句号\tju\t1\tX\n").unwrap_err();
    assert!(err.contains("第 1 行"), "{err}");
}

#[test]
fn from_tsv_rejects_bad_input() {
    let cases: &[(&str, &str)] = &[
        ("1\t3000\tZ\tCJK\t1\n", ""),                           // end 非十六进制
        ("1\t3000\t303F\tCJK\t2\n", ""),                        // common 非 0/1
        ("1\t3040\t303F\tCJK\t1\n", ""),                        // start > end
        ("1\t3000\t303F\tCJK\n", ""),                           // 缺列
        ("1\t3000\t303F\tCJK\t1\textra\n", ""),                 // 多列
        ("1\t3000\t303F\tCJK\t1\n1\t3030\t3040\tX\t1\n", ""),   // 区块重叠
        ("1\t3000\t303F\tCJK\t1\n1\t3000\t303F\tCJK\t1\n", ""), // 区块 ID 重复
        ("65536\t3000\t303F\tCJK\t1\n", ""),                    // id 超 u16
        (BLOCKS_TSV, "。\t句号\tju\t9\t0\n"),                   // 引用不存在的区块
        (BLOCKS_TSV, "\t句号\tju\t1\t0\n"),                     // 空 text
        (BLOCKS_TSV, "。\t句号\tju\t1\n"),                      // 缺列
    ];
    for (blocks, entries) in cases {
        assert!(
            SymbolEngine::from_tsv(blocks, entries).is_err(),
            "应拒绝：blocks={blocks:?} entries={entries:?}"
        );
    }
}

#[test]
fn from_tsv_allows_empty_data() {
    // 空符号表不是错误：面板空着好过崩。
    let e = SymbolEngine::from_tsv("", "").unwrap();
    assert!(e.common_blocks().is_empty());
    assert!(e.search("x").is_empty());
    assert_eq!(e.block_of('。'), None);
}

#[test]
fn search_returns_deterministic_unique() {
    // 同一关键字挂在多个条目上（两个条目都带 x 前缀关键字）也不得重复输出。
    let e = SymbolEngine::new(
        vec![Block {
            id: BlockId(1),
            start: 0x1F600,
            end: 0x1F64F,
            name: "表情".into(),
            common: true,
        }],
        vec![
            SymbolEntry {
                text: "😄".into(),
                name: "微笑".into(),
                keywords: vec!["xiao".into(), "x".into()],
                block: BlockId(1),
                emoji: true,
            },
            SymbolEntry {
                text: "😆".into(),
                name: "大笑".into(),
                keywords: vec!["xiao".into()],
                block: BlockId(1),
                emoji: true,
            },
        ],
    );
    let got = e.search("xiao");
    let mut texts: Vec<String> = got.iter().map(|s| s.text.clone()).collect();
    texts.sort();
    let mut uniq = texts.clone();
    uniq.dedup();
    assert_eq!(texts, uniq);
    assert_eq!(texts, vec!["😄".to_string(), "😆".to_string()]);
}
