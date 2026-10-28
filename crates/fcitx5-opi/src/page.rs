// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 候选翻页的 C 出口（2026-10-28）：UI 的翻页箭头（classicui 点击/滚轮、
//! D-Bus `NextPage`/`PrevPage`）此前是空操作，候选恒单页。**页码真源仍是 Rust**
//! （`CandidateState::page`）：C++ 的 `OpiCandidateList` 只把 hasPrev/hasNext/
//! prev/next 转成这两个出口，再用刷新后的当前页重建列表 —— 不把整份候选交给
//! `CommonCandidateList` 自己翻，避免两份页码漂移。
//!
//! 顺序承重：`state_of` 先 `page_count()`（全量取回并留下一次性缓存），宿主随后
//! `opi_fcitx5_candidates()` 命中同一缓存 —— 一次翻页只取回一次。

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::candidate::CandidateState;
use crate::{ensure_panic_hook, with_state};

/// Rust 侧与 `opi_rust_ffi.h` 的 `OpiPageState` 手工对齐。
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct OpiPageState {
    /// 当前页（0 起）。
    pub page: i32,
    /// 总页数；0 = 无候选。
    pub pages: i32,
}

fn state_of(s: &CandidateState) -> OpiPageState {
    let pages = s.page_count();
    OpiPageState {
        page: s.page() as i32,
        pages: pages as i32,
    }
}

/// 前进/后退一页（`delta` 只取符号），越界钳制；返回翻页后的状态。
fn step_of(s: &mut CandidateState, delta: i32) -> OpiPageState {
    if delta > 0 {
        s.next_page();
    } else if delta < 0 {
        s.prev_page();
    }
    state_of(s)
}

/// 当前页/总页数。未装载 → {0, 0}。
/// # Safety
///
/// 无指针参数；共享单例由内部 Mutex 保护。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_page_state() -> OpiPageState {
    ensure_panic_hook();
    catch_unwind(AssertUnwindSafe(|| {
        with_state(|s| state_of(s)).unwrap_or_default()
    }))
    .unwrap_or_default()
}

/// 翻一页（delta>0 下一页，<0 上一页，0 只读），返回翻页后的状态。
/// # Safety
///
/// 无指针参数；共享单例由内部 Mutex 保护。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_page_step(delta: i32) -> OpiPageState {
    ensure_panic_hook();
    catch_unwind(AssertUnwindSafe(|| {
        with_state(|s| step_of(s, delta)).unwrap_or_default()
    }))
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::composer::Mode;
    use engine_core::dictionary::InMemoryDictionary;

    /// 20 个 "qwx" 词条 → 3 页（与 candidate_tests.rs 的 state() 同惯例）。
    fn state() -> CandidateState {
        let mut d = InMemoryDictionary::new();
        for i in 0..20 {
            d.insert("qwx", &format!("词{i:02}"), (5000 - i) as u32);
        }
        let symbols = engine_core::symbols::SymbolEngine::builtin();
        let mut s = CandidateState {
            engine: engine_core::Engine::new(Box::new(d), symbols, true),
            page: 0,
            buffer_snapshot: String::new(),
            shift_state: crate::candidate::ShiftState::Off,
            held_passthrough: Default::default(),
            dicts: Default::default(),
            english_pro: true,
            fetch_cache: Default::default(),
            persist: Default::default(),
        };
        s.switch_mode(Mode::Pinyin);
        for c in ['q', 'w', 'x'] {
            s.input_key(c);
        }
        s.buffer_snapshot = s.engine.buffer().to_string();
        s
    }

    #[test]
    fn step_moves_page_and_clamps() {
        let mut s = state();
        assert_eq!((state_of(&s).page, state_of(&s).pages), (0, 3));
        assert_eq!(step_of(&mut s, 1).page, 1);
        // 翻页后紧跟的 candidates() 命中同一缓存且是第 2 页首条（一次取回）
        assert_eq!(s.candidates()[0], "词08");
        assert_eq!(step_of(&mut s, 1).page, 2);
        assert_eq!(step_of(&mut s, 1).page, 2, "末页再前进须钳制");
        assert_eq!(step_of(&mut s, -1).page, 1);
        assert_eq!(step_of(&mut s, 0).page, 1, "delta=0 只读");
    }

    #[test]
    fn no_candidates_is_page_zero_of_zero() {
        let s = CandidateState::load(None).expect("内置回退词库");
        let st = state_of(&s);
        assert_eq!((st.page, st.pages), (0, 0));
    }
}
