// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「语言切换保留缓冲」的宿主回归（2026-10-18 英文单词联想）。
//!
//! 用户裁决 2026-10-09：中文打字时切英文，拼音缓冲保留、切回中文可继续用。
//! 英文进入引擎匹配后，这条语义的唯一判据是引擎的携带标记
//! （`Engine::buffer_is_carried`）：**携带缓冲**保持旧直传（字母上屏、缓冲不动），
//! 英文自己的缓冲才进引擎。两轨逐条同构：本文件 ↔
//! `crates/fcitx5-opi/src/input_method_carried_tests.rs`（同名同序）。
//!
//! 宿主 UI 的切模式热键走 `switch_mode`（清缓冲），到不了这里；`switch_language`
//! 由平台外壳调用（TSF 轨目前无此入口），故测试直接调引擎。

use super::*;
use engine_core::dictionary::InMemoryDictionary;

/// `ni → 你`、`he → 和`：前者钉「携带缓冲不出英文候选」，后者钉「英文 he 不当拼音」。
fn carried_state() -> TsfLogic {
    let mut d = InMemoryDictionary::new();
    d.insert("ni", "你", 100);
    d.insert("he", "和", 200);
    let mut s = TsfLogic {
        engine: engine_core::Engine::new(
            Box::new(d),
            engine_core::symbols::SymbolEngine::builtin(),
            true,
        ),
        page: 0,
        buffer_snapshot: String::new(),
        shift_state: ShiftState::Off,
        english_pro: true,
        last_printable: None,
    };
    s.switch_mode(Mode::Pinyin);
    s
}

fn typed(s: &mut TsfLogic, word: &str) {
    for c in word.chars() {
        assert_eq!(s.input_key(c as u32, 0), KeyOutcome::CompositionChanged);
    }
}

/// 中→英：保留的拼音**不喂**英文引擎 —— 字母直传上屏，缓冲原样留着切回中文用。
#[test]
fn carried_pinyin_buffer_direct_passes_letters_and_survives() {
    let mut s = carried_state();
    typed(&mut s, "ni");
    s.engine
        .switch_language(Mode::English, engine_core::Scheme::Full);
    assert!(s.engine.buffer_is_carried(), "中→英保留缓冲须打标");
    assert_eq!(s.buffer(), "ni");
    assert!(
        s.engine.candidates(8).is_empty(),
        "携带拼音不是英文前缀，不出候选"
    );
    assert_eq!(s.input_key('a' as u32, 0), KeyOutcome::Commit("a".into()));
    assert_eq!(s.buffer(), "ni", "字母直传，保留的拼音不被追加");
}

/// 英→中：保留的英文词**不喂**拼音匹配 —— "he" 不得出「和」；空格按无候选提交原文。
#[test]
fn switching_back_to_chinese_does_not_match_english() {
    let mut s = carried_state();
    s.switch_mode(Mode::English); // 清缓冲
    typed(&mut s, "he");
    s.engine
        .switch_language(Mode::Pinyin, engine_core::Scheme::Full);
    assert_eq!(s.buffer(), "he", "英→中保留缓冲");
    assert!(s.engine.candidates(8).is_empty(), "英文词不是拼音：he ≠ 和");
    assert_eq!(s.input_key(KEY_SPACE, 0), KeyOutcome::Commit("he".into()));
    assert_eq!(s.buffer(), "");
}

/// 英文**自己的**缓冲：空格/数字/回车/退格全走引擎（不直传、不用旧英文直传语义）。
#[test]
fn english_own_buffer_edit_keys_use_engine() {
    let mut s = carried_state();
    s.switch_mode(Mode::English);
    typed(&mut s, "he");
    let top = s.engine.candidates(8)[0].text.clone();
    assert!(!top.is_empty(), "前置：he 有英文候选");
    // 空格 = 首候选
    assert_eq!(s.input_key(KEY_SPACE, 0), KeyOutcome::Commit(top.clone()));
    assert_eq!(s.buffer(), "");
    // 数字 = 页内选词（'1' → 第 0 项）
    typed(&mut s, "he");
    assert_eq!(s.input_key('1' as u32, 0), KeyOutcome::Commit(top.clone()));
    assert_eq!(s.buffer(), "");
    // 回车 = 引擎 enter_action（缺省选首候选）
    typed(&mut s, "he");
    assert_eq!(s.input_key(KEY_RETURN, 0), KeyOutcome::Commit(top));
    assert_eq!(s.buffer(), "");
    // 退格 = 引擎删字母，不直传
    typed(&mut s, "he");
    assert_eq!(
        s.input_key(KEY_BACK_SPACE, 0),
        KeyOutcome::CompositionChanged
    );
    assert_eq!(s.buffer(), "h");
}
