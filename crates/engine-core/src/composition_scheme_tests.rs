// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `composition_scheme` 的单测（2026-10-27 从源文件拆出：源文件原本 457 行，
//! 9/14 键组合改动后 527 行，撞 500 行门禁）。与 `candidates_tests.rs` /
//! `learner_tests.rs` 同一套 `#[path]` 先例，语义一字未动。

use super::*;
use crate::dictionary::InMemoryDictionary;
use crate::trie::Entry;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn dict() -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    // 两个报障词：`厌蠢 + 症`、`样样 + 通`（整词不是键）。
    d.insert("yanchun", "厌蠢", 700);
    d.insert("zheng", "症", 850);
    d.insert("zheng", "正", 950);
    d.insert("yangyang", "样样", 700);
    d.insert("tong", "通", 850);
    d.insert("tong", "同", 950);
    // 负控制：只有单音节键（`ma`）。
    d.insert("ma", "妈", 900);
    // 负控制：只有前缀扩展键（`zhengren` 不是 `zheng`）。
    d.insert("zhengren", "症人", 990);
    d
}

fn hits(input: &str) -> Vec<Hit> {
    candidates(&dict(), &Learner::new(false), 1_800, Scheme::Nine, input)
}

#[test]
fn user_reported_words_compose_in_digit_space() {
    let got = hits("926248694364"); // yanchunzheng
    assert_eq!(got[0].text, "厌蠢症", "PINNED 应置顶: {got:?}");
    assert_eq!(got[0].pinyin, "yanchunzheng", "整串拼音读数");
    assert!(got.iter().any(|h| h.text == "厌蠢症"));
    let got = hits("926492648664"); // yangyangtong
    assert_eq!(got[0].text, "样样通", "PINNED 应置顶: {got:?}");
    assert_eq!(got[0].pinyin, "yangyangtong");
}

/// 全是单音节段 ⇒ 不拼（与全键盘的负控制同一条判据）。
#[test]
fn all_single_syllable_chunks_are_not_composed() {
    // `ma` × 2：只有 2 个音节（总音节 ≥3 不成立）。
    assert!(hits("6262").is_empty(), "两个单音节段不该组合");
    // `ma` × 3：总音节够，但没有多音节条目。
    assert!(hits("626262").is_empty(), "单音节段拼接是噪音");
}

/// 段必须是**等长精确键**；只有以它开头的更长键不算。
#[test]
fn chunk_lookup_is_exact_not_prefix() {
    let mut d = InMemoryDictionary::new();
    d.insert("yanchun", "厌蠢", 700);
    d.insert("zhengren", "症人", 990); // 只有 `zheng` 的前缀扩展
    let got = candidates(
        &d,
        &Learner::new(false),
        1_800,
        Scheme::Nine,
        "926248694364",
    );
    assert!(
        !got.iter()
            .any(|h| h.text.starts_with("厌蠢症") || h.text.starts_with("厌蠢正")),
        "`94364` 不该拿 `zhengren` 当精确键: {got:?}"
    );
}

/// 只有 9 键走这条路；非法按键（`0`/`1`/字母/`'`）一律拒绝。
#[test]
fn only_nine_key_digits_are_accepted() {
    assert!(
        candidates(
            &dict(),
            &Learner::new(false),
            1_800,
            Scheme::Full,
            "926248694364"
        )
        .is_empty()
    );
    for input in ["926248694360", "92624869436a", "926'2486"] {
        assert!(hits(input).is_empty(), "非法输入 {input} 不该出组合");
    }
}

/// 同一输入两次结果逐条相同（排序/去重无隐藏状态）。
#[test]
fn results_are_deterministic() {
    assert_eq!(hits("926248694364"), hits("926248694364"));
}

/// 查询预算：长输入最多查 `MAX_QUERIES` 次；一条切法都没凑出时整档放弃。
/// （2026-10-27 起：**凑出了完整切法就保留**，预算只限制枚举深度。）
#[test]
fn query_budget_caps_every_call() {
    struct Counted {
        inner: InMemoryDictionary,
        calls: Arc<AtomicUsize>,
    }
    impl Dictionary for Counted {
        fn query(&self, pinyin: &str, limit: usize) -> Vec<Entry> {
            self.inner.query(pinyin, limit)
        }
        fn scheme_entries(
            &self,
            input: &str,
            scheme: Scheme,
            pin: Option<&str>,
            n: usize,
        ) -> Vec<(Entry, String)> {
            let _ = (scheme, pin, n);
            self.calls.fetch_add(1, Ordering::Relaxed);
            // 假词库：2..=7 键的**任何**段都是有效精确键（拼音都 ≤2 音节）。
            // 8 键以上无命中 ⇒ 4 段 × 最多 7 键的完整切法不存在，整档无结果。
            let pinyin = match input.len() {
                2 => "ni",
                3 => "zhe",
                4 => "xian",
                5 => "zhong",
                6 => "zhuang",
                7 => "zhongni",
                _ => return Vec::new(),
            };
            vec![(
                Entry {
                    word: format!("w{}", input.len()),
                    freq: 1_000,
                    pinyin_len: input.len(),
                },
                pinyin.to_string(),
            )]
        }
        fn len(&self) -> usize {
            self.inner.len()
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = Counted {
        inner: dict(),
        calls: calls.clone(),
    };
    // 32 键、按键不重复（缓存按**段内容**去重，重复串只有 ~31 个不同段，
    // 撞不到预算）：假词库让每个 2..=7 键的段都命中 ⇒ 枚举在每个前缀上都
    // 递归，查询数远超 MAX_QUERIES；但末段（≥11 键）永不命中 ⇒ 一条完整
    // 切法都没有，整档返回空。
    let got = candidates(
        &counted,
        &Learner::new(false),
        1_800,
        Scheme::Nine,
        "27493856283749562739485628374956",
    );
    assert!(got.is_empty(), "一条切法都没凑出时整档放弃: {got:?}");
    assert_eq!(
        calls.load(Ordering::Relaxed),
        MAX_QUERIES,
        "查询次数必须封顶"
    );
}
