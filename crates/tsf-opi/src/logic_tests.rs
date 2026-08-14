use super::*;
use engine_core::dictionary::InMemoryDictionary;

/// 20 个 "hao" 词条 + 引擎：确定性的 3 页候选。
fn state() -> TsfLogic {
    let mut d = InMemoryDictionary::new();
    for i in 0..20 {
        d.insert("hao", &format!("词{i:02}"), (5000 - i) as u32);
    }
    let symbols = engine_core::symbols::SymbolEngine::builtin();
    let mut s = TsfLogic {
        engine: Engine::new(Box::new(d), symbols, true),
        page: 0,
        buffer_snapshot: String::new(),
        shift_state: ShiftState::Off,
    };
    s.refresh_snapshot();
    s
}
fn pinyin_state() -> TsfLogic {
    let mut s = state();
    s.switch_mode(Mode::Pinyin);
    s
}
fn english_state() -> TsfLogic {
    let mut s = state();
    s.switch_mode(Mode::English);
    s
}
// ---- 装载：回退词库 / 坏路径（与 fcitx5-opi 同源） ----
#[test]
fn load_fallback_and_bad_path() {
    let mut s = TsfLogic::load(None).expect("fallback load");
    assert_eq!(s.buffer(), "");
    assert_eq!(s.mode(), Mode::Pinyin);
    // 空串等同 None（内置回退）
    assert!(TsfLogic::load(Some("")).is_ok());
    // 坏路径 → Err（load_or_fallback 原样语义）
    assert!(TsfLogic::load(Some("/nonexistent/opi.dict")).is_err());
    s.input_key('w' as u32, 0);
    assert_eq!(s.buffer(), "w");
}
// ---- 拼音模式：字母入缓冲 ----
#[test]
fn pinyin_letter_goes_to_engine_buffer() {
    let mut s = pinyin_state();
    assert_eq!(s.input_key('a' as u32, 0), KeyOutcome::CompositionChanged);
    assert_eq!(s.buffer(), "a");
    assert_eq!(s.input_key('b' as u32, 0), KeyOutcome::CompositionChanged);
    assert_eq!(s.buffer(), "ab");
}
#[test]
fn pinyin_uppercase_keysym_lowercased_into_buffer() {
    // 大写键值（物理 shift 已应用）；composer 转小写入缓冲
    let mut s = pinyin_state();
    assert_eq!(s.input_key('A' as u32, 0), KeyOutcome::CompositionChanged);
    assert_eq!(s.buffer(), "a");
}
#[test]
fn pinyin_symbol_unhandled() {
    let mut s = pinyin_state();
    assert_eq!(s.input_key('，' as u32, 0), KeyOutcome::Unhandled);
    assert_eq!(s.input_key(',' as u32, 0), KeyOutcome::Unhandled);
    assert_eq!(s.buffer(), "");
}
#[test]
fn pinyin_apostrophe_goes_to_buffer() {
    let mut s = pinyin_state();
    assert_eq!(s.input_key('\'' as u32, 0), KeyOutcome::CompositionChanged);
    assert_eq!(s.buffer(), "'");
}
// ---- 空格 ----
#[test]
fn space_with_buffer_commits_top_candidate() {
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        s.input_key(c as u32, 0);
    }
    assert_eq!(s.input_key(KEY_SPACE, 0), KeyOutcome::Commit("词00".into()));
    assert_eq!(s.buffer(), "");
}
#[test]
fn space_with_empty_buffer_commits_space() {
    let mut s = pinyin_state();
    assert_eq!(s.input_key(KEY_SPACE, 0), KeyOutcome::Commit(" ".into()));
    assert_eq!(s.buffer(), "");
}
// ---- 回车 ----
#[test]
fn enter_with_buffer_selects_first_candidate() {
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        s.input_key(c as u32, 0);
    }
    assert_eq!(
        s.input_key(KEY_RETURN, 0),
        KeyOutcome::Commit("词00".into())
    );
    assert_eq!(s.buffer(), "");
}
#[test]
fn enter_with_empty_buffer_unhandled() {
    let mut s = pinyin_state();
    assert_eq!(s.input_key(KEY_RETURN, 0), KeyOutcome::Unhandled);
}
// ---- 退格 ----
#[test]
fn backspace_with_buffer_deletes_codepoint() {
    let mut s = pinyin_state();
    for c in ['a', 'b'] {
        s.input_key(c as u32, 0);
    }
    assert_eq!(
        s.input_key(KEY_BACK_SPACE, 0),
        KeyOutcome::CompositionChanged
    );
    assert_eq!(s.buffer(), "a");
    assert_eq!(
        s.input_key(KEY_BACK_SPACE, 0),
        KeyOutcome::CompositionChanged
    );
    assert_eq!(s.buffer(), "");
    // 空缓冲 → 交应用
    assert_eq!(s.input_key(KEY_BACK_SPACE, 0), KeyOutcome::Unhandled);
}
#[test]
fn backspace_release_event_consumed() {
    let mut s = pinyin_state();
    s.input_key('a' as u32, 0);
    assert_eq!(
        s.input_key(KEY_BACK_SPACE, KEY_STATE_RELEASED),
        KeyOutcome::Consumed
    );
    assert_eq!(s.buffer(), "a");
}
// ---- Delete（与退格同一路由分支） ----
#[test]
fn delete_mirrors_backspace() {
    let mut s = pinyin_state();
    for c in ['a', 'b'] {
        s.input_key(c as u32, 0);
    }
    // 缓冲非空 → 引擎按码点删
    assert_eq!(s.input_key(KEY_DELETE, 0), KeyOutcome::CompositionChanged);
    assert_eq!(s.buffer(), "a");
    s.input_key(KEY_DELETE, 0);
    assert_eq!(s.buffer(), "");
    // 空缓冲 → 交应用
    assert_eq!(s.input_key(KEY_DELETE, 0), KeyOutcome::Unhandled);
    // 释放事件被消费，不删字符
    s.input_key('a' as u32, 0);
    assert_eq!(
        s.input_key(KEY_DELETE, KEY_STATE_RELEASED),
        KeyOutcome::Consumed
    );
    assert_eq!(s.buffer(), "a");
}
// ---- Tab / Esc：不拦截，交应用 ----
#[test]
fn tab_and_esc_unhandled() {
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        s.input_key(c as u32, 0);
    }
    assert_eq!(s.input_key(KEY_TAB, 0), KeyOutcome::Unhandled);
    assert_eq!(s.input_key(KEY_ESCAPE, 0), KeyOutcome::Unhandled);
    assert_eq!(s.buffer(), "hao"); // 缓冲未被吞
}
// ---- 英文模式直传（镜像 handleKey） ----
#[test]
fn english_empty_buffer_lowercase_commits() {
    let mut s = english_state();
    assert_eq!(s.input_key('a' as u32, 0), KeyOutcome::Commit("a".into()));
    assert_eq!(s.buffer(), "");
}
#[test]
fn english_empty_buffer_single_shift_commits_upper_and_consumes() {
    let mut s = english_state();
    s.shift_tap(); // Off → Single
    assert_eq!(s.input_key('a' as u32, 0), KeyOutcome::Commit("A".into()));
    // single 已消费：下个字母小写
    assert_eq!(s.input_key('b' as u32, 0), KeyOutcome::Commit("b".into()));
    assert_eq!(s.shift_state(), ShiftState::Off);
    assert_eq!(s.buffer(), "");
}
#[test]
fn english_empty_buffer_lock_keeps_uppercase() {
    let mut s = english_state();
    s.shift_long_press(); // Lock
    assert_eq!(s.input_key('a' as u32, 0), KeyOutcome::Commit("A".into()));
    // Lock 不被消费
    assert_eq!(s.input_key('b' as u32, 0), KeyOutcome::Commit("B".into()));
    assert_eq!(s.shift_state(), ShiftState::Lock);
    assert_eq!(s.buffer(), "");
}
#[test]
fn english_empty_buffer_physical_shift_keysym_commits_upper() {
    // 大写键值 + SHIFT 位，仍直传且消费 single
    let mut s = english_state();
    s.shift_tap();
    assert_eq!(
        s.input_key('A' as u32, KEY_STATE_SHIFT),
        KeyOutcome::Commit("A".into())
    );
    assert_eq!(s.shift_state(), ShiftState::Off);
}
#[test]
fn english_letters_always_direct_commit_buffer_stays_empty() {
    // 镜像 KeyRouter.handleKey：英文模式空缓冲每次直传，缓冲永远为空，
    // 故字母永不入引擎缓冲（与 Android 行为一致）
    let mut s = english_state();
    assert_eq!(s.input_key('a' as u32, 0), KeyOutcome::Commit("a".into()));
    assert_eq!(s.input_key('b' as u32, 0), KeyOutcome::Commit("b".into()));
    assert_eq!(s.buffer(), "");
    // 符号直传
    assert_eq!(s.input_key(',' as u32, 0), KeyOutcome::Commit(",".into()));
}
#[test]
fn english_nonempty_buffer_letter_goes_to_engine() {
    // 防御路径：仅当缓冲意外非空（如经 engine 直入）时才入引擎，
    // 镜像 KeyRouter 的 else 分支
    let mut s = english_state();
    s.engine.input_key('x'); // 绕过路由器直入引擎缓冲
    assert_eq!(s.buffer(), "x");
    assert_eq!(s.input_key('b' as u32, 0), KeyOutcome::CompositionChanged);
    assert_eq!(s.buffer(), "xb");
    // 缓冲非空时符号交应用
    assert_eq!(s.input_key(',' as u32, 0), KeyOutcome::Unhandled);
}
#[test]
fn english_nonempty_buffer_shift_uppercases_in_engine() {
    // 引擎 shift 打开时，非空缓冲路径由 composer 大写（镜像同一分支）
    let mut s = english_state();
    s.engine.input_key('x');
    s.shift_tap();
    assert_eq!(s.input_key('b' as u32, 0), KeyOutcome::CompositionChanged);
    assert_eq!(s.buffer(), "xB");
}
// ---- ⇧ 状态机（镜像 EngineController.shiftTap） ----
#[test]
fn shift_tap_cycles_off_single_off() {
    let mut s = english_state();
    assert_eq!(s.input_key(KEY_SHIFT, 0), KeyOutcome::Consumed);
    assert_eq!(s.shift_state(), ShiftState::Single);
    assert_eq!(s.input_key(KEY_SHIFT, 0), KeyOutcome::Consumed);
    assert_eq!(s.shift_state(), ShiftState::Off);
}
#[test]
fn shift_release_and_repeat_do_not_toggle() {
    let mut s = english_state();
    assert_eq!(
        s.input_key(KEY_SHIFT, KEY_STATE_RELEASED),
        KeyOutcome::Consumed
    );
    assert_eq!(s.shift_state(), ShiftState::Off);
    s.shift_tap();
    assert_eq!(
        s.input_key(KEY_SHIFT, KEY_STATE_REPEAT),
        KeyOutcome::Consumed
    );
    assert_eq!(s.shift_state(), ShiftState::Single);
}
#[test]
fn shift_long_press_locks() {
    let mut s = english_state();
    assert_eq!(
        s.input_key(KEY_SHIFT, KEY_STATE_LONG_PRESSED),
        KeyOutcome::Consumed
    );
    assert_eq!(s.shift_state(), ShiftState::Lock);
}
// ---- 候选选择与翻页 ----
#[test]
fn digit_selects_candidate_page_relative() {
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        s.input_key(c as u32, 0);
    }
    // '2' → 页内索引 1
    assert_eq!(
        s.input_key('2' as u32, 0),
        KeyOutcome::Commit("词01".into())
    );
    assert_eq!(s.buffer(), "");
}
#[test]
fn digit_one_selects_first_candidate() {
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        s.input_key(c as u32, 0);
    }
    // '1' → 页内索引 0，选中首候选
    assert_eq!(
        s.input_key('1' as u32, 0),
        KeyOutcome::Commit("词00".into())
    );
    assert_eq!(s.buffer(), "");
}
#[test]
fn digit_out_of_range_unhandled() {
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        s.input_key(c as u32, 0);
    }
    s.next_page();
    s.next_page(); // 末页 4 个候选（词16..词19）
    // '9' → 页内索引 8 越界 → 交应用
    assert_eq!(s.input_key('9' as u32, 0), KeyOutcome::Unhandled);
    // '0' → 无索引 → 交应用
    assert_eq!(s.input_key('0' as u32, 0), KeyOutcome::Unhandled);
}
#[test]
fn digit_without_candidates_unhandled() {
    let mut s = pinyin_state();
    // 无候选（缓冲空 / 无词条缓冲）
    assert_eq!(s.input_key('1' as u32, 0), KeyOutcome::Unhandled);
    s.input_key('x' as u32, 0);
    s.input_key('x' as u32, 0); // "xx" 无候选
    assert_eq!(s.input_key('1' as u32, 0), KeyOutcome::Unhandled);
}
#[test]
fn page_keys_navigate() {
    let mut s = pinyin_state();
    for c in ['h', 'a', 'o'] {
        s.input_key(c as u32, 0);
    }
    assert_eq!(
        s.input_key(KEY_PAGE_DOWN, 0),
        KeyOutcome::CompositionChanged
    );
    assert_eq!(s.page(), 1);
    assert_eq!(
        s.input_key(KEY_PAGE_DOWN, 0),
        KeyOutcome::CompositionChanged
    );
    assert_eq!(
        s.input_key(KEY_PAGE_DOWN, 0),
        KeyOutcome::CompositionChanged
    ); // 钳制
    assert_eq!(s.page(), 2);
    assert_eq!(s.input_key(KEY_PAGE_UP, 0), KeyOutcome::CompositionChanged);
    assert_eq!(s.page(), 1);
}
// ---- 修饰键与模式 ----
#[test]
fn ctrl_and_alt_combos_unhandled() {
    let mut s = pinyin_state();
    s.input_key('a' as u32, 0);
    assert_eq!(
        s.input_key('c' as u32, KEY_STATE_CTRL),
        KeyOutcome::Unhandled
    );
    assert_eq!(
        s.input_key('x' as u32, KEY_STATE_CTRL | KEY_STATE_SHIFT),
        KeyOutcome::Unhandled
    );
    assert_eq!(
        s.input_key('a' as u32, KEY_STATE_ALT),
        KeyOutcome::Unhandled
    );
    assert_eq!(s.buffer(), "a"); // 未被吞
}
#[test]
fn number_and_symbol_modes_unhandled() {
    let mut s = pinyin_state();
    s.switch_mode(Mode::Number);
    assert_eq!(s.input_key('2' as u32, 0), KeyOutcome::Unhandled);
    assert_eq!(s.input_key('a' as u32, 0), KeyOutcome::Unhandled);
    s.switch_mode(Mode::Symbol);
    assert_eq!(s.input_key('a' as u32, 0), KeyOutcome::Unhandled);
    assert_eq!(s.buffer(), "");
}
#[test]
fn non_ascii_keyval_unhandled() {
    let mut s = pinyin_state();
    assert_eq!(s.input_key(0x4e2d, 0), KeyOutcome::Unhandled); // 中
    assert_eq!(s.input_key(u32::MAX, 0), KeyOutcome::Unhandled);
    assert_eq!(s.buffer(), "");
}
#[test]
fn mode_switch_clears_buffer_then_english_direct() {
    // 拼音输入后切英文：缓冲清空，英文空缓冲直传
    let mut s = pinyin_state();
    s.input_key('a' as u32, 0);
    assert_eq!(s.buffer(), "a");
    s.switch_mode(Mode::English);
    assert_eq!(s.buffer(), "");
    assert_eq!(s.input_key('a' as u32, 0), KeyOutcome::Commit("a".into()));
}
// ---- 候选分页状态机（与 fcitx5-opi candidate.rs 同源） ----
#[test]
fn eight_candidates_per_page_and_page_count() {
    let mut s = state();
    s.input_key('h' as u32, 0);
    s.input_key('a' as u32, 0);
    s.input_key('o' as u32, 0);
    assert_eq!(s.buffer(), "hao");
    assert_eq!(s.candidates().len(), PAGE_SIZE);
    assert_eq!(s.page_count(), 3);
    assert_eq!(s.candidates()[0], "词00");
    assert_eq!(s.candidates()[7], "词07");
}
#[test]
fn paging_clamps_both_ends() {
    let mut s = state();
    for c in ['h', 'a', 'o'] {
        s.input_key(c as u32, 0);
    }
    // 首页 prev 钳制
    assert_eq!(s.prev_page(), 0);
    // next → 1 → 2（末页）
    assert_eq!(s.next_page(), 1);
    assert_eq!(s.next_page(), 2);
    assert_eq!(s.candidates()[0], "词16");
    // 末页 next 钳制
    assert_eq!(s.next_page(), 2);
    // set_page 双向钳制
    assert_eq!(s.set_page(99), 2);
    assert_eq!(s.set_page(0), 0);
}
#[test]
fn select_is_page_relative_and_commits() {
    let mut s = state();
    for c in ['h', 'a', 'o'] {
        s.input_key(c as u32, 0);
    }
    s.next_page(); // 第 2 页（global 8..16）
    assert_eq!(s.select(0), "词08");
    // 提交后 buffer 清空、页码归零
    assert_eq!(s.buffer(), "");
    assert_eq!(s.page(), 0);
}
#[test]
fn select_out_of_range_returns_empty() {
    let mut s = state();
    for c in ['h', 'a', 'o'] {
        s.input_key(c as u32, 0);
    }
    s.set_page(2); // 第 3 页仅 4 个候选（16..19）
    assert_eq!(s.select(7), "");
}
#[test]
fn set_shift_clamps_page() {
    let mut s = state();
    for c in ['h', 'a', 'o'] {
        s.input_key(c as u32, 0);
    }
    s.set_page(2); // 末页（3 页候选）
    assert_eq!(s.page(), 2);
    s.shift_tap(); // buffer 不变，页码须钳制在 page_count 内
    assert!(s.page() <= s.page_count().saturating_sub(1));
    assert!(!s.candidates().is_empty());
    s.shift_tap();
    assert!(s.page() <= s.page_count().saturating_sub(1));
    assert!(!s.candidates().is_empty());
}
#[test]
fn buffer_change_resets_page() {
    let mut s = state();
    for c in ['h', 'a', 'o'] {
        s.input_key(c as u32, 0);
    }
    s.next_page();
    assert_eq!(s.page(), 1);
    // 继续输入（buffer 变化）→ 页码归零
    s.input_key('x' as u32, 0);
    assert_eq!(s.buffer(), "haox");
    assert_eq!(s.page(), 0);
}
#[test]
fn backspace_and_clear_reset_page() {
    let mut s = state();
    for c in ['h', 'a', 'o'] {
        s.input_key(c as u32, 0);
    }
    s.next_page();
    s.input_key(KEY_BACK_SPACE, 0); // buffer 变化 → 归零
    assert_eq!(s.page(), 0);
    s.input_key('o' as u32, 0);
    s.next_page();
    s.clear(); // buffer 清空 → 归零
    assert_eq!(s.page(), 0);
    assert_eq!(s.buffer(), "");
}
#[test]
fn shift_machine_off_single_lock_cycle() {
    let mut s = state();
    assert_eq!(s.shift_state(), ShiftState::Off);
    // 单击：Off→Single
    s.shift_tap();
    assert_eq!(s.shift_state(), ShiftState::Single);
    // 再单击：Single→Off
    s.shift_tap();
    assert_eq!(s.shift_state(), ShiftState::Off);
    // 长按：Lock；单击：Lock→Off（镜像 EngineController.shiftTap else 分支）
    s.shift_long_press();
    assert_eq!(s.shift_state(), ShiftState::Lock);
    s.shift_tap();
    assert_eq!(s.shift_state(), ShiftState::Off);
    // single 消费后复位；lock 不受消费影响
    s.shift_tap();
    assert_eq!(s.shift_state(), ShiftState::Single);
    s.consume_single_shift();
    assert_eq!(s.shift_state(), ShiftState::Off);
    s.shift_long_press();
    s.consume_single_shift();
    assert_eq!(s.shift_state(), ShiftState::Lock);
}
#[test]
fn empty_buffer_has_no_pages() {
    let mut s = state();
    assert_eq!(s.candidates(), Vec::<String>::new());
    assert_eq!(s.page_count(), 0);
    assert_eq!(s.next_page(), 0);
    assert_eq!(s.set_page(5), 0);
}
