// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 对抗性输入与不变量（engine-core 纯逻辑层）。
//!
//! 断言都要求**改坏实现就会红**（见 memory: green-tests-only-prove-what-is-asserted）：
//! - `syllable_prefix_*`：手写二分 vs 线性扫描，覆盖全 410 音节的全前缀 —— 二分实现
//!   的比较子不是真比较（命中区返回 `Equal`），只有穷举能证明它的单调性假设成立。
//! - `segment_*`：切分必须无损、每段必须是最长匹配。
//! - `composer_*`：5 模式 × 2 ⇧ × 14 键的**独立期望表**（按文档语义重写，不抄实现），
//!   外加「缓冲内容 == 模型」的逐键对账。
//! - `rank_and_pick_*`：精确匹配必须整体压过前缀扩展（`exact_bonus = 0` 即红）。
//! - `engine_*`：缓冲空 ⇒ 候选必空；`select` 的全局下标 == 候选列表下标。

use engine_core::Engine;
use engine_core::candidates::{Candidate, USER_BOOST, rank_and_pick, rank_score};
use engine_core::composer::{Composer, KeyEffect, MAX_BUFFER, Mode};
use engine_core::dictionary::{Dictionary, InMemoryDictionary};
use engine_core::learner::Learner;
use engine_core::pinyin::{SYLLABLES, is_syllable_prefix, segment};
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

// ---------- 音节表与切分 ----------

/// 线性扫描是**定义**，二分是**实现**。两者必须对全 410 音节的全前缀（含空前缀）一致。
/// 比较子对命中区返回 `Equal`（而非真比较），其单调性是二分正确性的前提 —— 只有穷举能证。
#[test]
fn syllable_prefix_agrees_with_linear_scan() {
    let mut checked = 0usize;
    for &syl in SYLLABLES {
        let chars: Vec<char> = syl.chars().collect();
        for n in 0..=chars.len() {
            let p: String = chars[..n].iter().collect();
            let want = SYLLABLES.iter().any(|s| s.starts_with(&p));
            assert_eq!(
                is_syllable_prefix(&p),
                want,
                "前缀 {p:?}（来自 {syl}）与线性扫描不一致"
            );
            checked += 1;
        }
    }
    assert!(checked > 800, "覆盖不足：只查了 {checked} 个前缀");
}

proptest! {
    /// 任意串（含大写、数字、非 ASCII、emoji）都必须与线性扫描一致，且 == 自身（自反）。
    #[test]
    fn syllable_prefix_matches_linear_on_arbitrary_strings(s in ".{0,8}") {
        let want = SYLLABLES.iter().any(|syl| syl.starts_with(&s));
        prop_assert_eq!(is_syllable_prefix(&s), want, "输入 {:?}", s);
        // 命中一个音节就必然命中它自己（自反性）
        if SYLLABLES.contains(&s.as_str()) {
            prop_assert!(is_syllable_prefix(&s));
        }
    }

    /// 切分无损：拼接各段 == 输入（去掉硬分隔符 `'`），每段非空。
    #[test]
    fn segment_is_lossless(s in ".{0,24}") {
        let parts = segment(&s);
        let joined: String = parts.iter().flat_map(|p| p.chars()).collect();
        let want: String = s.chars().filter(|c| *c != '\'').collect();
        prop_assert_eq!(&joined, &want);
        prop_assert!(parts.iter().all(|p| !p.is_empty()));
    }

    /// 每段必须是「最长匹配」：该段要么是完整音节，要么以**单字符**结尾（无法再延长的证据）。
    /// 这条是贪婪实现的核心性质，只看「拼接还原」是抓不住的（把每段退化成单字符也还原）。
    #[test]
    fn segment_parts_are_greedy(s in "[a-z']{1,16}") {
        let chars: Vec<char> = s.chars().collect();
        let parts = segment(&s);
        let mut i = 0usize;
        for p in &parts {
            // 跳过输入里的硬分隔符
            while chars[i] == '\'' { i += 1; }
            let n = p.chars().count();
            prop_assert!(chars[i..i + n].iter().collect::<String>() == *p);
            if !is_syllable_prefix(p) {
                prop_assert_eq!(n, 1, "段 {:?} 不是音节前缀又不是单字符", p);
            } else {
                // 再吃一个字符就不再是任何音节的前缀（否则贪婪没吃满）
                let mut longer: String = p.clone();
                if i + n < chars.len() && chars[i + n] != '\'' {
                    longer.push(chars[i + n]);
                    prop_assert!(
                        !is_syllable_prefix(&longer),
                        "段 {:?} 可延长为 {:?} 却没延长", p, longer
                    );
                }
            }
            i += n;
        }
    }
}

// ---------- Composer：模式 × ⇧ × 键的独立期望表 ----------

const MODES: [Mode; 5] = [
    Mode::Pinyin,
    Mode::Traditional,
    Mode::English,
    Mode::Number,
    Mode::Symbol,
];

const CHARS: [char; 14] = [
    'a', 'z', 'A', 'Z', '1', '0', '\'', ' ', '.', ',', 'v', 'ü', '中', '😄',
];

/// **独立期望表**（按 composer.rs 的文档语义重写，不看实现）：
/// - Pinyin/Traditional：小写字母与 `'` 原样，大写降小写，其余忽略
/// - English：字母按 ⇧ 决定大小写（无 ⇧ 时保留原码点），其余忽略
/// - Number：数字，其余忽略
/// - Symbol：**字母数字**原样入缓冲（是符号搜索的**关键字**，如 dun/ballot），
///   其余忽略。空格不在其中：它是提交键（`Engine::input_key` 先拦）。
///   2026-09-27 标点表把这里从 `is_ascii_graphic` 收窄到 `is_ascii_alphanumeric`：
///   生产符号表的关键字一律 `^[a-z0-9]+$`，标点收进缓冲只会得到搜不出候选的死缓冲；
///   标点另有出路（标点表 → 原样交回调用方），边界由 tests/punctuation.rs 的
///   every_symbol_keyword_is_alnum 钉住。
fn expected_push(mode: Mode, shift: bool, ch: char) -> Option<char> {
    match mode {
        Mode::Pinyin | Mode::Traditional => {
            if ch.is_ascii_lowercase() || ch == '\'' {
                Some(ch)
            } else if ch.is_ascii_uppercase() {
                Some(ch.to_ascii_lowercase())
            } else {
                None
            }
        }
        Mode::English => {
            if ch.is_ascii_alphabetic() {
                Some(if shift { ch.to_ascii_uppercase() } else { ch })
            } else {
                None
            }
        }
        Mode::Number => ch.is_ascii_digit().then_some(ch),
        Mode::Symbol => ch.is_ascii_alphanumeric().then_some(ch),
    }
}

#[test]
fn composer_matrix_matches_spec_table() {
    for mode in MODES {
        for shift in [false, true] {
            for ch in CHARS {
                let mut c = Composer::new();
                c.switch_mode(mode);
                c.set_shift(shift);
                let (eff, s) = c.input_key(ch);
                match expected_push(mode, shift, ch) {
                    Some(want) => {
                        assert_eq!(
                            eff,
                            KeyEffect::Updated,
                            "{mode:?}/⇧{shift}/键 {ch:?} 应被接受"
                        );
                        assert_eq!(s.buffer, want.to_string(), "{mode:?}/⇧{shift}/键 {ch:?}");
                    }
                    None => {
                        assert_eq!(
                            eff,
                            KeyEffect::Ignored,
                            "{mode:?}/⇧{shift}/键 {ch:?} 应被忽略"
                        );
                        assert_eq!(s.buffer, "", "{mode:?}/⇧{shift}/键 {ch:?} 不得进缓冲");
                    }
                }
            }
        }
    }
}

#[test]
fn composer_accepts_exactly_max_buffer_chars() {
    for mode in MODES {
        // Symbol 与拼音同一条上限（关键字缓冲同样不得无限增长）。种子按键：
        // 数字模式的唯一合法输入是数字，其余模式用字母；Symbol 用字母（关键字是词）。
        for shift in [false, true] {
            let seed = if mode == Mode::Number { '1' } else { 'a' };
            for pre in [0, 1, MAX_BUFFER - 1, MAX_BUFFER] {
                let mut c = Composer::new();
                c.switch_mode(mode);
                c.set_shift(shift);
                for _ in 0..pre {
                    c.input_key(seed);
                }
                assert_eq!(c.session().buffer.chars().count(), pre, "前置铺设失败");
                let (eff, s) = c.input_key(seed);
                let want = pre < MAX_BUFFER;
                assert_eq!(
                    eff == KeyEffect::Updated,
                    want,
                    "{mode:?} 前缀 {pre} 字符：上限应为 {MAX_BUFFER}"
                );
                assert_eq!(s.buffer.chars().count(), if want { pre + 1 } else { pre });
            }
        }
    }
}

#[derive(Debug, Clone)]
enum Op {
    Key(char),
    Back,
    Clear,
}

/// 模型对账：缓冲内容必须**逐键等于**按期望表推出的字符串。
/// 只看「长度没超」是抓不住大小写/降级错误的（`to_ascii_lowercase` 删掉也绿）。
#[test]
fn composer_matches_model_over_random_ops() {
    let ops = prop::collection::vec(
        prop_oneof![
            prop::sample::select(CHARS.to_vec()).prop_map(Op::Key),
            Just(Op::Back),
            Just(Op::Clear),
        ],
        0..40,
    );
    proptest!(|(
        ops in ops,
        mode_idx in 0usize..5,
        shift in any::<bool>(),
        // 中途换模式：模式切换必须清缓冲与 ⇧（否则 ⇧ 粘滞态跨模式残留）
        switch_at in prop::option::of(0usize..40),
        switch_to in 0usize..5,
    )| {
        let mode = MODES[mode_idx];
        let mut c = Composer::new();
        c.switch_mode(mode);
        c.set_shift(shift);
        let mut model = String::new();
        let mut shift_state = shift;
        let mut cur = mode;
        for (i, op) in ops.iter().enumerate() {
            if Some(i) == switch_at {
                cur = MODES[switch_to];
                c.switch_mode(cur);
                model.clear();
                shift_state = false; // switch_mode 清 shift
            }
            match op {
                Op::Key(ch) => {
                    let want = expected_push(cur, shift_state, *ch);
                    let accepted = want.is_some() && model.chars().count() < MAX_BUFFER;
                    let (eff, s) = c.input_key(*ch);
                    if accepted {
                        model.push(want.unwrap());
                    }
                    prop_assert_eq!(eff == KeyEffect::Updated, accepted,
                        "模式 {:?} ⇧{} 键 {:?}", cur, shift_state, ch);
                    prop_assert_eq!(&s.buffer, &model, "模式 {:?} ⇧{} 键 {:?}", cur, shift_state, ch);
                }
                Op::Back => {
                    model.pop();
                    let s = c.backspace();
                    prop_assert_eq!(&s.buffer, &model);
                }
                Op::Clear => {
                    model.clear();
                    let s = c.clear();
                    prop_assert_eq!(&s.buffer, &model);
                }
            }
            // 缓冲是 UI 的 preedit 来源，任何路径都不得突破上限
            prop_assert!(c.session().buffer.chars().count() <= MAX_BUFFER);
        }
    });
}

// ---------- Engine 门面 ----------

fn test_engine(learner: bool) -> Engine {
    let mut d = InMemoryDictionary::new();
    for i in 0..20 {
        d.insert("hao", &format!("词{i:02}"), 5000 - i as u32);
    }
    d.insert("xiao", "笑", 3000);
    Engine::new(Box::new(d), no_symbols(), learner)
}

#[test]
fn engine_never_exceeds_limits_and_clears_buffer_on_commit() {
    let keys = prop::sample::select(vec!['h', 'a', 'o', 'x', 'i', 'n', '\'', '1', '中', ' ']);
    proptest!(|(keys in prop::collection::vec(keys, 0..40))| {
        let mut e = test_engine(true);
        for k in &keys {
            let out = e.input_key(*k);
            prop_assert!(e.buffer().chars().count() <= MAX_BUFFER);
            prop_assert!(e.candidates(8).len() <= 8);
            // 缓冲空 ⇒ 候选必空（拼音/繁体模式下不该凭空给候选）
            if e.buffer().is_empty() {
                prop_assert!(e.candidates(8).is_empty(), "空缓冲仍有候选");
            }
            // 非空格键永不提交。`'` 除外：缓冲空时它是标点表的引号（`‘’`），
            // 缓冲非空时才是音节分隔符（进缓冲、不提交）—— 见 tests/punctuation.rs。
            if *k != ' ' && *k != '\'' {
                prop_assert!(out.is_empty(), "键 {:?} 不应提交 {:?}", k, out);
            }
            // 提交过就必须清空缓冲（下一次输入不得拼接到旧串上）
            if !out.is_empty() {
                prop_assert!(e.buffer().is_empty());
            }
        }
    });
}

/// `select` 的下标必须是**全量候选列表**的下标：逐项与 `candidates(512)` 对齐。
/// 越界返回空串且不得动缓冲（否则越界的数字键会把拼音吃掉）。
#[test]
fn select_index_aligns_with_candidate_list() {
    let typed = ['h', 'a', 'o'];
    let texts: Vec<String> = {
        let mut e = test_engine(true);
        for c in typed {
            e.input_key(c);
        }
        cand_texts(&e.candidates(512))
    };
    assert!(
        texts.len() >= 20,
        "前置：应有 20 条候选，实际 {}",
        texts.len()
    );
    for (i, want) in texts.iter().enumerate() {
        let mut e = test_engine(true);
        for c in typed {
            e.input_key(c);
        }
        assert_eq!(&e.select(i), want, "第 {i} 项选错");
        assert_eq!(e.buffer(), "", "选中后必须清缓冲");
    }
    let mut e = test_engine(true);
    for c in typed {
        e.input_key(c);
    }
    let n = texts.len();
    assert_eq!(e.select(n), "", "越界应返回空串");
    assert_eq!(e.buffer(), "hao", "越界不得动缓冲");
}

/// 分数公式与 boost 缩放的契约：`score = static + user × boost`，且
/// `boost = max(USER_BOOST, max_freq × 2)` 让一次选词（1×boost）压过任意静态词。
#[test]
fn user_boost_scales_with_dictionary_max_freq() {
    assert_eq!(rank_score(7, 3, 100), 7 + 300);
    // 一次选词（1×boost）必须压过**任意**静态词频，含 u32::MAX ——
    // 这正是 boost 要按词典 max_freq × 2 缩放的理由：固定 USER_BOOST 下
    // rank_score(0, 1, USER_BOOST) = 10 万，输给 freq = u32::MAX 的静态词。
    let scaled = USER_BOOST.max(u32::MAX as u64 * 2);
    assert!(rank_score(0, 1, scaled) > rank_score(u32::MAX, 0, scaled));

    // 词库最大词频远超 USER_BOOST 时，boost 必须跟着涨，否则学过的低频词永远翻不了身
    let mut d = InMemoryDictionary::new();
    d.insert("hao", "好", 10);
    d.insert("hao", "号", 4_000_000_000);
    let mut e = Engine::new(Box::new(d), no_symbols(), true);
    for c in "hao".chars() {
        e.input_key(c);
    }
    let idx = e.candidates(8).iter().position(|c| c.text == "好").unwrap();
    assert_eq!(e.select(idx), "好");
    for c in "hao".chars() {
        e.input_key(c);
    }
    assert_eq!(
        e.candidates(8)[0].text,
        "好",
        "选过一次的词必须压过 40 亿静态词频（boost 未随 max_freq 缩放）"
    );
}
