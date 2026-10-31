// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 英文单词联想（数据 `data/raw/en_words.tsv`，CC-BY 3.0 —— 来源与署名见
//! `data/raw/LICENSES.md`；**不是牛津词典**，见 `scripts/gen_en_dict.py` 头注）。
//!
//! 行为契约（用户裁决 2026-10-18「英文模式下调用匹配」）：
//! - 英文模式每个字母进引擎缓冲，候选 = 以缓冲为前缀的词，按词频降序；
//! - 匹配大小写不敏感；显示用词库的**规范词形**（`China` / `café`），用户首字母
//!   大写时规范小写词形跟着首字母大写；
//! - 空格：有候选提首候选、无候选提原文（英文打原文的逃生口）；
//! - 数字选词走 `digit_select`（有候选才消费，无候选直传）；切模式清缓冲。
//!
//! 数据是入库的 pinned 产物，断言里的词形/词频顺序因此可以写死（换数据须同步改这里）。

use engine_core::candidates::CandidateKind;
use engine_core::dictionary::InMemoryDictionary;
use engine_core::keys::{KEY_BACK_SPACE, KEY_RETURN};
use engine_core::router::{KeyAction, KeyRouter, ShiftState};
use engine_core::scheme::Scheme;
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Mode};

/// 英文候选不查主词库（数据在 engine-core 内嵌），主库给空表即可。
fn engine() -> Engine {
    Engine::new(
        Box::new(InMemoryDictionary::new()),
        SymbolEngine::new(Vec::new(), Vec::new()),
        false,
    )
}

fn english_state() -> Engine {
    let mut e = engine();
    e.switch_mode(Mode::English);
    e
}

fn typed(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

/// 带拼音词条（`ni`→你、`he`→和）的引擎：中→英回归需要真拼音候选作对照。
fn pinyin_engine() -> Engine {
    let mut d = InMemoryDictionary::new();
    d.insert("ni", "你", 1000);
    d.insert("he", "和", 900);
    Engine::new(
        Box::new(d),
        SymbolEngine::new(Vec::new(), Vec::new()),
        false,
    )
}

fn router_type(r: &mut KeyRouter, s: &str) {
    for c in s.chars() {
        assert_eq!(r.key_event(c as u32, 0), KeyAction::EngineHandled);
    }
}

#[test]
fn prefix_candidates_are_frequency_ordered() {
    let mut e = english_state();
    typed(&mut e, "hel");
    let got = e.candidates(8);
    assert!(!got.is_empty(), "hel 应有前缀候选");
    assert_eq!(got[0].text, "help", "hel 的最高频前缀词");
    assert!(got.iter().all(|c| c.text.to_lowercase().starts_with("hel")));
    assert!(got.iter().all(|c| c.kind == CandidateKind::English));
    let scores: Vec<u64> = got.iter().map(|c| c.score).collect();
    let mut want = scores.clone();
    want.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(scores, want, "候选必须按词频降序");
}

/// 打完整词时整词第一：旧行为按纯词频排，派生词（abandoned 1795794 >
/// abandon 814642、protection > protect、kind > kin）会把用户已经打完的词压到第 2。
#[test]
fn exact_word_ranks_before_longer_prefix_words() {
    for (input, expected) in [
        ("abandon", "abandon"),
        ("immigrant", "immigrant"),
        ("protect", "protect"),
        ("kin", "kin"),
    ] {
        let mut e = english_state();
        typed(&mut e, input);
        assert_eq!(e.candidates(3)[0].text, expected, "{input}: 整词必须排第一");
    }
}

#[test]
fn limit_caps_candidates() {
    let mut e = english_state();
    typed(&mut e, "a");
    assert_eq!(e.candidates(3).len(), 3);
    assert!(
        e.candidates(usize::MAX).len() > 8,
        "宿主取大量候选的路径不得被内部截断"
    );
}

#[test]
fn single_letter_puts_input_first_then_words() {
    let mut e = english_state();
    typed(&mut e, "h");
    let got = e.candidates(8);
    assert_eq!(got[0].text, "h", "单字母缓冲的首项必须是输入原文");
    assert_eq!(got.len(), 8, "原文占 1 个名额，其余由词库补满");
    assert!(got.iter().all(|c| c.kind == CandidateKind::English));
    assert!(
        got[1..]
            .iter()
            .all(|c| c.text.to_lowercase().starts_with('h')),
        "原文之后仍是既有的前缀匹配词：{got:?}"
    );
}

#[test]
fn single_letter_preserves_case() {
    let mut e = english_state();
    typed(&mut e, "H");
    assert_eq!(e.candidates(4)[0].text, "H", "原文候选保留用户输入的大小写");
}

#[test]
fn single_letter_dedupes_dictionary_entry() {
    // a / I 本身就是词库词条：原文候选与词条同形时只留一条（大小写不敏感）。
    for input in ["a", "i", "I"] {
        let mut e = english_state();
        typed(&mut e, input);
        let got = e.candidates(usize::MAX);
        let same = got
            .iter()
            .filter(|c| c.text.eq_ignore_ascii_case(input))
            .count();
        assert_eq!(same, 1, "{input}: 同形字母只许一条，实得 {got:?}");
        assert_eq!(got[0].text, input, "{input}: 原文仍占首位");
    }
}

#[test]
fn single_letter_limit_counts_the_raw_letter() {
    let mut e = english_state();
    typed(&mut e, "a");
    let got = e.candidates(1);
    assert_eq!(got.len(), 1, "limit=1 时只留原文");
    assert_eq!(got[0].text, "a");

    let got = e.candidates(3);
    assert_eq!(got.len(), 3, "原文占 1 个名额 + 2 个词");
    assert_eq!(got[0].text, "a");
}

#[test]
fn multi_letter_buffer_has_no_raw_prefix_candidate() {
    // 多字母保持现状：候选全是词库词，不前置原文缓冲。
    let mut e = english_state();
    typed(&mut e, "hel");
    assert!(
        e.candidates(8).iter().all(|c| c.text != "hel"),
        "多字母缓冲不前置原文"
    );
}

#[test]
fn space_on_single_letter_commits_the_letter_itself() {
    let mut e = english_state();
    typed(&mut e, "q");
    assert_eq!(e.input_space(), "q", "首候选是原文 ⇒ 空格上屏原文");
}

#[test]
fn matching_is_case_insensitive_and_shift_capitalizes_display() {
    // 首字母大写（shift 语义）⇒ 规范小写词形首字母跟着大写；匹配按小写做。
    let mut e = english_state();
    typed(&mut e, "Hel");
    assert_eq!(e.candidates(1)[0].text, "Help");

    let mut e = english_state();
    typed(&mut e, "HEL"); // 锁定大写：仍只保留首字母大写（词库词形是小写）
    assert_eq!(e.candidates(1)[0].text, "Help");
}

#[test]
fn canonical_word_forms_are_preserved() {
    // 词库第一列是规范词形、第二列才是小写查询键：专名与带音调字符不丢。
    let mut e = english_state();
    typed(&mut e, "china");
    assert_eq!(e.candidates(1)[0].text, "China");

    let mut e = english_state();
    typed(&mut e, "cafe");
    assert_eq!(e.candidates(1)[0].text, "café");
}

#[test]
fn space_commits_first_candidate_and_clears_buffer() {
    let mut e = english_state();
    typed(&mut e, "hel");
    assert_eq!(e.input_space(), "help");
    assert_eq!(e.buffer(), "");
    assert!(e.candidates(8).is_empty(), "候选随缓冲清空");
}

#[test]
fn space_without_match_commits_raw_buffer() {
    let mut e = english_state();
    typed(&mut e, "zzzq");
    assert!(e.candidates(8).is_empty());
    assert_eq!(e.input_space(), "zzzq", "无候选 = 英文打原文的逃生口");
    assert_eq!(e.buffer(), "");
}

#[test]
fn select_commits_candidate_and_clears_buffer() {
    let mut e = english_state();
    typed(&mut e, "hel");
    let top = e.candidates(1)[0].text.clone();
    assert_eq!(e.select(0), top);
    assert_eq!(e.buffer(), "");
}

#[test]
fn backspace_updates_candidates() {
    let mut e = english_state();
    typed(&mut e, "helx");
    assert!(e.candidates(8).is_empty(), "helx 无前缀词");
    e.backspace();
    assert_eq!(e.candidates(1)[0].text, "help", "退格后候选随之更新");
}

#[test]
fn switching_mode_clears_english_buffer() {
    let mut e = english_state();
    typed(&mut e, "hel");
    assert!(!e.candidates(8).is_empty(), "前置：有候选");
    e.switch_mode(Mode::Pinyin);
    assert_eq!(e.buffer(), "");
    assert!(e.candidates(8).is_empty(), "切模式清缓冲，候选随之清空");
}

#[test]
fn fullwidth_applies_to_candidate_commit() {
    let mut e = english_state();
    e.toggle_fullwidth();
    typed(&mut e, "hel");
    assert_eq!(e.input_space(), "ｈｅｌｐ", "候选提交仍走 fullwidthize");

    // 数字选词/面板点击走的 `select` 也是候选提交，与空格同一条全角规则。
    let mut e = english_state();
    e.toggle_fullwidth();
    typed(&mut e, "hel");
    assert_eq!(e.select(0), "ｈｅｌｐ", "select 提交同样宽化");
}

#[test]
fn digit_selects_candidate_in_english_mode() {
    let mut r = KeyRouter::new(engine());
    r.switch_mode(Mode::English);
    for c in ['h', 'e', 'l'] {
        assert_eq!(r.key_event(c as u32, 0), KeyAction::EngineHandled);
    }
    assert_eq!(r.key_event('1' as u32, 0), KeyAction::Input("help".into()));
    assert_eq!(r.buffer(), "");
}

/// 粗略成本证据（`cargo test --release -- --nocapture` 打印实际耗时）：
/// 单字母前缀是全表命中最多的一档（"a" 前缀数百条），1000 次取候选仍应在毫秒级 ——
/// 索引只建一次、排序只对命中项做，不是每键重排 1 万条。
#[test]
fn single_letter_lookup_cost_is_bounded() {
    let mut e = english_state();
    typed(&mut e, "a");
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        assert!(!e.candidates(8).is_empty());
    }
    let elapsed = start.elapsed();
    eprintln!("1000 × candidates(8) for prefix 'a': {elapsed:?}");
    assert!(
        elapsed.as_secs() < 5,
        "单键候选成本异常（1000 次 {elapsed:?}）"
    );
}

#[test]
fn fullwidth_applies_to_digit_selection() {
    let mut r = KeyRouter::new(engine());
    r.switch_mode(Mode::English);
    r.engine_mut().toggle_fullwidth();
    for c in ['h', 'e', 'l'] {
        r.key_event(c as u32, 0);
    }
    assert_eq!(
        r.key_event('1' as u32, 0),
        KeyAction::Input("ｈｅｌｐ".into()),
        "数字选词与空格同一条全角规则"
    );
}

/// 2026-10-09 用户裁决回归：拼音打 `ni`（缓冲非空）→ 切英文 → 按 `a`。
/// 旧行为是字母**直传**成英文、缓冲 `ni` 原样保留（切回中文继续）；不能把 `a`
/// 追加成 `nia`（英文打不出来，保留的拼音也被毁掉）。
#[test]
fn carried_pinyin_buffer_direct_passes_letters_and_survives() {
    let mut r = KeyRouter::new(pinyin_engine());
    router_type(&mut r, "ni");
    assert_eq!(r.buffer(), "ni");
    r.switch_language(Mode::English, Scheme::Full);
    assert!(
        r.engine_mut().buffer_is_carried(),
        "切英文保留的拼音必须带「携带」标"
    );
    assert!(r.candidates().is_empty(), "携带的拼音不进英文前缀匹配");
    assert_eq!(
        r.key_event('a' as u32, 0),
        KeyAction::Input("a".into()),
        "携带缓冲下字母直传"
    );
    assert_eq!(r.buffer(), "ni", "保留的拼音不被追加成 nia");
}

/// 直传路径的 ⇧/全角语义照旧（直传绕过引擎，规则见 `Engine::fullwidthize`）。
#[test]
fn carried_buffer_direct_pass_keeps_fullwidth() {
    let mut r = KeyRouter::new(pinyin_engine());
    router_type(&mut r, "ni");
    r.switch_language(Mode::English, Scheme::Full);
    r.engine_mut().toggle_fullwidth();
    assert_eq!(r.key_event('a' as u32, 0), KeyAction::Input("ａ".into()));
    assert_eq!(r.buffer(), "ni");
}

/// 切到 English 时前端 ⇧ 三态必须重推给引擎：`switch_mode`/`switch_language` 会清
/// 引擎 shift（跨模式粘滞态的既有理由），而前端 SINGLE/LOCK 还在 —— 不重推则首个
/// 字母小写、前端仍高亮（四端同一条语义）。
#[test]
fn switch_mode_to_english_reapplies_single_shift() {
    let mut r = KeyRouter::new(engine());
    r.shift_tap(); // SINGLE
    r.switch_mode(Mode::English);
    assert_eq!(r.key_event('a' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(r.buffer(), "A", "SINGLE 重推给引擎：首个字母大写");
    assert_eq!(r.shift_state(), ShiftState::Off, "single 用掉即复位");
}

#[test]
fn switch_mode_to_english_reapplies_lock_shift() {
    let mut r = KeyRouter::new(engine());
    r.shift_long_press(); // LOCK
    r.switch_mode(Mode::English);
    assert_eq!(r.key_event('a' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(r.key_event('b' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(r.buffer(), "AB", "LOCK 重推且持续");
    assert_eq!(r.shift_state(), ShiftState::Lock);
}

#[test]
fn switch_language_to_english_reapplies_single_shift() {
    let mut r = KeyRouter::new(engine());
    r.shift_tap();
    r.switch_language(Mode::English, Scheme::Full);
    assert_eq!(r.key_event('a' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(r.buffer(), "A");
    assert_eq!(r.shift_state(), ShiftState::Off);
}

/// 携带拼音缓冲：字母直传（不经引擎 shift），前端大写照旧，single 用掉即复位。
#[test]
fn carried_buffer_switch_language_keeps_shift_uppercase() {
    let mut r = KeyRouter::new(pinyin_engine());
    router_type(&mut r, "ni");
    r.shift_tap(); // SINGLE
    r.switch_language(Mode::English, Scheme::Full);
    assert_eq!(r.key_event('a' as u32, 0), KeyAction::Input("A".into()));
    assert_eq!(r.buffer(), "ni", "携带缓冲原样保留");
    assert_eq!(r.shift_state(), ShiftState::Off);
}

/// 英文**自己的**缓冲照常匹配（与携带缓冲的分界）。
#[test]
fn english_own_buffer_matches_normally() {
    let mut r = KeyRouter::new(engine());
    r.switch_mode(Mode::English);
    router_type(&mut r, "hel");
    assert!(!r.engine_mut().buffer_is_carried(), "英文自己的缓冲不带标");
    assert_eq!(r.candidates()[0], "help", "hel 前缀最高频词");
    assert_eq!(r.key_event('p' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(r.buffer(), "help", "英文自己的缓冲继续进引擎匹配");
}

/// 切回中文：英文缓冲原样保留，但**不拿英文词当拼音匹配**（"he" ≠ 和）；
/// 空格按既有「无候选」语义提交原文。再切回英文，缓冲仍是英文自己的（候选恢复）。
#[test]
fn switching_back_to_chinese_does_not_treat_english_as_pinyin() {
    let mut e = pinyin_engine();
    e.switch_language(Mode::English, Scheme::Full);
    typed(&mut e, "he");
    assert!(!e.candidates(8).is_empty(), "英文缓冲先有候选");

    e.switch_language(Mode::Pinyin, Scheme::Full);
    assert_eq!(e.buffer(), "he", "英→中保留缓冲");
    assert!(
        e.candidates(8).is_empty(),
        "英文词不得当拼音匹配（he ≠ 和）"
    );
    assert_eq!(e.input_space(), "he", "无候选 → 既有语义提交原文");

    // 中→英→中 回到来处：携带标清掉，拼音照常匹配。
    let mut e = pinyin_engine();
    typed(&mut e, "ni");
    e.switch_language(Mode::English, Scheme::Full);
    assert!(e.candidates(8).is_empty(), "携带缓冲不出英文候选");
    e.switch_language(Mode::Pinyin, Scheme::Full);
    assert!(!e.buffer_is_carried(), "回到来处清携带标");
    assert_eq!(e.candidates(8)[0].text, "你", "拼音缓冲恢复自己的匹配");
}

/// 携带缓冲下的编辑键逐条对齐旧直传语义：空格提交原文、退格改缓冲、
/// 回车无候选不动缓冲、数字直传（不进选词）。
#[test]
fn carried_buffer_edit_keys_keep_old_semantics() {
    let mut r = KeyRouter::new(pinyin_engine());
    router_type(&mut r, "ni");
    r.switch_language(Mode::English, Scheme::Full);

    assert_eq!(r.key_event('1' as u32, 0), KeyAction::Input("1".into()));
    assert_eq!(r.buffer(), "ni", "携带缓冲下数字直传，不进选词");

    assert_eq!(r.key_event(KEY_BACK_SPACE, 0), KeyAction::EngineHandled);
    assert_eq!(r.buffer(), "n", "退格删的是携带缓冲本身");

    assert_eq!(r.key_event(KEY_RETURN, 0), KeyAction::EngineHandled);
    assert_eq!(r.buffer(), "n", "无候选的回车不提交、不清缓冲");

    assert_eq!(r.key_event(' ' as u32, 0), KeyAction::Input("n".into()));
    assert_eq!(r.buffer(), "", "空格提交原文并清缓冲");
}

#[test]
fn digit_passes_through_when_no_english_candidate() {
    let mut r = KeyRouter::new(engine());
    r.switch_mode(Mode::English);
    for c in ['z', 'z', 'z', 'q'] {
        r.key_event(c as u32, 0);
    }
    assert_eq!(
        r.key_event('1' as u32, 0),
        KeyAction::PassThrough,
        "无候选时数字不得被选词吃掉（直传）"
    );
    assert_eq!(r.buffer(), "zzzq", "直传不动缓冲");
}
