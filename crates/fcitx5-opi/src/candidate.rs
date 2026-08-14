//! 候选翻页状态：包装 engine_core::Engine，持有候选分页状态。
//!
//! 语义与 Android 侧一致：每页 8 个候选，一次最多抓取 FETCH_LIMIT 条
//! （Android fetchLimit=64 的对应物），页码越界钳制，buffer 变化时页码
//! 归零。纯逻辑结构体（无全局状态、无 FFI），可独立单测；
//! C 出口（lib.rs）经 Mutex 单例访问本状态。

use engine_core::Engine;
use engine_core::candidates::Candidate;
use engine_core::composer::Mode;

/// 每页候选数（与 Android 候选栏一致）。
pub const PAGE_SIZE: usize = 8;
/// 一次抓取的候选批量上限（对应 Android 侧 fetchLimit=64）。
pub const FETCH_LIMIT: usize = 64;

/// 引擎 + 候选分页状态。
pub struct CandidateState {
    engine: Engine,
    /// 当前页（0 起）。
    page: usize,
    /// 上次操作后的 buffer 快照；变化时页码归零。
    buffer_snapshot: String,
}

impl CandidateState {
    /// 装载引擎。`None`/空串 → 内置回退词库（35 词）；非空路径 →
    /// load_or_fallback 原样语义（坏路径返回 Err）。语义与 opi-ffi
    /// api::install 一致（学习默认开）。
    pub fn load(path: Option<&str>) -> Result<Self, String> {
        let dict: Box<dyn engine_core::dictionary::Dictionary> = match path {
            Some(p) if !p.is_empty() => {
                engine_data::load_or_fallback(Some(std::path::Path::new(p)))?
            }
            _ => Box::new(engine_data::fallback_dict()),
        };
        let symbols = engine_core::symbols::SymbolEngine::builtin();
        let mut s = CandidateState {
            engine: Engine::new(dict, symbols, true),
            page: 0,
            buffer_snapshot: String::new(),
        };
        s.refresh_snapshot();
        Ok(s)
    }

    fn refresh_snapshot(&mut self) {
        self.buffer_snapshot = self.engine.buffer().to_string();
    }

    /// buffer 与快照不一致 → 页码归零（与 Android 语义一致），再刷新快照。
    fn reset_page_if_buffer_changed(&mut self) {
        if self.engine.buffer() != self.buffer_snapshot {
            self.page = 0;
        }
        self.refresh_snapshot();
    }

    pub fn buffer(&self) -> String {
        self.engine.buffer().to_string()
    }

    pub fn mode(&self) -> Mode {
        self.engine.mode()
    }

    /// 输入单字符键。空串/多字符由调用方（input_method）拒绝；
    /// 非 ASCII 由引擎层拒绝。返回引擎输出（如英文模式下已提交文本）。
    pub fn input_key(&mut self, ch: char) -> String {
        let out = self.engine.input_key(ch);
        self.reset_page_if_buffer_changed();
        out
    }

    pub fn input_space(&mut self) -> String {
        let out = self.engine.input_space();
        self.reset_page_if_buffer_changed();
        out
    }

    pub fn backspace(&mut self) {
        self.engine.backspace();
        self.reset_page_if_buffer_changed();
    }

    pub fn clear(&mut self) {
        self.engine.clear();
        self.reset_page_if_buffer_changed();
    }

    pub fn switch_mode(&mut self, mode: Mode) {
        self.engine.switch_mode(mode);
        self.reset_page_if_buffer_changed();
    }

    pub fn set_shift(&mut self, on: bool) {
        self.engine.set_shift(on);
        // shift 不改变 buffer（reset_page_if_buffer_changed 不会触发），但
        // 可能改变候选集；将页码钳制到当前 page_count 边界，防 page 越界。
        self.set_page(self.page);
    }

    /// 提交当前页第 `index` 个候选（页内索引，0 起）。越界返回空串。
    pub fn select(&mut self, index: usize) -> String {
        let global = self.page * PAGE_SIZE + index;
        let out = self.engine.select(global);
        self.reset_page_if_buffer_changed();
        out
    }

    /// 批量抓取（FETCH_LIMIT 内，engine 全量排序后截断）。
    fn fetched(&self) -> Vec<Candidate> {
        self.engine.candidates(FETCH_LIMIT)
    }

    /// 当前页候选文本（最多 PAGE_SIZE 条）。
    pub fn candidates(&self) -> Vec<String> {
        self.fetched()
            .iter()
            .skip(self.page * PAGE_SIZE)
            .take(PAGE_SIZE)
            .map(|c| c.text.clone())
            .collect()
    }

    pub fn page(&self) -> usize {
        self.page
    }

    /// 总页数（无候选 → 0）。
    pub fn page_count(&self) -> usize {
        self.fetched().len().div_ceil(PAGE_SIZE)
    }

    /// 下一页，越界钳制到最后一页；返回新页码。
    pub fn next_page(&mut self) -> usize {
        self.set_page(self.page + 1)
    }

    /// 上一页，越界钳制到首页；返回新页码。
    pub fn prev_page(&mut self) -> usize {
        self.page = self.page.saturating_sub(1);
        self.page
    }

    /// 直接跳到第 `p` 页（0 起），钳制到 [0, page_count-1]；返回实际页码。
    pub fn set_page(&mut self, p: usize) -> usize {
        let count = self.page_count();
        self.page = if count == 0 { 0 } else { p.min(count - 1) };
        self.page
    }
}

#[cfg(test)]
mod tests {
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
    fn empty_buffer_has_no_pages() {
        let mut s = state();
        assert_eq!(s.candidates(), Vec::<String>::new());
        assert_eq!(s.page_count(), 0);
        assert_eq!(s.next_page(), 0);
        assert_eq!(s.set_page(5), 0);
    }
}
