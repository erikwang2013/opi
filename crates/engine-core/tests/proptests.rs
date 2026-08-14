use engine_core::candidates::rank_score;
use engine_core::composer::{Composer, MAX_BUFFER};
use engine_core::learner::Learner;
use engine_core::pinyin::segment;
use engine_core::trie::Trie;
use proptest::prelude::*;

proptest! {
    #[test]
    fn composer_buffer_matches_input(s in "[a-z']{0,30}") {
        let mut c = Composer::new();
        for ch in s.chars() {
            c.input_key(ch);
        }
        // MAX_BUFFER 上限：超限按键被忽略，buffer 等于输入截断
        let expected: String = s.chars().take(MAX_BUFFER).collect();
        prop_assert_eq!(&c.session().buffer, &expected);
    }

    #[test]
    fn pinyin_segment_joins_to_input(s in "[a-z']{1,20}") {
        // `'` 是硬分隔符，segment 输出不含它，期望值需过滤
        let expected: String = s.chars().filter(|c| *c != '\'').collect();
        let parts = segment(&s);
        let joined: String = parts.iter().flat_map(|p| p.chars()).collect();
        prop_assert_eq!(joined, expected);
    }

    #[test]
    fn trie_roundtrip(words in prop::collection::vec("[a-z]{1,6}", 1..20), freqs in prop::collection::vec(1u32..100_000, 1..20)) {
        let mut t = Trie::new();
        for (w, f) in words.iter().zip(freqs.iter()) {
            t.insert(w, w, *f);
        }
        let got = t.query_prefix(&words[0], usize::MAX);
        prop_assert!(!got.is_empty());
        prop_assert!(got.windows(2).all(|x| x[0].freq >= x[1].freq));
    }

    #[test]
    fn rank_score_monotonic(static_f: u32, user_f: u32) {
        prop_assert!(rank_score(static_f, user_f) >= static_f as u64);
        prop_assert!(rank_score(0, user_f) >= rank_score(0, 0));
    }

    #[test]
    fn learner_counts_match_selections(words in prop::collection::vec("[a-z]{1,8}", 1..10)) {
        let mut l = Learner::new(true);
        for w in &words {
            l.record_selection(w);
        }
        for w in &words {
            prop_assert_eq!(l.freq_of(w), words.iter().filter(|x| *x == w).count() as u32);
        }
    }
}
