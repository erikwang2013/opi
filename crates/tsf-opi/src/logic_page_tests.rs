// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 组字中的 `-` / `=` 翻页 —— 与 `crates/fcitx5-opi/src/input_method_page_tests.rs`
//! 逐条同构（同名同序，两轨共用同一张行为表）。
//!
//! 判据：**缓冲非空**（= 正在组字、候选页存在）时，`-` → 上一页、`=` → 下一页，
//! 与 PageUp/PageDown 同一动作、同一返回值（`CompositionChanged`）；缓冲空时
//! **不抢键**，仍走标点表/直通的老路。
//!
//! 有意**不**扩展到英文/数字/符号模式：那几路里 `-`/`=` 另有归属（引擎的
//! 「标点永远有归宿」原地提交、直传通道等）—— 抢走它们才是 bug。

use super::*;
use engine_core::dictionary::InMemoryDictionary;

/// 20 个 "qwx" 词条 + 引擎：确定性的 3 页候选（同 logic_tests.rs 的夹具；键避开符号/emoji 关键字）。
fn page_state() -> TsfLogic {
    let mut d = InMemoryDictionary::new();
    for i in 0..20 {
        d.insert("qwx", &format!("词{i:02}"), (5000 - i) as u32);
    }
    let mut s = TsfLogic {
        engine: engine_core::Engine::new(
            Box::new(d),
            engine_core::symbols::SymbolEngine::builtin(),
            true,
        ),
        page: 0,
        buffer_snapshot: String::new(),
        shift_state: ShiftState::Off,
        last_printable: None,
    };
    s.refresh_snapshot();
    s.switch_mode(Mode::Pinyin);
    s
}

fn typed(s: &mut TsfLogic, word: &str) {
    for c in word.chars() {
        s.input_key(c as u32, 0);
    }
}

/// 组字中：`=` 下一页、`-` 上一页、两端钳制；缓冲不被翻页动。
#[test]
fn minus_and_equal_page_while_composing() {
    let mut s = page_state();
    typed(&mut s, "qwx");
    assert_eq!(s.input_key('=' as u32, 0), KeyOutcome::CompositionChanged);
    assert_eq!(s.page(), 1, "'=' 下一页");
    assert_eq!(s.input_key('=' as u32, 0), KeyOutcome::CompositionChanged);
    assert_eq!(s.input_key('=' as u32, 0), KeyOutcome::CompositionChanged); // 钳制
    assert_eq!(s.page(), 2);
    assert_eq!(s.input_key('-' as u32, 0), KeyOutcome::CompositionChanged);
    assert_eq!(s.page(), 1, "'-' 上一页");
    s.input_key('-' as u32, 0);
    assert_eq!(s.input_key('-' as u32, 0), KeyOutcome::CompositionChanged); // 钳制
    assert_eq!(s.page(), 0);
    assert_eq!(s.buffer(), "qwx", "翻页不动缓冲");
}

/// 抬起与按下同判：按下被消费（翻页），抬起也必须消费 ——
/// 否则客户端收到无 keydown 的 keyup（见 logic_input_method.rs 可打印分支的注释）。
#[test]
fn minus_and_equal_release_consumed_after_consumed_press() {
    let mut s = page_state();
    typed(&mut s, "qwx");
    s.input_key('=' as u32, 0);
    assert_eq!(
        s.input_key('=' as u32, KEY_STATE_RELEASED),
        KeyOutcome::Consumed
    );
}

/// 缓冲空：不翻页、不吞键 —— 行为与本轮改动前逐字相同（全角开 → 机械全角上屏）。
#[test]
fn empty_buffer_minus_equal_keep_old_path() {
    let mut s = page_state();
    for c in ['-', '='] {
        let outcome = s.input_key(c as u32, 0);
        assert!(
            matches!(outcome, KeyOutcome::Commit(_) | KeyOutcome::Unhandled),
            "空缓冲下 {c:?} 被吞成了 {outcome:?}"
        );
        assert_eq!(s.page(), 0, "空缓冲不翻页");
    }
    // 关掉全角后同样是「直通」而不是「吞」。
    s.toggle_fullwidth();
    for c in ['-', '='] {
        assert_eq!(
            s.input_key(c as u32, 0),
            KeyOutcome::Unhandled,
            "全角关：{c:?} 直通客户端"
        );
    }
}

/// 符号模式不归翻页管：`-` 走引擎的「标点永远有归宿」规则**原地提交**半角连字符
/// （实测的现行行为），本轮改动不许碰它 —— 边界就是模式闸门。
#[test]
fn symbol_mode_minus_keeps_its_punctuation_home() {
    let mut s = page_state();
    s.switch_mode(Mode::Symbol);
    assert_eq!(s.input_key('-' as u32, 0), KeyOutcome::Commit("-".into()));
    assert_eq!(s.page(), 0, "符号模式不翻页");
}
