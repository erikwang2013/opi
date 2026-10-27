// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 联想状态的生命周期（Task 6 清空矩阵）：任何会改变输入上下文的入口都必须
//! 先收起联想 —— 否则一个按键之后候选行还停在上一次的联想上。

mod common;

use common::{AssocDict, type_str};
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Mode, Scheme};

fn active_engine() -> Engine {
    let d = AssocDict::new()
        .word("jintian", "今天", 900)
        .prefix("今天", &[("今天晚上", 700)])
        .suffix("今天", &[("吃饭", 90)]);
    let mut e = Engine::new(Box::new(d), SymbolEngine::builtin(), true);
    type_str(&mut e, "jintian");
    assert_eq!(e.select(0), "今天");
    assert!(e.association_active(), "前置：提交后进入联想态");
    e
}

#[test]
fn every_input_entry_clears_association() {
    let mut e = active_engine();
    e.input_key('a');
    assert!(!e.association_active(), "input_key");

    let mut e = active_engine();
    e.backspace();
    assert!(!e.association_active(), "backspace");

    let mut e = active_engine();
    let _ = e.input_punct_at('，', 0);
    assert!(!e.association_active(), "input_punct_at");

    let mut e = active_engine();
    e.clear();
    assert!(!e.association_active(), "clear");

    let mut e = active_engine();
    e.switch_mode(Mode::English);
    assert!(!e.association_active(), "switch_mode");

    let mut e = active_engine();
    e.switch_language(Mode::English, Scheme::Full);
    assert!(!e.association_active(), "switch_language");

    let mut e = active_engine();
    e.set_scheme(Scheme::Nine);
    assert!(!e.association_active(), "set_scheme");

    let mut e = active_engine();
    e.toggle_symbol();
    assert!(!e.association_active(), "toggle_symbol");

    let mut e = active_engine();
    e.set_shift(true);
    assert!(!e.association_active(), "set_shift");
}

#[test]
fn dictionary_setters_clear_association() {
    let mut e = active_engine();
    e.set_extra_dicts(Vec::new());
    assert!(!e.association_active(), "set_extra_dicts");

    let mut e = active_engine();
    e.set_trad_dict(None);
    assert!(!e.association_active(), "set_trad_dict");

    let mut e = active_engine();
    e.set_stroke_dict(None);
    assert!(!e.association_active(), "set_stroke_dict");

    let mut e = active_engine();
    e.set_wubi_dict(None);
    assert!(!e.association_active(), "set_wubi_dict");
}
