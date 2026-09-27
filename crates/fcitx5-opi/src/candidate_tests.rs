// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 候选分页状态机测试（与 tsf-opi logic.rs 测试同源）。
use super::*;
use engine_core::dictionary::InMemoryDictionary;

/// 20 个 "hao" 词条 → 确定性的 3 页候选（20 / 8 = 2.5 → 3 页）。
fn state() -> CandidateState {
    let mut d = InMemoryDictionary::new();
    for i in 0..20 {
        d.insert("hao", &format!("词{i:02}"), (5000 - i) as u32);
    }
    let symbols = engine_core::symbols::SymbolEngine::builtin();
    let mut s = CandidateState {
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
    let mut s = CandidateState::load(None).expect("fallback load");
    assert_eq!(s.buffer(), "");
    assert_eq!(s.mode(), Mode::Pinyin);
    // 空串等同 None（内置回退）
    assert!(CandidateState::load(Some("")).is_ok());
    // 坏路径 → Err（load_or_fallback 原样语义）
    assert!(CandidateState::load(Some("/nonexistent/opi.dict")).is_err());
    s.input_key('w');
    assert_eq!(s.buffer(), "w");
}

#[test]
fn eight_candidates_per_page_and_page_count() {
    let mut s = state();
    s.input_key('h');
    s.input_key('a');
    s.input_key('o');
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
        s.input_key(c);
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
        s.input_key(c);
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
        s.input_key(c);
    }
    s.set_page(2); // 第 3 页仅 4 个候选（16..19）
    assert_eq!(s.select(7), "");
}

/// 20 条 "hao" 候选、缓冲已置好的状态 —— 提交成功会清缓冲，故每个断言各建一个
/// （同 `engine-core/tests/select_index_bounds.rs` 的 `router_with(20)` + `type_hao`）。
/// 符号表取**空表**：候选恰为 20 条（20 = 8+8+4，「末页 4 项」这类前提才确定）——
/// 上面 `state()` 用的是内建符号表，`hao` 会多出 2 条符号候选（实测 22 条）。
fn hao_state() -> CandidateState {
    let mut d = InMemoryDictionary::new();
    for i in 0..20 {
        d.insert("hao", &format!("词{i:02}"), (5000 - i) as u32);
    }
    let symbols = engine_core::symbols::SymbolEngine::new(Vec::new(), Vec::new());
    let mut s = CandidateState {
        engine: Engine::new(Box::new(d), symbols, true),
        page: 0,
        buffer_snapshot: String::new(),
        shift_state: ShiftState::Off,
        last_printable: None,
    };
    for c in ['h', 'a', 'o'] {
        s.engine.input_key(c);
    }
    s.refresh_snapshot();
    s
}

/// 回归（本轮缺陷）：**页内**下标入口 `select`（候选窗点选：宿主交进来的是页内下标）的
/// 越界判据曾只看**全量**列表 —— 页内越界（第 0 页只有 8 项时点第 9 项，k=8）被换算成
/// **次页**下标照常提交，用户屏幕上根本看不见那一条。判据本轮提到 `select_from`
/// （数字键 / 回车 / 点选三条路的公共下游），本用例钉点选那条。
///
/// ⚠️ 别拿「全量越界」当判据（`select_out_of_range_returns_empty` 那种，末页 k=7 全量也越界）：
/// 那种用例在旧实现下也返回空串，**抓不到本缺陷**。故下面先钉住全量第 8 项**确实存在**。
#[test]
fn select_beyond_page_returns_empty() {
    // 页内正常项不受影响（先钉住不误伤）
    let mut s = hao_state();
    assert_eq!(s.select(0), "词00", "第 0 页首位");
    let mut s = hao_state();
    assert_eq!(
        s.select(PAGE_SIZE - 1),
        "词07",
        "第 0 页末位（页内最大合法下标）"
    );

    // 页内越界 k=8：全量列表里第 8 项**存在**（就是旧实现提交的那一条）
    let mut s = hao_state();
    assert_eq!(s.page(), 0);
    assert_eq!(s.candidates().len(), PAGE_SIZE, "前置：第 0 页是满页 8 条");
    let full: Vec<String> = s.fetched().into_iter().map(|c| c.text).collect();
    assert_eq!(
        full[PAGE_SIZE], "词08",
        "前置：全量第 8 项确实存在（页内越界与全量越界因此必须分开判）"
    );
    assert_eq!(
        s.select(PAGE_SIZE),
        "",
        "页内没有第 9 项 → 空串；实际提交了 {:?}（次页首条，用户看不见）",
        full[PAGE_SIZE]
    );
    assert_eq!(s.buffer(), "hao", "越界不得提交 ⇒ 缓冲不变");
    assert_eq!(s.page(), 0, "越界不得改页码");

    // 第 1 页同样：k=8 → 全局 16（第 2 页首条）也存在，必须空串
    let mut s = hao_state();
    s.set_page(1);
    assert_eq!(s.select(PAGE_SIZE), "", "第 1 页的页内越界同样必须空串");
    assert_eq!(s.buffer(), "hao");
    // 而第 1 页的页内合法项照旧
    let mut s = hao_state();
    s.set_page(1);
    assert_eq!(s.select(0), "词08", "第 1 页首位 = 全量第 8 项");
    let mut s = hao_state();
    s.set_page(1);
    assert_eq!(s.select(PAGE_SIZE - 1), "词15", "第 1 页末位");

    // 末页只有 4 项（20 = 8+8+4）：k=3 合法、k=4 页内越界（此处全量也越界）
    let mut s = hao_state();
    s.set_page(2);
    assert_eq!(s.candidates().len(), 4, "前置：末页不满");
    assert_eq!(s.select(3), "词19", "末页最后一项");
    let mut s = hao_state();
    s.set_page(2);
    assert_eq!(s.select(4), "", "末页第 5 项（页内越界）");
    assert_eq!(s.buffer(), "hao");
}

#[test]
fn set_shift_clamps_page() {
    let mut s = state();
    for c in ['h', 'a', 'o'] {
        s.input_key(c);
    }
    s.set_page(2); // 末页（3 页候选）
    assert_eq!(s.page(), 2);
    s.set_shift(true); // buffer 不变，页码须钳制在 page_count 内
    assert!(s.page() <= s.page_count().saturating_sub(1));
    assert!(!s.candidates().is_empty());
    s.set_shift(false);
    assert!(s.page() <= s.page_count().saturating_sub(1));
    assert!(!s.candidates().is_empty());
}

#[test]
fn buffer_change_resets_page() {
    let mut s = state();
    for c in ['h', 'a', 'o'] {
        s.input_key(c);
    }
    s.next_page();
    assert_eq!(s.page(), 1);
    // 继续输入（buffer 变化）→ 页码归零
    s.input_key('x');
    assert_eq!(s.buffer(), "haox");
    assert_eq!(s.page(), 0);
}

#[test]
fn backspace_and_clear_reset_page() {
    let mut s = state();
    for c in ['h', 'a', 'o'] {
        s.input_key(c);
    }
    s.next_page();
    s.backspace(); // buffer 变化 → 归零
    assert_eq!(s.page(), 0);
    s.input_key('o');
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
    use crate::input_method::{KeyAction, handle_key};
    use std::sync::atomic::Ordering;
    let n = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut d = InMemoryDictionary::new();
    for i in 0..20 {
        d.insert("hao", &format!("词{i:02}"), (5000 - i) as u32);
    }
    let mut s = CandidateState {
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
        handle_key(&mut s, c as u32, 0);
    }
    // 基准：读一次当前页候选表要查几次词库（= 一次全量抓取）
    n.store(0, Ordering::Relaxed);
    let _ = s.candidates();
    let one_fetch = n.load(Ordering::Relaxed);
    assert!(one_fetch > 0, "基准抓取没查到词库，用例失效");
    n.store(0, Ordering::Relaxed);
    assert_eq!(
        handle_key(&mut s, '1' as u32, 0),
        KeyAction::Input("词00".to_string())
    );
    assert_eq!(
        n.load(Ordering::Relaxed),
        one_fetch,
        "数字选词应只抓一次候选表（两次 = 整表排两遍）"
    );
}
