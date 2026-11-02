// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

use std::time::{Duration, Instant};

use tsf_opi::logic::{KEY_SHIFT, KEY_SPACE, KEY_STATE_RELEASED, TsfLogic};
use tsf_opi::vk::ModeHotkey;

use crate::session::{ImmAction, ImmSession};

fn session() -> ImmSession {
    ImmSession::new(TsfLogic::load(None).expect("fallback dict"))
}

fn type_keys(s: &mut ImmSession, keys: &str) {
    for c in keys.chars() {
        s.process(c as u32, 0);
    }
}

fn has_result(actions: &[ImmAction], text: &str) -> bool {
    actions
        .iter()
        .any(|a| matches!(a, ImmAction::Result(t) if t == text))
}

fn candidate_list(actions: &[ImmAction]) -> Option<Vec<String>> {
    actions.iter().find_map(|a| match a {
        ImmAction::ShowCandidates { items, .. } => Some(items.clone()),
        _ => None,
    })
}

#[test]
fn typing_pinyin_shows_composition_and_candidates() {
    let mut s = session();
    let first = s.process('n' as u32, 0);
    assert!(matches!(first.first(), Some(ImmAction::StartComposition)));
    assert!(
        first
            .iter()
            .any(|a| matches!(a, ImmAction::SetComposition(t) if t == "n"))
    );

    let second = s.process('i' as u32, 0);
    assert!(!second.contains(&ImmAction::StartComposition), "只开始一次");
    assert!(
        second
            .iter()
            .any(|a| matches!(a, ImmAction::SetComposition(t) if t == "ni"))
    );
    let items = candidate_list(&second).expect("ni 应有候选");
    assert!(items.iter().any(|c| c.contains('你')), "候选={items:?}");
}

#[test]
fn space_commits_first_candidate_then_ends_composition() {
    let mut s = session();
    type_keys(&mut s, "ni");
    let out = s.process(KEY_SPACE, 0);
    assert!(has_result(&out, "你"), "actions={out:?}");
    assert!(out.contains(&ImmAction::EndComposition));
    assert!(out.contains(&ImmAction::HideCandidates));
}

#[test]
fn backspacing_to_empty_ends_composition() {
    let mut s = session();
    type_keys(&mut s, "ni");
    let back = tsf_opi::logic::KEY_BACK_SPACE;
    let out = s.process(back, 0);
    assert!(!out.contains(&ImmAction::EndComposition), "还有一个字符");
    let out = s.process(back, 0);
    assert!(out.contains(&ImmAction::EndComposition), "actions={out:?}");
    assert!(out.contains(&ImmAction::HideCandidates));
}

#[test]
fn shift_tap_switches_language_and_flushes_pending_pinyin() {
    let t = Instant::now();
    let mut s = session();
    type_keys(&mut s, "ni");
    let down = s.process_at(KEY_SHIFT, 0, t);
    assert!(down.is_empty(), "{down:?}");
    let up = s.process_at(KEY_SHIFT, KEY_STATE_RELEASED, t + Duration::from_millis(50));
    assert!(has_result(&up, "ni"), "切英文时应上原文：{up:?}");
    assert!(up.contains(&ImmAction::EndComposition));

    // 已切到英文：字母直出，不再组字。
    let out = s.process('a' as u32, 0);
    assert!(has_result(&out, "a"), "{out:?}");
}

#[test]
fn long_shift_hold_does_not_toggle() {
    let t = Instant::now();
    let mut s = session();
    s.process_at(KEY_SHIFT, 0, t);
    let up = s.process_at(
        KEY_SHIFT,
        KEY_STATE_RELEASED,
        t + Duration::from_millis(500),
    );
    assert!(up.is_empty(), "长按不切：{up:?}");
    let out = s.process('n' as u32, 0);
    assert!(
        out.iter()
            .any(|a| matches!(a, ImmAction::SetComposition(t) if t == "n"))
    );
}

#[test]
fn shift_plus_letter_does_not_toggle() {
    let t = Instant::now();
    let mut s = session();
    s.process_at(KEY_SHIFT, 0, t);
    s.process_at('a' as u32, 0, t + Duration::from_millis(20));
    let up = s.process_at(KEY_SHIFT, KEY_STATE_RELEASED, t + Duration::from_millis(60));
    assert!(up.is_empty(), "Shift+字母不是轻按：{up:?}");
    // 仍在中文：a 已进缓冲。
    let out = s.process('b' as u32, 0);
    assert!(
        out.iter().any(
            |a| matches!(a, ImmAction::SetComposition(t) if t.contains('a') && t.contains('b'))
        ),
        "{out:?}"
    );
}

#[test]
fn selecting_a_candidate_commits_it() {
    let mut s = session();
    type_keys(&mut s, "ni");
    let out = s.select(0);
    assert!(has_result(&out, "你"), "{out:?}");
    assert!(out.contains(&ImmAction::EndComposition));
    assert!(out.contains(&ImmAction::HideCandidates));
}

#[test]
fn ctrl_hotkey_toggles_mode_and_flushes_composition() {
    let mut s = session();
    type_keys(&mut s, "ni");
    let out = s.toggle_mode_hotkey(ModeHotkey::ToggleEnglish);
    assert!(has_result(&out, "ni"), "切英文应上原文：{out:?}");
    assert!(out.contains(&ImmAction::EndComposition));
    let out = s.process('a' as u32, 0);
    assert!(has_result(&out, "a"), "已切英文：{out:?}");
}

#[test]
fn unclaimed_keydown_disarms_shift_tap() {
    let t = Instant::now();
    let mut s = session();
    s.process_at(KEY_SHIFT, 0, t);
    s.disarm_shift_tap();
    let up = s.process_at(KEY_SHIFT, KEY_STATE_RELEASED, t + Duration::from_millis(50));
    assert!(up.is_empty(), "Shift+方向键后松开不应切中英：{up:?}");
    let out = s.process('n' as u32, 0);
    assert!(
        out.iter()
            .any(|a| matches!(a, ImmAction::SetComposition(t) if t == "n"))
    );
}

#[test]
fn ctrl_combinations_are_not_claimed() {
    let s = session();
    assert!(!s.claim('c' as u32, tsf_opi::logic::KEY_STATE_CTRL));
    assert!(s.claim('n' as u32, 0));
    assert!(s.claim(KEY_SHIFT, 0));
    assert!(!s.claim('n' as u32, KEY_STATE_RELEASED), "字母抬起交给应用");
    assert!(
        s.claim(KEY_SHIFT, KEY_STATE_RELEASED),
        "Shift 抬起必须进 IME"
    );
}
