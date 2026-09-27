// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `candidates.rs` 的单元测试（`#[path]` 引入，见该文件尾部）：单测独立成文件以保持
//! 源文件 <500 行，与 `router_tests.rs` / 两轨的 `input_method_tests.rs` 同惯例。

use super::*;
use crate::dictionary::InMemoryDictionary;

fn test_dict() -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    d.insert("hao", "好", 5000);
    d.insert("hao", "号", 1200);
    d.insert("hao", "豪", 800);
    d.insert("xiao", "笑", 3000);
    d.insert("xiao", "小", 2000);
    d.insert("xiao", "校", 1000);
    d
}

/// 排序测试用的**空**符号表：符号来自生成的数据文件，其关键字会往拼音候选里掺料，
/// 钉死「恰好几个候选」的断言不该受它影响。
fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

/// 只含 😄(xiao/smile) 与 ♥(heart) 的内联符号表，专供合并/去重路径断言。
fn emoji_symbols() -> SymbolEngine {
    use crate::symbols::{Block, BlockId, SymbolEntry};
    SymbolEngine::new(
        vec![
            Block {
                id: BlockId(1),
                start: 0x2600,
                end: 0x26FF,
                name: "杂项符号".into(),
                common: true,
            },
            Block {
                id: BlockId(2),
                start: 0x1F600,
                end: 0x1F64F,
                name: "表情符号".into(),
                common: true,
            },
        ],
        vec![
            SymbolEntry {
                text: "♥".into(),
                name: "心形".into(),
                keywords: vec!["heart".into()],
                block: BlockId(1),
                emoji: false,
            },
            SymbolEntry {
                text: "😄".into(),
                name: "微笑".into(),
                keywords: vec!["xiao".into(), "smile".into()],
                block: BlockId(2),
                emoji: true,
            },
        ],
    )
}

#[test]
fn rank_score_boost_dominates() {
    assert!(rank_score(0, 1, USER_BOOST) > rank_score(5000, 0, USER_BOOST));
}

#[test]
fn picks_dictionary_sorted_by_freq() {
    let d = test_dict();
    let s = no_symbols();
    let l = Learner::new(false);
    let got = rank_and_pick(&d, &s, &l, "hao", Mode::Pinyin, DEFAULT_TOP_N, USER_BOOST);
    assert_eq!(got.len(), 3);
    assert_eq!(got[0].text, "好");
    assert_eq!(got[1].text, "号");
    assert_eq!(got[2].text, "豪");
    assert!(got.iter().all(|c| c.kind == CandidateKind::Hanzi));
}

#[test]
fn empty_input_gives_empty() {
    let d = test_dict();
    let s = no_symbols();
    let l = Learner::new(false);
    assert!(rank_and_pick(&d, &s, &l, "", Mode::Pinyin, DEFAULT_TOP_N, USER_BOOST).is_empty());
}

#[test]
fn non_pinyin_mode_gives_empty() {
    let d = test_dict();
    let s = no_symbols();
    let l = Learner::new(false);
    assert!(rank_and_pick(&d, &s, &l, "hao", Mode::English, DEFAULT_TOP_N, USER_BOOST).is_empty());
}

#[test]
fn traditional_mode_queries_dict() {
    let d = test_dict();
    let s = no_symbols();
    let l = Learner::new(false);
    let got = rank_and_pick(
        &d,
        &s,
        &l,
        "hao",
        Mode::Traditional,
        DEFAULT_TOP_N,
        USER_BOOST,
    );
    assert_eq!(got[0].text, "好");
}

#[test]
fn traditional_empty_input_gives_empty() {
    let d = test_dict();
    let s = no_symbols();
    let l = Learner::new(false);
    assert!(rank_and_pick(&d, &s, &l, "", Mode::Traditional, DEFAULT_TOP_N, USER_BOOST).is_empty());
}

#[test]
fn learner_boost_reorders() {
    let d = test_dict();
    let s = no_symbols();
    let mut l = Learner::new(true);
    l.record_selection("豪");
    let got = rank_and_pick(&d, &s, &l, "hao", Mode::Pinyin, DEFAULT_TOP_N, USER_BOOST);
    assert_eq!(got[0].text, "豪");
}

#[test]
fn emoji_via_keyword_merge() {
    let d = test_dict();
    let s = emoji_symbols();
    let l = Learner::new(false);
    let got = rank_and_pick(&d, &s, &l, "xiao", Mode::Pinyin, DEFAULT_TOP_N, USER_BOOST);
    assert!(
        got.iter()
            .any(|c| c.kind == CandidateKind::Emoji && c.text == "😄")
    );
    assert!(got[0].text == "笑" || got[0].text == "小" || got[0].text == "校");
}

#[test]
fn limit_truncates() {
    let d = test_dict();
    let s = no_symbols();
    let l = Learner::new(false);
    let got = rank_and_pick(&d, &s, &l, "hao", Mode::Pinyin, 1, USER_BOOST);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].text, "好");
}

#[test]
fn dedupes_by_text() {
    let d = test_dict();
    let s = no_symbols();
    let mut l = Learner::new(true);
    l.record_selection("好");
    let got = rank_and_pick(&d, &s, &l, "hao", Mode::Pinyin, DEFAULT_TOP_N, USER_BOOST);
    assert_eq!(got.iter().filter(|c| c.text == "好").count(), 1);
}

/// 词典与符号表同词碰撞：跨源重复必须去重，即使分数不同不相邻。
fn colliding_symbols() -> SymbolEngine {
    let blocks = vec![crate::symbols::Block {
        id: crate::symbols::BlockId(1),
        start: 0x4E00,
        end: 0x9FFF,
        name: "CJK".into(),
        common: false,
    }];
    let entries = vec![crate::symbols::SymbolEntry {
        text: "好".into(),
        name: "好".into(),
        keywords: vec!["hao".into()],
        block: crate::symbols::BlockId(1),
        emoji: false,
    }];
    SymbolEngine::new(blocks, entries)
}

#[test]
fn dedupes_across_sources() {
    let d = test_dict();
    let s = colliding_symbols();
    let l = Learner::new(false);
    let got = rank_and_pick(&d, &s, &l, "hao", Mode::Pinyin, DEFAULT_TOP_N, USER_BOOST);
    assert_eq!(got.iter().filter(|c| c.text == "好").count(), 1);
    assert_eq!(got.len(), 3);
}

/// 逐音节回退分支必须与主路径遵守同一条不变式：**不能**按静态词频先截断再算分。
/// 回归形态：`dict.query(&syl, 3)` 截的是静态前 3，学过的词在加 boost 之前就被丢弃，
/// 于是「学了几十次也不出现」。
#[test]
fn fallback_keeps_learned_word_beyond_static_top3() {
    let mut d = InMemoryDictionary::new();
    d.insert("hao", "好", 10); // 静态词频最低，静态前 3 之外
    d.insert("hao", "号", 5000);
    d.insert("hao", "豪", 4000);
    d.insert("hao", "耗", 3000);
    d.insert("xiao", "笑", 5000);
    let s = no_symbols();
    let mut l = Learner::new(true);
    l.record_selection("好");

    // 整串 "haoxiao" 无词条 → 走逐音节回退
    let got = rank_and_pick(
        &d,
        &s,
        &l,
        "haoxiao",
        Mode::Pinyin,
        DEFAULT_TOP_N,
        USER_BOOST,
    );
    assert!(
        got.iter().any(|c| c.text == "好"),
        "学过的「好」不得在加 boost 前被静态词频前 3 截掉：{:?}",
        got.iter().map(|c| &c.text).collect::<Vec<_>>()
    );
    assert_eq!(got[0].text, "好", "带 boost 的「好」应排首位");
}

#[test]
fn multi_syllable_falls_back_to_per_syllable() {
    let d = test_dict();
    let s = no_symbols();
    let l = Learner::new(false);
    // 整串 "haoxiao" 无词条 → 按音节 [hao][xiao] 补出逐字候选
    let got = rank_and_pick(
        &d,
        &s,
        &l,
        "haoxiao",
        Mode::Pinyin,
        DEFAULT_TOP_N,
        USER_BOOST,
    );
    assert!(got.iter().any(|c| c.text == "好"));
    assert!(got.iter().any(|c| c.text == "笑"));
}

/// 精确等长匹配必须整体排在前缀扩展之前：查询是前缀语义（`xian` 命中 `xiang`），
/// 若只按静态词频排，「想」(xiang, 4640) 会顶掉「现」(xian, 3900) 与「先」(xian, 1962)。
/// 真机数据同形：luna `xian` 首位曾是「想」、`shu` 首位「说」(shuo)、`xi` 首位「下」(xia)。
#[test]
fn exact_pinyin_beats_prefix_extension() {
    let mut d = InMemoryDictionary::new();
    d.insert("xian", "现", 3900);
    d.insert("xian", "先", 1962);
    d.insert("xiang", "想", 4640); // 前缀扩展，静态词频更高
    let s = no_symbols();
    let l = Learner::new(false);
    let got = rank_and_pick(&d, &s, &l, "xian", Mode::Pinyin, DEFAULT_TOP_N, USER_BOOST);
    let pos = |w: &str| got.iter().position(|c| c.text == w).unwrap();
    let texts: Vec<&str> = got.iter().map(|c| c.text.as_str()).collect();
    assert!(
        pos("现") < pos("想"),
        "精确的「现」应压过前缀扩展的「想」：{texts:?}"
    );
    assert!(
        pos("先") < pos("想"),
        "精确的「先」应压过前缀扩展的「想」：{texts:?}"
    );
    assert_eq!(got[0].text, "现", "精确匹配内部仍按词频排：{texts:?}");
}

/// 学习必须仍能反超精确匹配：加成（boost/2）不得吃掉一次选词的收益（1×boost）。
/// 否则「你最常用的那个词」再也排不到首位（README 缘起 #3）。
#[test]
fn learned_prefix_extension_still_outranks_unlearned_exact() {
    let mut d = InMemoryDictionary::new();
    d.insert("xian", "现", 3900);
    d.insert("xiang", "想", 4640);
    let s = no_symbols();
    let mut l = Learner::new(true);
    l.record_selection("想");
    let got = rank_and_pick(&d, &s, &l, "xian", Mode::Pinyin, DEFAULT_TOP_N, USER_BOOST);
    assert_eq!(got[0].text, "想");
}

#[test]
fn symbol_only_input() {
    let d = test_dict();
    let s = emoji_symbols();
    let l = Learner::new(false);
    let got = rank_and_pick(&d, &s, &l, "heart", Mode::Pinyin, DEFAULT_TOP_N, USER_BOOST);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].text, "♥");
    assert_eq!(got[0].kind, CandidateKind::Symbol);
}

#[test]
fn symbol_prefix_search_merges_into_candidates() {
    let d = test_dict();
    let s = emoji_symbols();
    let l = Learner::new(false);
    // "x" 前缀命中 😄（keyword xiao），同时拼音 xiao 词也出现
    let got = rank_and_pick(&d, &s, &l, "x", Mode::Pinyin, DEFAULT_TOP_N, USER_BOOST);
    assert!(
        got.iter()
            .any(|c| c.kind == CandidateKind::Emoji && c.text == "😄")
    );
}

/// 符号模式（用户裁决）：候选**只出符号**，不与词库合并。
///
/// 合并版本里「、」永远赢不了：dun 组汉字带精确加成（3.9e9 量级），符号未学习态
/// score 0，被 DEFAULT_TOP_N 与 FETCH_LIMIT 双重截掉 —— 模式开关等于没生效。
///
/// 用**生产符号表**（`SymbolEngine::builtin()`，编译期嵌入 `data/raw/symbols.tsv`）
/// 而非内联假表：这条断言的价值全在数据侧（`dun` 这个关键字真的挂在「、」上）。
#[test]
fn symbol_mode_returns_symbols_only() {
    let mut d = InMemoryDictionary::new();
    d.insert("dun", "盾", 3_949_434_661);
    d.insert("dun", "顿", 3_940_724_765);
    let l = Learner::new(false);
    let got = rank_and_pick(
        &d,
        &SymbolEngine::builtin(),
        &l,
        "dun",
        Mode::Symbol,
        DEFAULT_TOP_N,
        USER_BOOST,
    );
    let texts: Vec<&str> = got.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(
        texts.first(),
        Some(&"、"),
        "符号模式第一条必须是「、」：{texts:?}"
    );
    assert!(
        got.iter()
            .all(|c| matches!(c.kind, CandidateKind::Symbol | CandidateKind::Emoji)),
        "符号模式混进了词库候选：{got:?}"
    );
}

/// 逐音节回退路径里也要有模糊（lead 订正 2）：整串无命中时按音节补候选，
/// 每个音节除自身外还要查它的模糊变体 —— `zong` 的变体 `zhong` 才带得来「中」。
/// 变体命中一律非精确，故「中」必须排在精确档的「总/国」之后。
#[test]
fn fallback_loop_appends_fuzzy_after_exact() {
    let mut d = InMemoryDictionary::new();
    d.insert("zong", "总", 1_000);
    d.insert("guo", "国", 900);
    d.insert("zhong", "中", 9_999);
    let l = Learner::new(false);
    let got = rank_and_pick(
        &d,
        &no_symbols(),
        &l,
        "zongguo",
        Mode::Pinyin,
        DEFAULT_TOP_N,
        USER_BOOST,
    );
    let texts: Vec<&str> = got.iter().map(|c| c.text.as_str()).collect();
    assert!(
        texts.contains(&"中"),
        "逐音节回退没有模糊：「中」缺：{texts:?}"
    );
    let pos = |w: &str| texts.iter().position(|t| *t == w).expect("候选必须在");
    assert!(
        pos("总") < pos("中") && pos("国") < pos("中"),
        "模糊命中抢到精确档前面：{texts:?}"
    );
}

/// 上一条的**另一条分支**：词库里一旦有连写词组，整串变体先命中（`wide_hits`），
/// 逐音节模糊**整段跳过** —— 「更具体的匹配优先」，与「整串命中就不逐音节」同一条分层。
///
/// 词库只比上一条多一行 `zhongguo → 中國`，行为就换轨：打 `zongguo` 出的是**词**「中國」，
/// 而不是单字「中」。这是产品上的严格更优（`zongguo` 的人是想要那个词）。
///
/// **两库的真实形态都已经是这一条**（2026-09-27 实测）：trad 词组连写（8 槽第 7 位「中國」），
/// 新 luna 的简体词组同样是连写键（`query_all("zongguo")` 0 行、`zhongguo` 81 行，
/// 8 槽第 7 位「中国」）。上一条（逐音节模糊版）现在是**替身规模下**才出现的形态。
/// **这条分支承重却曾无覆盖**：替身词库不插连写词组就永远到不了这里。
#[test]
fn whole_input_variant_suppresses_per_syllable_fuzzy() {
    let mut d = InMemoryDictionary::new();
    d.insert("zong", "总", 1_000);
    d.insert("guo", "国", 900);
    d.insert("zhong", "中", 9_999); // 逐音节模糊会带回的单字（词频故意压过「总」）
    d.insert("zhongguo", "中國", 800); // 连写词组：整串变体 zongguo→zhongguo 命中
    let l = Learner::new(false);
    let got = rank_and_pick(
        &d,
        &no_symbols(),
        &l,
        "zongguo",
        Mode::Pinyin,
        DEFAULT_TOP_N,
        USER_BOOST,
    );
    let texts: Vec<&str> = got.iter().map(|c| c.text.as_str()).collect();
    assert!(
        texts.contains(&"中國"),
        "整串变体必须带回词组「中國」：{texts:?}"
    );
    assert!(
        texts.len() <= DEFAULT_TOP_N,
        "候选栏只有 8 槽，不能靠挤压塞进来：{texts:?}"
    );
    assert!(
        !texts.contains(&"中"),
        "整串变体命中时逐音节模糊应被跳过（「中」会顶掉「中國」所在的那一槽）：{texts:?}"
    );
}
