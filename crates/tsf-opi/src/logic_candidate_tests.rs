// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

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
        last_printable: None,
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

/// 只数 `query` 的字典包装（`query_all` 走 trait 默认实现 → `query`）；与
/// engine-core 侧 `tests/router_invariants.rs` 同形，两轨各一枚。
struct Counting {
    inner: InMemoryDictionary,
    n: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl engine_core::dictionary::Dictionary for Counting {
    fn query(&self, pinyin: &str, limit: usize) -> Vec<engine_core::trie::Entry> {
        self.n.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.inner.query(pinyin, limit)
    }
    fn len(&self) -> usize {
        self.inner.len()
    }
    fn max_freq(&self) -> u64 {
        self.inner.max_freq()
    }
}

/// 数字选词旧路径抓两次候选表：`candidates()` 一次，`select()` 内 `engine.select`
/// 又一次 —— 每次选词白付一次全量排序。抓取次数是计数式指标，比时间稳
/// （同一台机器上还有别的编译在跑）。改回两次即红。
#[test]
fn digit_select_fetches_candidate_table_once() {
    use std::sync::atomic::Ordering;
    let n = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut d = InMemoryDictionary::new();
    for i in 0..20 {
        d.insert("hao", &format!("词{i:02}"), (5000 - i) as u32);
    }
    let mut s = TsfLogic {
        engine: Engine::new(
            Box::new(Counting {
                inner: d,
                n: n.clone(),
            }),
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
    for c in ['h', 'a', 'o'] {
        s.input_key(c as u32, 0);
    }
    // 基准：读一次当前页候选表要查几次词库（= 一次全量抓取）
    n.store(0, Ordering::Relaxed);
    let _ = s.candidates();
    let one_fetch = n.load(Ordering::Relaxed);
    assert!(one_fetch > 0, "基准抓取没查到词库，用例失效");
    n.store(0, Ordering::Relaxed);
    assert_eq!(
        s.input_key('1' as u32, 0),
        KeyOutcome::Commit("词00".to_string())
    );
    assert_eq!(
        n.load(Ordering::Relaxed),
        one_fetch,
        "数字选词应只抓一次候选表（两次 = 整表排两遍）"
    );
}
