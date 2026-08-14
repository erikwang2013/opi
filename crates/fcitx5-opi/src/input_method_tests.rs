use super::*;
use crate::candidate::{CandidateState, ShiftState};
use engine_core::dictionary::InMemoryDictionary;
/// 20 个 "hao" 词条 + 引擎：确定性的 3 页候选。
fn state() -> CandidateState {
    let mut d = InMemoryDictionary::new();
    for i in 0..20 {
        d.insert("hao", &format!("词{i:02}"), (5000 - i) as u32);
    }
    let symbols = engine_core::symbols::SymbolEngine::builtin();
    let mut s = CandidateState {
        engine: engine_core::Engine::new(Box::new(d), symbols, true),
        page: 0,
        buffer_snapshot: String::new(),
        shift_state: ShiftState::Off,
    };
    s.buffer_snapshot = s.engine.buffer().to_string();
    s
}
fn pinyin_state() -> CandidateState {
    let mut s = state();
    s.switch_mode(Mode::Pinyin);
    s
}
// ---- 拼音模式：字母入缓冲 ----
#[test]
fn pinyin_letter_goes_to_engine_buffer() {
    let mut s = pinyin_state();
    assert_eq!(handle_key(&mut s, 'a' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(s.buffer(), "a");
    assert_eq!(handle_key(&mut s, 'b' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(s.buffer(), "ab");
}
#[test]
fn pinyin_uppercase_keysym_lowercased_into_buffer() {
    // xkb 应用物理 shift 后给出 'A' 键值；composer 转小写入缓冲
    let mut s = pinyin_state();
    assert_eq!(handle_key(&mut s, 'A' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(s.buffer(), "a");
}
#[test]
fn pinyin_symbol_passes_through() {
    let mut s = pinyin_state();
    assert_eq!(handle_key(&mut s, '，' as u32, 0), KeyAction::PassThrough);
    assert_eq!(handle_key(&mut s, ',' as u32, 0), KeyAction::PassThrough);
    assert_eq!(s.buffer(), "");
}
#[test]
fn pinyin_apostrophe_goes_to_buffer() {
    let mut s = pinyin_state();
    assert_eq!(handle_key(&mut s, '\'' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(s.buffer(), "'");
}
// ---- 空格 ----
#[test]
fn space_with_buffer_commits_top_candidate() {
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        handle_key(&mut s, c as u32, 0);
    }
    assert_eq!(
        handle_key(&mut s, KEY_SPACE, 0),
        KeyAction::Input("词00".into())
    );
    assert_eq!(s.buffer(), "");
}
#[test]
fn space_with_empty_buffer_commits_space() {
    let mut s = pinyin_state();
    assert_eq!(
        handle_key(&mut s, KEY_SPACE, 0),
        KeyAction::Input(" ".into())
    );
    assert_eq!(s.buffer(), "");
}
// ---- 回车 ----
#[test]
fn enter_with_buffer_selects_first_candidate() {
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        handle_key(&mut s, c as u32, 0);
    }
    assert_eq!(
        handle_key(&mut s, KEY_RETURN, 0),
        KeyAction::Input("词00".into())
    );
    assert_eq!(s.buffer(), "");
}
#[test]
fn enter_with_empty_buffer_passes_through() {
    let mut s = pinyin_state();
    assert_eq!(handle_key(&mut s, KEY_RETURN, 0), KeyAction::PassThrough);
}
// ---- 退格 ----
#[test]
fn backspace_with_buffer_deletes_codepoint() {
    let mut s = pinyin_state();
    for c in ['a', 'b'] {
        handle_key(&mut s, c as u32, 0);
    }
    assert_eq!(
        handle_key(&mut s, KEY_BACK_SPACE, 0),
        KeyAction::EngineHandled
    );
    assert_eq!(s.buffer(), "a");
    assert_eq!(
        handle_key(&mut s, KEY_BACK_SPACE, 0),
        KeyAction::EngineHandled
    );
    assert_eq!(s.buffer(), "");
    // 空缓冲 → 直通客户端
    assert_eq!(
        handle_key(&mut s, KEY_BACK_SPACE, 0),
        KeyAction::PassThrough
    );
}
#[test]
fn backspace_release_event_consumed() {
    let mut s = pinyin_state();
    handle_key(&mut s, 'a' as u32, 0);
    assert_eq!(
        handle_key(&mut s, KEY_BACK_SPACE, KEY_STATE_RELEASED),
        KeyAction::EngineHandled
    );
    assert_eq!(s.buffer(), "a");
}
// ---- 英文模式直传（镜像 handleKey） ----
fn english_state() -> CandidateState {
    let mut s = state();
    s.switch_mode(Mode::English);
    s
}
#[test]
fn english_empty_buffer_lowercase_passes_through() {
    let mut s = english_state();
    assert_eq!(
        handle_key(&mut s, 'a' as u32, 0),
        KeyAction::Input("a".into())
    );
    assert_eq!(s.buffer(), "");
}
#[test]
fn english_empty_buffer_single_shift_commits_upper_and_consumes() {
    let mut s = english_state();
    s.shift_tap(); // Off → Single
    assert_eq!(
        handle_key(&mut s, 'a' as u32, 0),
        KeyAction::Input("A".into())
    );
    // single 已消费：下个字母小写
    assert_eq!(
        handle_key(&mut s, 'b' as u32, 0),
        KeyAction::Input("b".into())
    );
    assert_eq!(s.shift_state(), ShiftState::Off);
    assert_eq!(s.buffer(), "");
}
#[test]
fn english_empty_buffer_lock_keeps_uppercase() {
    let mut s = english_state();
    s.shift_long_press(); // Lock
    assert_eq!(
        handle_key(&mut s, 'a' as u32, 0),
        KeyAction::Input("A".into())
    );
    // Lock 不被消费
    assert_eq!(
        handle_key(&mut s, 'b' as u32, 0),
        KeyAction::Input("B".into())
    );
    assert_eq!(s.shift_state(), ShiftState::Lock);
    assert_eq!(s.buffer(), "");
}
#[test]
fn english_empty_buffer_physical_shift_keysym_passes_upper() {
    // xkb 已应用物理 shift：键值为 'A' + SHIFT 位，仍直传且消费 single
    let mut s = english_state();
    s.shift_tap();
    assert_eq!(
        handle_key(&mut s, 'A' as u32, KEY_STATE_SHIFT),
        KeyAction::Input("A".into())
    );
    assert_eq!(s.shift_state(), ShiftState::Off);
}
#[test]
fn english_letters_always_direct_commit_buffer_stays_empty() {
    // 镜像 KeyRouter.handleKey：英文模式空缓冲每次直传，缓冲永远为空，
    // 故字母永不入引擎缓冲（与 Android 行为一致）
    let mut s = english_state();
    assert_eq!(
        handle_key(&mut s, 'a' as u32, 0),
        KeyAction::Input("a".into())
    );
    assert_eq!(
        handle_key(&mut s, 'b' as u32, 0),
        KeyAction::Input("b".into())
    );
    assert_eq!(s.buffer(), "");
    // 符号直传
    assert_eq!(
        handle_key(&mut s, ',' as u32, 0),
        KeyAction::Input(",".into())
    );
}
#[test]
fn english_nonempty_buffer_letter_goes_to_engine() {
    // 防御路径：仅当缓冲意外非空（如经 engine 直入）时才入引擎，
    // 镜像 KeyRouter 的 else 分支
    let mut s = english_state();
    s.engine.input_key('x'); // 绕过路由器直入引擎缓冲
    assert_eq!(s.buffer(), "x");
    assert_eq!(handle_key(&mut s, 'b' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(s.buffer(), "xb");
    // 缓冲非空时符号直通
    assert_eq!(handle_key(&mut s, ',' as u32, 0), KeyAction::PassThrough);
}
#[test]
fn english_nonempty_buffer_shift_uppercases_in_engine() {
    // 引擎 shift 打开时，非空缓冲路径由 composer 大写（镜像同一分支）
    let mut s = english_state();
    s.engine.input_key('x');
    s.shift_tap();
    assert_eq!(handle_key(&mut s, 'b' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(s.buffer(), "xB");
}
// ---- ⇧ 状态机（镜像 EngineController.shiftTap） ----
#[test]
fn shift_tap_cycles_off_single_off() {
    let mut s = english_state();
    assert_eq!(handle_key(&mut s, KEY_SHIFT_L, 0), KeyAction::EngineHandled);
    assert_eq!(s.shift_state(), ShiftState::Single);
    assert_eq!(handle_key(&mut s, KEY_SHIFT_R, 0), KeyAction::EngineHandled);
    assert_eq!(s.shift_state(), ShiftState::Off);
}
#[test]
fn shift_release_and_repeat_do_not_toggle() {
    let mut s = english_state();
    assert_eq!(
        handle_key(&mut s, KEY_SHIFT_L, KEY_STATE_RELEASED),
        KeyAction::EngineHandled
    );
    assert_eq!(s.shift_state(), ShiftState::Off);
    s.shift_tap();
    assert_eq!(
        handle_key(&mut s, KEY_SHIFT_L, KEY_STATE_REPEAT),
        KeyAction::EngineHandled
    );
    assert_eq!(s.shift_state(), ShiftState::Single);
}
#[test]
fn shift_long_press_locks() {
    let mut s = english_state();
    assert_eq!(
        handle_key(&mut s, KEY_SHIFT_L, KEY_STATE_LONG_PRESSED),
        KeyAction::EngineHandled
    );
    assert_eq!(s.shift_state(), ShiftState::Lock);
}
// ---- 候选选择与翻页 ----
#[test]
fn digit_selects_candidate_page_relative() {
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        handle_key(&mut s, c as u32, 0);
    }
    // '2' → 页内索引 1
    assert_eq!(
        handle_key(&mut s, '2' as u32, 0),
        KeyAction::Input("词01".into())
    );
    assert_eq!(s.buffer(), "");
}
#[test]
fn digit_out_of_range_passes_through() {
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        handle_key(&mut s, c as u32, 0);
    }
    s.next_page();
    s.next_page(); // 末页 4 个候选（词16..词19）
    // '9' → 页内索引 8 越界 → 直通
    assert_eq!(handle_key(&mut s, '9' as u32, 0), KeyAction::PassThrough);
    // '0' → 无索引 → 直通
    assert_eq!(handle_key(&mut s, '0' as u32, 0), KeyAction::PassThrough);
}
#[test]
fn digit_without_candidates_passes_through() {
    let mut s = pinyin_state();
    // 无候选（缓冲空 / 无词条缓冲）
    assert_eq!(handle_key(&mut s, '1' as u32, 0), KeyAction::PassThrough);
    handle_key(&mut s, 'x' as u32, 0);
    handle_key(&mut s, 'x' as u32, 0); // "xx" 无候选
    assert_eq!(handle_key(&mut s, '1' as u32, 0), KeyAction::PassThrough);
}
#[test]
fn page_keys_navigate() {
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        handle_key(&mut s, c as u32, 0);
    }
    assert_eq!(
        handle_key(&mut s, KEY_PAGE_DOWN, 0),
        KeyAction::EngineHandled
    );
    assert_eq!(s.page(), 1);
    assert_eq!(
        handle_key(&mut s, KEY_PAGE_DOWN, 0),
        KeyAction::EngineHandled
    );
    assert_eq!(
        handle_key(&mut s, KEY_PAGE_DOWN, 0),
        KeyAction::EngineHandled
    ); // 钳制
    assert_eq!(s.page(), 2);
    assert_eq!(handle_key(&mut s, KEY_PAGE_UP, 0), KeyAction::EngineHandled);
    assert_eq!(s.page(), 1);
}
// ---- 修饰键与模式 ----
#[test]
fn ctrl_and_alt_combos_pass_through() {
    let mut s = pinyin_state();
    handle_key(&mut s, 'a' as u32, 0);
    assert_eq!(
        handle_key(&mut s, 'c' as u32, KEY_STATE_CTRL),
        KeyAction::PassThrough
    );
    assert_eq!(
        handle_key(&mut s, 'x' as u32, KEY_STATE_CTRL | KEY_STATE_SHIFT),
        KeyAction::PassThrough
    );
    assert_eq!(
        handle_key(&mut s, 'a' as u32, KEY_STATE_ALT),
        KeyAction::PassThrough
    );
    assert_eq!(s.buffer(), "a"); // 未被吞
}
#[test]
fn number_and_symbol_modes_pass_through() {
    let mut s = pinyin_state();
    s.switch_mode(Mode::Number);
    assert_eq!(handle_key(&mut s, '2' as u32, 0), KeyAction::PassThrough);
    assert_eq!(handle_key(&mut s, 'a' as u32, 0), KeyAction::PassThrough);
    s.switch_mode(Mode::Symbol);
    assert_eq!(handle_key(&mut s, 'a' as u32, 0), KeyAction::PassThrough);
    assert_eq!(s.buffer(), "");
}
#[test]
fn non_ascii_keyval_passes_through() {
    let mut s = pinyin_state();
    assert_eq!(handle_key(&mut s, 0x4e2d, 0), KeyAction::PassThrough); // 中
    assert_eq!(handle_key(&mut s, u32::MAX, 0), KeyAction::PassThrough);
    assert_eq!(s.buffer(), "");
}
#[test]
fn mode_switch_clears_buffer_then_english_direct() {
    // 拼音输入后切英文：缓冲清空，英文空缓冲直传
    let mut s = pinyin_state();
    handle_key(&mut s, 'a' as u32, 0);
    assert_eq!(s.buffer(), "a");
    s.switch_mode(Mode::English);
    assert_eq!(s.buffer(), "");
    assert_eq!(
        handle_key(&mut s, 'a' as u32, 0),
        KeyAction::Input("a".into())
    );
}
