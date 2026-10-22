// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **多字词组合识别**（2026-10-23）的引擎级门禁：整词不在词库、但由词库里的几段
//! 精确键拼成时，候选要给出组合词（`yanchunzheng` → 厌蠢 + 症）。
//!
//! 钉住的判据（实现见 `composition.rs` 头注）：
//! 1. **整串全覆盖**：只拼连续覆盖整个输入的段，不许跳字；每段必须是**精确**词库键
//!    （前缀扩展不算）；
//! 2. **不是噪音**：全是单音节段的拼接不出组合（「随便几个同音字凑一串」）；
//! 3. **不抢既有匹配**：整串有精确命中时，管线逐字节不变（组合层不动手）；
//! 4. 段数从少到多、同段数首段更长优先；结果有上限；
//! 5. 选中组合词 = **整段消耗**（不留半截后缀）。
//!
//! 真库（luna.opid）的验收在 `crates/opi-tools/tests/composition_real.rs`。

use engine_core::candidates::RankOptions;
use engine_core::candidates::{Candidate, rank_and_pick_with};
use engine_core::dictionary::InMemoryDictionary;
use engine_core::learner::Learner;
use engine_core::router::KeyRouter;
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Mode};

fn dict() -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    // 3 字：厌蠢 + 症（`厌蠢` 两音节、`症` 一音节）
    d.insert("yanchun", "厌蠢", 700);
    d.insert("yan", "眼", 900);
    d.insert("chun", "春", 800);
    d.insert("chunzheng", "纯正", 600);
    d.insert("zheng", "症", 850);
    d.insert("zheng", "正", 950);
    d.insert("zheng", "政", 940);
    d.insert("zheng", "争", 930);
    // 另一条 3 字：样样 + 通
    d.insert("yangyang", "样样", 700);
    d.insert("yangyang", "痒痒", 710);
    d.insert("yang", "样", 900);
    d.insert("tong", "通", 850);
    d.insert("tong", "同", 950);
    // 4 字 / 5 字：两段都是多音节词
    d.insert("zhongguo", "中国", 900);
    d.insert("renmin", "人民", 880);
    d.insert("zhongyang", "中央", 860);
    d.insert("zhengfu", "政府", 840);
    // 负控制：只有单音节键
    d.insert("ma", "妈", 900);
    // 显示/逐字确认夹具（与组合同文件，避免再建一份词库）
    d.insert("ni", "你", 900);
    d.insert("hao", "好", 850);
    // 负控制：只有「前缀扩展」键（`zhengren` 不是 `zheng`）
    d.insert("zhengren", "症人", 990);
    d
}

fn engine() -> Engine {
    Engine::new(
        Box::new(dict()),
        SymbolEngine::new(Vec::new(), Vec::new()),
        false,
    )
}

fn type_str(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

fn texts(e: &Engine, limit: usize) -> Vec<String> {
    e.candidates(limit).into_iter().map(|c| c.text).collect()
}

fn index_of(cands: &[Candidate], text: &str) -> usize {
    cands
        .iter()
        .position(|c| c.text == text)
        .unwrap_or_else(|| {
            panic!(
                "候选里没有 {text}: {:?}",
                cands.iter().map(|c| &c.text).collect::<Vec<_>>()
            )
        })
}

#[test]
fn three_char_words_compose_from_two_chunks() {
    let mut e = engine();
    type_str(&mut e, "yanchunzheng");
    let got = texts(&e, 64);
    assert!(got.contains(&"厌蠢症".to_string()), "厌蠢 + 症: {got:?}");
    // 同段数（2 段）内首段更长（`yanchun` > `yan`）优先：`厌蠢症` 全在
    // `yan + chunzheng` 那条切法之前。
    let pos = got.iter().position(|t| t == "厌蠢症").unwrap();
    let alt = got.iter().position(|t| t == "眼纯正");
    assert!(alt.is_none_or(|a| pos < a), "首段更长的切法优先: {got:?}");

    let mut e = engine();
    type_str(&mut e, "yangyangtong");
    let got = texts(&e, 64);
    assert!(got.contains(&"样样通".to_string()), "样样 + 通: {got:?}");
}

#[test]
fn four_and_five_char_words_compose() {
    for (input, want) in [
        ("zhongguorenmin", "中国人民"),
        ("zhongyangzhengfu", "中央政府"),
    ] {
        let mut e = engine();
        type_str(&mut e, input);
        let got = texts(&e, 64);
        assert!(got.contains(&want.to_string()), "{input} → {want}: {got:?}");
    }
}

/// 负控制 1：全是单音节段 ⇒ 不拼（否则任意同音字都能凑成一串）。
#[test]
fn all_single_syllable_chunks_are_not_composed() {
    let mut e = engine();
    type_str(&mut e, "mamama");
    let got = texts(&e, 64);
    assert!(
        got.iter().all(|t| t.chars().count() <= 1),
        "单音节段拼接是噪音，不该出多字组合: {got:?}"
    );
}

/// 负控制 2：整串必须**连续覆盖**，有缺口不出候选（`yan` + `zheng` 不能跳过 `chun`）。
#[test]
fn chunks_must_cover_every_letter() {
    let mut d = InMemoryDictionary::new();
    d.insert("yan", "眼", 900);
    d.insert("chun", "春", 800);
    d.insert("zheng", "正", 950);
    let mut e = Engine::new(
        Box::new(d),
        SymbolEngine::new(Vec::new(), Vec::new()),
        false,
    );
    type_str(&mut e, "yanchunzheng");
    let got = texts(&e, 64);
    assert!(
        !got.iter().any(|t| t.contains("眼正") || t == "眼症"),
        "跳过 chun 的拼接不该出现: {got:?}"
    );
}

/// 负控制 3：段必须是**精确键**；只有以它为前缀的更长键不算。
#[test]
fn chunk_lookup_is_exact_not_prefix() {
    let mut d = InMemoryDictionary::new();
    d.insert("yanchun", "厌蠢", 700);
    d.insert("zhengren", "症人", 990); // 只有 `zheng` 的前缀扩展
    let mut e = Engine::new(
        Box::new(d),
        SymbolEngine::new(Vec::new(), Vec::new()),
        false,
    );
    type_str(&mut e, "yanchunzheng");
    let got = texts(&e, 64);
    assert!(
        !got.iter().any(|t| t.starts_with("厌蠢症")),
        "`zheng` 不该拿 `zhengren` 当精确键: {got:?}"
    );
}

/// 规则 3：整串有精确命中 ⇒ 组合层不动手，管线逐字节相同。
#[test]
fn exact_hit_leaves_the_pipeline_untouched() {
    let mut d = dict();
    d.insert("yanchunzheng", "厌蠢症", 999);
    let learner = Learner::new(false);
    let opts = |composition| RankOptions {
        fuzzy: 0,
        correction: false,
        jianpin: false,
        prefer_simplified: true,
        segment_fallback: true,
        composition,
        tiers: true,
    };
    let on = rank_and_pick_with(
        &d,
        &SymbolEngine::new(Vec::new(), Vec::new()),
        &learner,
        "yanchunzheng",
        Mode::Pinyin,
        64,
        1_800,
        opts(true),
    );
    let off = rank_and_pick_with(
        &d,
        &SymbolEngine::new(Vec::new(), Vec::new()),
        &learner,
        "yanchunzheng",
        Mode::Pinyin,
        64,
        1_800,
        opts(false),
    );
    assert_eq!(on, off, "整串精确命中时组合层必须完全惰性");
}

/// 结果上限：真库同一前缀的竞争者可能很多，组合结果封顶（`composition::MAX_RESULTS`）。
#[test]
fn composition_results_are_capped() {
    let mut d = InMemoryDictionary::new();
    d.insert("yanchun", "厌蠢", 700);
    for (i, ch) in "正政争整证征症睁蒸挣郑帧桢祯铮筝狰峥钲鲭"
        .chars()
        .enumerate()
    {
        d.insert("zheng", &ch.to_string(), 900u32.saturating_sub(i as u32));
    }
    let learner = Learner::new(false);
    let got = rank_and_pick_with(
        &d,
        &SymbolEngine::new(Vec::new(), Vec::new()),
        &learner,
        "yanchunzheng",
        Mode::Pinyin,
        64,
        1_800,
        RankOptions {
            fuzzy: 0,
            correction: false,
            jianpin: false,
            prefer_simplified: false,
            segment_fallback: true,
            composition: true,
            tiers: true,
        },
    );
    assert_eq!(got.len(), 16, "组合结果上限 16 条: {got:?}");
    assert!(got.iter().all(|c| c.text.chars().count() > 1));
}

/// 选中组合词 = 整段消耗：不能按「第一个字的回退」只留后缀。
#[test]
fn selecting_a_composition_consumes_the_whole_buffer() {
    let mut r = KeyRouter::new(engine());
    type_str(r.engine_mut(), "yanchunzheng");
    let cands = r.engine().candidates(usize::MAX);
    let idx = index_of(&cands, "厌蠢症");
    let committed = r.select(idx);
    assert_eq!(committed, "厌蠢症");
    assert_eq!(r.engine().buffer(), "", "整段消耗（不留下半截拼音）");
    assert_eq!(r.engine().pending_text(), "", "组合不是逐字确认");
}

// ---- 显示层自动分词（全键盘）----

#[test]
fn full_keyboard_display_segments_syllables_without_touching_the_buffer() {
    for (input, shown) in [
        ("n", "n"),
        ("ni", "ni"),
        ("nih", "ni'h"),
        ("nihao", "ni'hao"),
        ("nihaob", "ni'hao'b"),
        ("yanchunzheng", "yan'chun'zheng"),
        // 手写分词符是硬边界：保留用户的位置，其余照常补。
        ("yan'chunzheng", "yan'chun'zheng"),
    ] {
        let mut e = engine();
        type_str(&mut e, input);
        assert_eq!(e.display_buffer(), shown, "display({input})");
        assert_eq!(e.buffer(), input, "buffer() 必须保持原样（行为判定读它）");
    }
}

#[test]
fn display_keeps_the_confirmed_prefix() {
    let mut r = KeyRouter::new(engine());
    type_str(r.engine_mut(), "nihaob");
    let cands = r.engine().candidates(8);
    assert_eq!(cands[0].text, "你");
    r.select(0);
    assert_eq!(r.engine().display_buffer(), "你hao'b");
    assert_eq!(r.engine().buffer(), "haob");
}
