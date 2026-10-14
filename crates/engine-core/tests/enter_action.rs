// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **回车判据**门禁（2026-10-14 接线；实现与理由见 `src/engine_enter.rs`）。
//!
//! 接线前这条设置在四端都是摆设（三处 handleEnter 各自写死「选首候选」）——
//! 本文件钉住「单一判据」的实际取值，宿主侧的消费各有一条自己的用例
//! （fcitx5 / TSF 的 `*_tests.rs`）。

use engine_core::composer::Mode;
use engine_core::dictionary::InMemoryDictionary;
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, EnterAction, Scheme, WubiReturnStyle};

fn engine() -> Engine {
    let mut d = InMemoryDictionary::new();
    d.insert("wqvb", "你好", 900);
    d.insert("ni", "你", 900);
    let mut e = Engine::new(Box::new(d), SymbolEngine::builtin(), true);
    e.switch_mode(Mode::Pinyin);
    e
}

fn type_str(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

/// 非五笔方案：回车语义是「选首候选」，**设置位不参与**（拼音/双拼/9 键一字不动）。
#[test]
fn other_schemes_always_select_the_first_candidate() {
    let mut e = engine();
    e.set_wubi_return_style(WubiReturnStyle::CommitRaw); // 故意设成五笔才认的档
    type_str(&mut e, "ni");
    assert_eq!(e.enter_action(), EnterAction::SelectFirst);
}

/// 五笔：默认 `Clear` ⇒ 清空、不上屏；换 `CommitRaw` ⇒ 上屏**缓冲原文**。
#[test]
fn wubi_reads_the_setting() {
    let mut e = engine();
    e.set_scheme(Scheme::Wubi);
    type_str(&mut e, "wqvb");
    assert_eq!(e.enter_action(), EnterAction::Clear, "默认档 = 清空");
    e.set_wubi_return_style(WubiReturnStyle::CommitRaw);
    assert_eq!(
        e.enter_action(),
        EnterAction::CommitRaw("wqvb".to_string()),
        "原文 = 缓冲里的码，原样"
    );
}

/// 空缓冲：判据不吃这一格（宿主自己直通回车 —— 三处 handleEnter 都是先判空）。
#[test]
fn empty_buffer_falls_back_to_select_first() {
    let mut e = engine();
    e.set_scheme(Scheme::Wubi);
    assert_eq!(e.buffer(), "");
    assert_eq!(e.enter_action(), EnterAction::SelectFirst);
}
