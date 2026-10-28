// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `claim_on_test` 的回归：它决定 TSF 的 `OnTestKeyDown` 答不答 TRUE，而 TRUE
//! 是不可撤回的吞键决定（见 `logic_claim.rs` 模块头）。这里钉住两类：
//! 引擎**不会**消费的键必须答 FALSE（否则功能键/空缓冲退格被吞），会消费的
//! 必须答 TRUE（否则中文输入/空格上屏断掉）。

use engine_core::composer::Mode;
use engine_core::dictionary::InMemoryDictionary;
use engine_core::{Engine, symbols::SymbolEngine};

use crate::logic::{
    KEY_BACK_SPACE, KEY_DELETE, KEY_ESCAPE, KEY_PAGE_DOWN, KEY_PAGE_UP, KEY_RETURN, KEY_SPACE,
    KEY_TAB, ShiftState, TsfLogic,
};
use crate::vk::NO_KEYVAL;

fn state() -> TsfLogic {
    let mut d = InMemoryDictionary::new();
    for i in 0..20 {
        d.insert("qwx", &format!("词{i:02}"), (5000 - i) as u32);
    }
    let mut s = TsfLogic {
        engine: Engine::new(Box::new(d), SymbolEngine::builtin(), true),
        page: 0,
        buffer_snapshot: String::new(),
        shift_state: ShiftState::Off,
        english_pro: true,
        last_printable: None,
        shift_toggle: true,
        association_enabled: true,
        fetch_cache: Default::default(),
    };
    s.refresh_snapshot();
    s
}

fn typed(text: &str) -> TsfLogic {
    let mut s = state();
    for c in text.chars() {
        s.input_key(c as u32, 0);
    }
    s
}

#[test]
fn unmapped_keys_are_never_claimed() {
    // `claim_on_test` 收的是**引擎键码**：功能键的 raw VK 由 `to_engine_keycode`
    // 先转成 NO_KEYVAL（vk 模块的单测钉住那条映射），这里只钉终态。
    let s = typed("qwx");
    assert!(!s.claim_on_test(NO_KEYVAL));
}

#[test]
fn empty_buffer_special_keys_pass_to_the_app() {
    let s = state();
    for key in [
        KEY_BACK_SPACE,
        KEY_DELETE,
        KEY_RETURN,
        KEY_ESCAPE,
        KEY_PAGE_UP,
        KEY_PAGE_DOWN,
        KEY_TAB,
    ] {
        assert!(!s.claim_on_test(key), "空缓冲的 {key:#x} 必须交给应用");
    }
    // 空格永远由引擎消费（空缓冲提交半角空格）。
    assert!(s.claim_on_test(KEY_SPACE));
}

#[test]
fn composing_backspace_enter_and_pages_are_claimed() {
    let s = typed("qwx");
    for key in [
        KEY_BACK_SPACE,
        KEY_DELETE,
        KEY_RETURN,
        KEY_PAGE_UP,
        KEY_PAGE_DOWN,
    ] {
        assert!(s.claim_on_test(key), "组字中的 {key:#x} 必须由引擎处理");
    }
}

#[test]
fn pinyin_letters_digits_and_punctuation_are_claimed() {
    let s = typed("qwx");
    for c in ['a', 'z', '\'', '1', '8', ',', '[', '-', '='] {
        assert!(s.claim_on_test(c as u32), "{c:?} 必须由拼音引擎处理");
    }
    // PAGE_SIZE=8：第 9 项不在本页，数字 9/0 都不选词（引擎 Unhandled）⇒ 不能认领。
    assert!(!s.claim_on_test('9' as u32));
    assert!(!s.claim_on_test('0' as u32));
}

#[test]
fn non_ascii_printable_keys_pass_through() {
    let s = typed("qwx");
    for c in ['ü', 'ñ', '中'] {
        assert!(!s.claim_on_test(c as u32), "{c:?} 引擎不处理，必须放行");
    }
}

#[test]
fn number_mode_with_fullwidth_off_passes_printable_keys() {
    let mut s = state();
    s.switch_mode(Mode::Number);
    if s.fullwidth() {
        s.toggle_fullwidth();
    }
    for c in ['1', 'a', ','] {
        assert!(!s.claim_on_test(c as u32), "数字模式全角关：{c:?} 必须放行");
    }
    s.toggle_fullwidth();
    assert!(s.claim_on_test('1' as u32), "全角开：数字由引擎直传");
}

#[test]
fn symbol_mode_digits_without_candidates_pass_to_the_app() {
    let mut s = state();
    s.switch_mode(Mode::Symbol);
    assert!(!s.claim_on_test('1' as u32), "无候选的数字键必须放行");
    assert!(s.claim_on_test('a' as u32), "符号关键字字母由引擎消费");
}

#[test]
fn english_mode_claims_letters_and_empty_buffer_printables() {
    let mut s = state();
    s.switch_mode(Mode::English);
    for c in ['a', 'Z', '1', ','] {
        assert!(s.claim_on_test(c as u32), "英文模式空缓冲：{c:?} 应直传");
    }
}
