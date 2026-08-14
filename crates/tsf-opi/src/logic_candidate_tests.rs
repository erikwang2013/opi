//! 候选分页状态机测试（与 fcitx5-opi candidate.rs 测试同源）。
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
