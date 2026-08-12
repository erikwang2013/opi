use crate::composer::Mode;
use crate::dictionary::Dictionary;
use crate::learner::Learner;
use crate::symbols::SymbolEngine;

/// 用户词频权重：一次选词 ≈ 10 万次静态词频，保证学习迅速生效。
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

/// 排序分：静态词频 + 用户词频 × USER_BOOST。
pub fn rank_score(static_freq: u32, user_freq: u32) -> u64 {
    static_freq as u64 + user_freq as u64 * USER_BOOST
}

/// 合并词典候选与符号候选，排序、去重、截断。
pub fn rank_and_pick<D: Dictionary + ?Sized>(
    dict: &D,
    symbols: &SymbolEngine,
    learner: &Learner,
    input: &str,
    mode: Mode,
    limit: usize,
) -> Vec<Candidate> {
    if input.is_empty() || mode != Mode::Pinyin {
        return Vec::new();
    }
    let mut merged: Vec<Candidate> = dict
        .query(input, usize::MAX)
        .into_iter()
        .map(|e| Candidate {
            text: e.word.clone(),
            kind: CandidateKind::Hanzi,
            score: rank_score(e.freq, learner.freq_of(&e.word)),
        })
        .collect();
    for s in symbols.search(input) {
        merged.push(Candidate {
            text: s.text.clone(),
            kind: if s.emoji { CandidateKind::Emoji } else { CandidateKind::Symbol },
            score: rank_score(0, learner.freq_of(&s.text)),
        });
    }
    // 不能下推 limit 到 dict.query：USER_BOOST 可让低静态词反超截断线外的词，
    // 全量收集 + 排序是唯一正确方案。
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

    #[test]
    fn rank_score_boost_dominates() {
        assert!(rank_score(0, 1) > rank_score(5000, 0));
    }

    #[test]
    fn picks_dictionary_sorted_by_freq() {
        let d = test_dict();
        let s = SymbolEngine::builtin();
        let l = Learner::new(false);
        let got = rank_and_pick(&d, &s, &l, "hao", Mode::Pinyin, DEFAULT_TOP_N);
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].text, "好");
        assert_eq!(got[1].text, "号");
        assert_eq!(got[2].text, "豪");
        assert!(got.iter().all(|c| c.kind == CandidateKind::Hanzi));
    }

    #[test]
    fn empty_input_gives_empty() {
        let d = test_dict();
        let s = SymbolEngine::builtin();
        let l = Learner::new(false);
        assert!(rank_and_pick(&d, &s, &l, "", Mode::Pinyin, DEFAULT_TOP_N).is_empty());
    }

    #[test]
    fn non_pinyin_mode_gives_empty() {
        let d = test_dict();
        let s = SymbolEngine::builtin();
        let l = Learner::new(false);
        assert!(rank_and_pick(&d, &s, &l, "hao", Mode::English, DEFAULT_TOP_N).is_empty());
    }

    #[test]
    fn learner_boost_reorders() {
        let d = test_dict();
        let s = SymbolEngine::builtin();
        let mut l = Learner::new(true);
        l.record_selection("豪");
        let got = rank_and_pick(&d, &s, &l, "hao", Mode::Pinyin, DEFAULT_TOP_N);
        assert_eq!(got[0].text, "豪");
    }

    #[test]
    fn emoji_via_keyword_merge() {
        let d = test_dict();
        let s = SymbolEngine::builtin();
        let l = Learner::new(false);
        let got = rank_and_pick(&d, &s, &l, "xiao", Mode::Pinyin, DEFAULT_TOP_N);
        assert!(got.iter().any(|c| c.kind == CandidateKind::Emoji && c.text == "😄"));
        assert!(got[0].text == "笑" || got[0].text == "小" || got[0].text == "校");
    }

    #[test]
    fn limit_truncates() {
        let d = test_dict();
        let s = SymbolEngine::builtin();
        let l = Learner::new(false);
        let got = rank_and_pick(&d, &s, &l, "hao", Mode::Pinyin, 1);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].text, "好");
    }

    #[test]
    fn dedupes_by_text() {
        let d = test_dict();
        let s = SymbolEngine::builtin();
        let mut l = Learner::new(true);
        l.record_selection("好");
        let got = rank_and_pick(&d, &s, &l, "hao", Mode::Pinyin, DEFAULT_TOP_N);
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
        let got = rank_and_pick(&d, &s, &l, "hao", Mode::Pinyin, DEFAULT_TOP_N);
        assert_eq!(got.iter().filter(|c| c.text == "好").count(), 1);
        assert_eq!(got.len(), 3);
    }

    #[test]
    fn symbol_only_input() {
        let d = test_dict();
        let s = SymbolEngine::builtin();
        let l = Learner::new(false);
        let got = rank_and_pick(&d, &s, &l, "heart", Mode::Pinyin, DEFAULT_TOP_N);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].text, "♥");
        assert_eq!(got[0].kind, CandidateKind::Symbol);
    }
}
