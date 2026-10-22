// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态推送快照：把 `sink::push_state` 需要的引擎数据在 **logic 锁内一次抓完**，
//! 让管道写（可能被候选窗/宿主拖住）发生在锁外。
//!
//! 2026-10-22 真机报障「电脑物理键盘功能键无效并卡死，只能输入英文」：按键线程
//! 持着 logic 锁调 `sink`（命名管道写）。候选窗进程一慢，按键线程连同宿主 UI
//! 线程一起冻住 —— 本输入法是 in-proc，宿主（Word/浏览器）跟着无响应。
//! 锁内只允许碰引擎；`snapshot()` 是它的唯一出口，抓完即可放锁。
//!
//! 从 `logic.rs` 拆出守 500 行；纯逻辑、主机可单测（见文件末）。

use engine_core::candidates::Candidate;
use engine_core::composer::Mode;

use crate::logic::{PAGE_SIZE, TsfLogic};

/// 一次「推给候选窗」所需的全部数据。`composing` 供调用方判断要不要
/// 取 caret（空缓冲时 `push_state` 马上 hide，取位置是白花一次同步 COM 会话）。
pub struct StateSnapshot {
    pub buffer: String,
    pub candidates: Vec<String>,
    pub page: usize,
    pub page_count: usize,
    pub mode: Mode,
    pub composing: bool,
}

impl TsfLogic {
    /// 作废一次抓取缓存。编辑/提交经 `reset_page_if_buffer_changed`；设置位入口在
    /// `logic_settings.rs` / `logic_shift.rs` 各自调（它们改候选集但不动 buffer）。
    pub(crate) fn invalidate_fetch_cache(&mut self) {
        self.fetch_cache.get_mut().take();
    }

    /// 取走与当前 (buffer, pending) 匹配的缓存（**一次性**）。
    pub(crate) fn take_fetch_cache(&self) -> Option<Vec<Candidate>> {
        let key = self.fetch_key();
        let mut slot = self.fetch_cache.borrow_mut();
        let hit = slot
            .as_ref()
            .is_some_and(|(b, p, _)| b == &key.0 && p == &key.1);
        if hit {
            slot.take().map(|(_, _, list)| list)
        } else {
            // 键不匹配的旧表不再可能有用（编辑/设置位都走失效），别留着。
            slot.take();
            None
        }
    }

    /// 缓存命中则只数条数，不重排。
    pub(crate) fn cached_page_count(&self) -> Option<usize> {
        let key = self.fetch_key();
        let cache = self.fetch_cache.borrow();
        let (b, p, list) = cache.as_ref()?;
        (b == &key.0 && p == &key.1).then(|| list.len().div_ceil(PAGE_SIZE))
    }

    /// 留下全量表给紧接着的 `fetched()`，返回页数。
    pub(crate) fn store_fetch_cache(&self, list: Vec<Candidate>) -> usize {
        let count = list.len().div_ceil(PAGE_SIZE);
        let key = self.fetch_key();
        *self.fetch_cache.borrow_mut() = Some((key.0, key.1, list));
        count
    }

    /// 缓存键与 fcitx5 轨同口径：真缓冲 + 已确认的逐字片段。
    fn fetch_key(&self) -> (String, String) {
        (self.engine.buffer().to_string(), self.engine.pending_text())
    }

    /// 锁内调用，**只碰引擎**（不碰 sink/COM）。一次 `fetched()` 同时算候选与页数
    /// （2026-10-18：此前各算一次，每键整份排序两遍）。
    pub fn snapshot(&self) -> StateSnapshot {
        let fetched = self.fetched();
        let candidates: Vec<String> = fetched
            .iter()
            .skip(self.page * PAGE_SIZE)
            .take(PAGE_SIZE)
            .map(|c| c.text.clone())
            .collect();
        StateSnapshot {
            buffer: self.display_buffer(),
            candidates,
            page: self.page,
            page_count: fetched.len().div_ceil(PAGE_SIZE),
            mode: self.mode(),
            composing: self.has_composing(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::ShiftState;
    use engine_core::Engine;
    use engine_core::dictionary::InMemoryDictionary;

    /// 20 条 "qwx"（3 页）+ 缓冲 qwx —— 与 logic_candidate_tests 的确定性夹具同形。
    fn state() -> TsfLogic {
        let mut d = InMemoryDictionary::new();
        for i in 0..20 {
            d.insert("qwx", &format!("词{i:02}"), (5000 - i) as u32);
        }
        let symbols = engine_core::symbols::SymbolEngine::builtin();
        let mut s = TsfLogic {
            engine: Engine::new(Box::new(d), symbols, true),
            page: 0,
            buffer_snapshot: String::new(),
            shift_state: ShiftState::Off,
            english_pro: true,
            last_printable: None,
            shift_toggle: true,
            fetch_cache: Default::default(),
        };
        s.refresh_snapshot();
        for c in ['q', 'w', 'x'] {
            s.input_key(c as u32, 0);
        }
        s
    }

    #[test]
    fn snapshot_carries_page_candidates_and_buffer() {
        let mut s = state();
        s.next_page();
        let snap = s.snapshot();
        // 显示串走 Engine::display_buffer（全键盘自动分词）：qwx → q'w'x；行为判定
        // 仍读 engine.buffer()，分段只影响候选窗上那一行拼音。
        assert_eq!(snap.buffer, "q'w'x");
        assert_eq!(s.engine.buffer(), "qwx");
        assert_eq!(snap.page, 1);
        assert_eq!(snap.page_count, 3);
        assert_eq!(snap.candidates, &s.candidates()[..]);
        assert_eq!(snap.candidates[0], "词08");
        assert_eq!(snap.mode, Mode::Pinyin);
        assert!(snap.composing);
    }

    #[test]
    fn empty_engine_snapshot_is_not_composing() {
        let snap = TsfLogic::load(None).expect("内置回退词库").snapshot();
        assert_eq!(snap.buffer, "");
        assert!(snap.candidates.is_empty());
        assert_eq!(snap.page_count, 0);
        assert!(!snap.composing);
    }
}
