// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 联想态的按键语义（2026-10-27，Spec §5）：按下先收起、抬起不动、
//! Ctrl/Alt 直通也收起、联想态数字不选词（空缓冲数字本来就落 PassThrough）。
//! 用内存词库走真引擎路径：喂 `jintian` 选「今天」进联想态。

use super::{KEY_SPACE, KEY_STATE_CTRL, KEY_STATE_RELEASED, KeyAction, handle_key};
use crate::candidate::CandidateState;
use engine_core::Engine;
use engine_core::composer::Mode;
use engine_core::symbols::SymbolEngine;
use engine_data::{AssocRow, OpDict, RawEntry, load_bytes, serialize_with_extras};

/// 内存词库：今天 / 今天晚上 + assoc「今天→回家」。
fn association_state() -> CandidateState {
    let entries = vec![
        RawEntry {
            pinyin: "jintian".into(),
            word: "今天".into(),
            freq: 900,
        },
        RawEntry {
            pinyin: "jintianwanshang".into(),
            word: "今天晚上".into(),
            freq: 700,
        },
        RawEntry {
            pinyin: "huijia".into(),
            word: "回家".into(),
            freq: 600,
        },
    ];
    let bytes = serialize_with_extras(
        &OpDict {
            pinyin_total: entries.iter().map(|e| e.pinyin.len()).sum(),
            entries,
        },
        true,
        &[AssocRow {
            root: "今天".into(),
            suffix: "回家".into(),
            rank: 90,
        }],
    );
    let dict = Box::new(load_bytes(bytes).expect("测试词库应可装载"));
    let mut state = CandidateState::load(None).expect("回退词库应可装载");
    state.engine = Engine::with_dictionaries(dict, None, SymbolEngine::builtin(), true);
    state.switch_mode(Mode::Pinyin);
    for ch in "jintian".chars() {
        state.engine.input_key(ch);
    }
    assert_eq!(
        state.engine.select(0),
        "今天",
        "测试前提：jintian 首候选是今天"
    );
    assert!(
        state.engine.association_active(),
        "测试前提：提交后进入联想态"
    );
    state
}

#[test]
fn keydown_dismisses_association_before_dispatch() {
    let mut state = association_state();
    assert!(state.engine.association_active());
    assert_eq!(
        handle_key(&mut state, KEY_SPACE, 0),
        KeyAction::Input(" ".into())
    );
    assert!(!state.engine.association_active());
}

#[test]
fn release_does_not_dismiss() {
    let mut state = association_state();
    let _ = handle_key(&mut state, KEY_SPACE, KEY_STATE_RELEASED);
    assert!(state.engine.association_active());
}

#[test]
fn ctrl_combo_dismisses_but_passes_through() {
    let mut state = association_state();
    assert_eq!(
        handle_key(&mut state, 'a' as u32, KEY_STATE_CTRL),
        KeyAction::PassThrough
    );
    assert!(!state.engine.association_active());
}

#[test]
fn digit_in_association_does_not_select_candidate() {
    let mut state = association_state();
    let action = handle_key(&mut state, '1' as u32, 0);
    // 空缓冲数字落 PassThrough（不进 select），且联想已清
    assert_eq!(action, KeyAction::PassThrough);
    assert!(!state.engine.association_active());
}
