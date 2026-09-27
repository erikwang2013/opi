// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 候选排序不变量（`rank_and_pick`）—— 每条都对应一个已知会退回的缺陷形态：
//! - 精确等长匹配必须**整体**压过前缀扩展：`exact_bonus = boost/2` 是这条的**唯一**
//!   实现手段，改成 0（或把 bonus 加在排序之后）即红。
//! - `limit` 只能用于**排序之后**的截断，不得下推给 `dict.query`：否则学过的低静态词
//!   在加 boost 之前就被丢掉（缘起 #3 的回归点）。
//! - 空输入不出候选：词典替身对空串照答，守卫必须长在 `rank_and_pick` 这一层。
//! - 去重取**最高分**那一条：同一词挂在多条拼音行下（多音字）时，`dict.query` 会返回
//!   同一文本多次，去重关掉即红。

use engine_core::candidates::{Candidate, USER_BOOST, rank_and_pick, rank_score};
use engine_core::composer::Mode;
use engine_core::dictionary::{Dictionary, InMemoryDictionary};
use engine_core::learner::Learner;
use engine_core::symbols::SymbolEngine;
use engine_core::trie::Entry;
use proptest::prelude::*;

/// 空符号表：符号数据来自生成的文件，与这里的断言无关。
fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

fn cand_texts(cs: &[Candidate]) -> Vec<String> {
    cs.iter().map(|c| c.text.clone()).collect()
}

/// 「不设防」的词典替身：对空串也照答不误。
///
/// 两个真实实现（`Trie` 与 mmap）各自在 `query` 里挡了空串，因此**只有替身**能把
/// `rank_and_pick` 自己的空输入守卫暴露到断言下。契约「空缓冲 ⇒ 无候选」属于这一层
/// （`Dictionary::query` 的文档并未承诺空串返回空），靠下层兜等于没兜 —— 换一个实现
/// （如未来的 JNI/mmap 直连）就会漏。
struct NaiveDict;

impl Dictionary for NaiveDict {
    fn query(&self, _pinyin: &str, limit: usize) -> Vec<Entry> {
        vec![Entry {
            word: "好".into(),
            freq: 9,
            pinyin_len: 3,
        }]
        .into_iter()
        .take(limit)
        .collect()
    }
    fn len(&self) -> usize {
        1
    }
}

/// 空缓冲不得出候选 —— 词典替身对空串照答，`rank_and_pick` 必须自己挡住。
#[test]
fn empty_input_yields_no_candidates_even_if_dictionary_answers() {
    let symbols = no_symbols();
    let learner = Learner::new(true);
    for mode in [
        Mode::Pinyin,
        Mode::Traditional,
        Mode::English,
        Mode::Number,
        Mode::Symbol,
    ] {
        assert!(
            rank_and_pick(&NaiveDict, &symbols, &learner, "", mode, 8, USER_BOOST).is_empty(),
            "{mode:?}: 空输入出了候选"
        );
    }
}

/// 词与拼音一一对应，故「文本 → 是否精确匹配」无歧义。
const PAIRS: [(&str, &str); 10] = [
    ("xi", "甲"),
    ("xian", "乙"),
    ("xiang", "丙"),
    ("xia", "丁"),
    ("hao", "戊"),
    ("ha", "己"),
    ("h", "庚"),
    ("ni", "辛"),
    ("n", "壬"),
    ("xiao", "癸"),
];

fn pinyin_of(word: &str) -> &'static str {
    PAIRS
        .iter()
        .find(|(_, w)| *w == word)
        .map(|(p, _)| *p)
        .expect("候选文本必须来自 PAIRS")
}

/// 排序不变量（学习关闭时，对**任意**随机词频成立）：
/// 1. 候选数 ≤ limit；2. 文本不重复；3. 按 (分数降序, 文本升序) 排；
/// 4. **任取一个精确匹配与一个前缀扩展，前者的分数 ≥ 后者** ——
///    `exact_bonus = boost/2 ≥ 最大静态词频` 的直接推论，`exact_bonus = 0` 即红。
#[test]
fn rank_and_pick_invariants() {
    let inputs = prop::sample::select(vec!["xi", "xian", "xia", "hao", "ni", "xiao"]);
    let rows = prop::collection::vec((0usize..10, 0u32..1_000_000), 0..10);
    proptest!(|(input in inputs, rows in rows, limit in 0usize..12)| {
        let mut d = InMemoryDictionary::new();
        let mut max_f = 0u32;
        for (i, f) in &rows {
            d.insert(PAIRS[*i].0, PAIRS[*i].1, *f);
            max_f = max_f.max(*f);
        }
        let boost = USER_BOOST.max(max_f as u64 * 2); // 与 Engine::with_dictionaries 同源
        let l = Learner::new(false);
        let got = rank_and_pick(&d, &no_symbols(), &l, input, Mode::Pinyin, limit, boost);

        prop_assert!(got.len() <= limit, "候选 {} 条超过 limit {}", got.len(), limit);
        let texts = cand_texts(&got);
        let mut uniq = texts.clone();
        uniq.sort();
        uniq.dedup();
        prop_assert_eq!(uniq.len(), texts.len(), "候选文本重复: {:?}", texts);
        for w in got.windows(2) {
            let ordered = w[0].score > w[1].score
                || (w[0].score == w[1].score && w[0].text <= w[1].text);
            prop_assert!(ordered, "排序错误：{:?} 在 {:?} 之前", w[0], w[1]);
        }
        let exact: Vec<&Candidate> = got.iter()
            .filter(|c| pinyin_of(&c.text).len() == input.len()).collect();
        let ext: Vec<&Candidate> = got.iter()
            .filter(|c| pinyin_of(&c.text).len() != input.len()).collect();
        for e in &exact {
            for n in &ext {
                prop_assert!(e.score >= n.score,
                    "精确 {:?}({}) 输给前缀扩展 {:?}({})：boost={}", e, e.score, n, n.score, boost);
            }
        }
    });
}

/// `limit` 不得下推：截断只能发生在**排序之后**。构造一个静态词频远在 top-N 之外的词，
/// 给它学习权重后必须出现在候选里 —— 若把 limit 传给 `dict.query`，它在加 boost 前就被丢了。
#[test]
fn limit_is_not_pushed_down_into_dictionary_query() {
    let mut d = InMemoryDictionary::new();
    for i in 0..40 {
        d.insert("hao", &format!("生{i:02}"), 10_000 - i as u32);
    }
    d.insert("hao", "好", 1); // 静态垫底
    let s = no_symbols();
    let mut l = Learner::new(true);
    l.record_selection("好");
    let boost = USER_BOOST.max(10_000 * 2);
    let got = rank_and_pick(&d, &s, &l, "hao", Mode::Pinyin, 8, boost);
    assert_eq!(
        got[0].text,
        "好",
        "学过的词必须能反超截断线：{:?}",
        cand_texts(&got)
    );
    assert!(got.len() <= 8);
}

/// 逐音节兜底路径的 top-3 同样必须按**含学习权重的最终分**选，不能按静态词频选 ——
/// 静态垫底但学过的字（hao 组里的「好」，静态第 7）必须能进候选。
/// 若把 top-3 交给 `dict.query(&syl, 3)`（或先按静态排序再 take），它在加 boost 前就被丢了。
#[test]
fn per_syllable_fallback_picks_top3_by_final_score() {
    let mut d = InMemoryDictionary::new();
    for i in 0..6 {
        d.insert("hao", &format!("生{i}"), 10_000 - i as u32);
    }
    d.insert("hao", "好", 1); // 静态垫底
    d.insert("xiao", "笑", 9_000);
    let mut l = Learner::new(true);
    l.record_selection("好");
    let boost = USER_BOOST.max(10_000 * 2);
    let got = rank_and_pick(&d, &no_symbols(), &l, "haoxiao", Mode::Pinyin, 8, boost);
    assert_eq!(
        got[0].text,
        "好",
        "整串无命中时的兜底候选漏了学过的字：{:?}",
        cand_texts(&got)
    );
    assert!(
        got.iter().any(|c| c.text == "笑"),
        "第二个音节（xiao）的候选丢了：{:?}",
        cand_texts(&got)
    );
    assert!(got.len() <= 8);
}

/// 同一词挂在**多条拼音行**下（多音字：好 hǎo/hào，行 xíng/háng）时，`dict.query("ha")`
/// 会把同一文本返回多次；候选里每个文本只能出现一次，且留下的必须是**最高分**那条 ——
/// 「好」的两行里，先到的（hao/好 900）最终分反而低（它只是前缀扩展），
/// 后到的（ha/好 500）是精确匹配、含 exact_bonus，必须赢。
#[test]
fn duplicate_rows_are_merged_keeping_the_best_score() {
    let mut d = InMemoryDictionary::new();
    d.insert("ha", "好", 500); // 对输入 "ha" 是**精确**匹配 → 最终分最高
    d.insert("hao", "好", 900); // 前缀扩展 → 静态分高但最终分低
    d.insert("hao", "戊", 100);
    d.insert("haoh", "戊", 900);
    let raw = d.query("ha", usize::MAX);
    assert_eq!(
        raw.iter().map(|e| e.freq).collect::<Vec<_>>(),
        vec![900, 900, 500, 100],
        "前置：底层查询确实返回了重复文本（好×2、戊×2）"
    );

    let boost = USER_BOOST;
    let got = rank_and_pick(
        &d,
        &no_symbols(),
        &Learner::new(false),
        "ha",
        Mode::Pinyin,
        8,
        boost,
    );
    assert_eq!(cand_texts(&got), vec!["好", "戊"], "同词必须合并为一条");
    assert_eq!(
        got.iter().filter(|c| c.text == "好").count(),
        1,
        "「好」在候选里出现了两次（去重失效）"
    );
    assert_eq!(
        got[0].score,
        rank_score(500, 0, boost) + boost / 2,
        "好 应取精确行（ha/好 500 + exact_bonus），而不是先到的扩展行（900）"
    );
    assert_eq!(
        got[1].score,
        rank_score(900, 0, boost),
        "戊 应取高分行（haoh/戊 900），不是先到的低分行（hao/戊 100）"
    );
}
