// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

use crate::composer::Mode;
use crate::dictionary::Dictionary;
use crate::learner::Learner;
use crate::pinyin::segment;
use crate::symbols::SymbolEngine;
use crate::trie::Entry;

/// 用户词频权重下限：一次选词 ≈ 10 万次静态词频，保证学习迅速生效。
/// 实际权重在 Engine::new 按词典最大词频动态缩放（max_freq × 2），
/// 固定值在 luna 百万级词频下失配：选一次"我"(10万) 仍输给"倭"(50万)。
pub const USER_BOOST: u64 = 100_000;
/// 默认候选栏容量。
pub const DEFAULT_TOP_N: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateKind {
    Hanzi,
    English,
    Emoji,
    Symbol,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub text: String,
    pub kind: CandidateKind,
    pub score: u64,
}

/// 排序分：静态词频 + 用户词频 × boost。
pub fn rank_score(static_freq: u32, user_freq: u32, boost: u64) -> u64 {
    (static_freq as u64).saturating_add((user_freq as u64).saturating_mul(boost))
}

/// 合并词典候选与符号候选，排序、去重、截断。
pub fn rank_and_pick<D: Dictionary + ?Sized>(
    dict: &D,
    symbols: &SymbolEngine,
    learner: &Learner,
    input: &str,
    mode: Mode,
    limit: usize,
    boost: u64,
) -> Vec<Candidate> {
    if input.is_empty() || !matches!(mode, Mode::Pinyin | Mode::Traditional) {
        return Vec::new();
    }
    let input_len = input.len();
    // 精确等长匹配加成 = boost/2。boost = max(USER_BOOST, 词典最大静态词频×2)
    // （见 Engine::with_dictionaries），故 boost/2 ≥ 最大静态词频 —— 任何精确匹配（哪怕
    // freq=0）都排在任何前缀扩展之前：输入 xian 不再由 xiang 的「想」占首位。
    // 而一次选词的最小收益是 1×boost > boost/2，学过的前缀扩展词仍能反超（缘起 #3 不受损）。
    let exact_bonus = boost / 2;
    let hanzi = |e: Entry, exact: bool| Candidate {
        text: e.word.clone(),
        kind: CandidateKind::Hanzi,
        score: rank_score(e.freq, learner.freq_of(&e.word), boost).saturating_add(if exact {
            exact_bonus
        } else {
            0
        }),
    };
    let mut merged: Vec<Candidate> = dict
        .query(input, usize::MAX)
        .into_iter()
        .map(|e| {
            let exact = e.pinyin_len == input_len;
            hanzi(e, exact)
        })
        .collect();
    // 多音节整串无命中时按音节逐段补候选（segment 此前是死代码）：
    // nihao → [你][好] 逐字可选；单字母音节跳过避免噪音。
    // 每音节仅取 top 3：luna 繁体词库下全量并入会把生僻字顶进 top-8。
    //
    // 这个 top 3 必须按**含 learner boost 的最终分**排，不能下推给
    // `dict.query(&syl, 3)` —— 后者截的是静态词频前 3，学过的常用字会在加 boost
    // 之前就被丢掉（"haoxiao" 里「好」在 hao 组排第 6，输入再多次也进不来）。
    // 与下方 85-86 行主路径是同一条不变式，此处曾违反。
    if merged.is_empty() && input.chars().count() > 1 {
        for syl in segment(input) {
            if syl.chars().count() < 2 {
                continue;
            }
            let mut syl_cands: Vec<Candidate> = dict
                .query(&syl, usize::MAX)
                .into_iter()
                .map(|e| {
                    // 精确性按**音节**判（该路径下"输入"就是音节），与主路径同一条规则。
                    let exact = e.pinyin_len == syl.len();
                    hanzi(e, exact)
                })
                .collect();
            syl_cands.sort_by(|a, b| b.score.cmp(&a.score).then(a.text.cmp(&b.text)));
            merged.extend(syl_cands.into_iter().take(3));
        }
    }
    for s in symbols.search(input) {
        // 单字符时仅并入 emoji：符号英文关键字前缀（如 comma→顿号）泄漏进
        // 拼音候选是噪音——真机 "c" 键唯一候选曾是顿号。emoji 保留作趣味反馈。
        if input.chars().count() == 1 && !s.emoji {
            continue;
        }
        // 符号不带拼音，故不参与「精确/扩展」分层（其分数本就只有学习权重）。
        merged.push(Candidate {
            text: s.text.clone(),
            kind: if s.emoji {
                CandidateKind::Emoji
            } else {
                CandidateKind::Symbol
            },
            score: rank_score(0, learner.freq_of(&s.text), boost),
        });
    }
    // 不能下推 limit 到 dict.query：学过的低静态词可反超截断线外的词，
    // 全量收集 + 排序是唯一正确方案（select 用有限 limit 只限 FFI 载荷）。
    merged.sort_by(|a, b| b.score.cmp(&a.score).then(a.text.cmp(&b.text)));
    let mut seen = std::collections::HashSet::new();
    merged.retain(|c| seen.insert(c.text.clone()));
    merged.truncate(limit);
    merged
}

#[cfg(test)]
mod tests {
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
        assert!(
            rank_and_pick(&d, &s, &l, "hao", Mode::English, DEFAULT_TOP_N, USER_BOOST).is_empty()
        );
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
        assert!(
            rank_and_pick(&d, &s, &l, "", Mode::Traditional, DEFAULT_TOP_N, USER_BOOST).is_empty()
        );
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
}
