// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 候选翻页状态：包装 engine_core::Engine，持有候选分页状态。
//!
//! 语义与 Android 侧一致：每页 8 个候选，一次抓取 FETCH_LIMIT 条（**不设上限**，见该
//! 常量注释；Android 的 fetchLimit 是 JNI 另一条通路，够不到这里的分页出口），页码越界
//! 钳制，buffer 变化时页码归零。纯逻辑结构体（无全局状态、无 FFI），可独立单测；
//! C 出口（lib.rs）经 Mutex 单例访问本状态。

use engine_core::Engine;
use engine_core::candidates::Candidate;
use engine_core::composer::Mode;

/// 每页候选数（与 Android 候选栏一致）。
pub const PAGE_SIZE: usize = 8;
/// 一次抓取的候选批量上限：**不设上限**。曾是 64（对齐 Android fetchLimit），后果见
/// `engine-core/src/router.rs` 同名常量：真词库下 `y` 前缀一次命中**八千余条**，用户只能
/// 翻到前 64 条（≈0.8%）—— 而第 64 与第 65 名的静态词频只差 0.01%，没有断崖可供察觉。
/// ⚠️ 「八千余」**别换成精确数字**（它会漂，实测差异见 `candidate_limits_tests.rs` 头注释）。
/// 上限也不省成本：`rank_and_pick` 无论 limit 多大都全量收集 + 排序，截断只是扔掉算好的尾巴。
pub const FETCH_LIMIT: usize = usize::MAX;

// 与 `engine-core/src/router.rs` 的**同名常量**绑死 —— 期望值来自**另一份源码**，不是
// 本文件的字面量。三处各存一份、彼此零绑定的年代，v1.3.0 只抬了那一份，本轨留在 64 上，
// **没有任何门禁红**（2026-09-28 修，见 `candidate_limits_tests.rs`）。
// 编译期求值：漂移是 E0080，不是「某条测试恰好没跑到」。放在常量正下方是刻意的 ——
// 改常量的人没法不看见它（同 `vk.rs` 那条 const 断言的取舍）。
const _: () = {
    assert!(PAGE_SIZE == engine_core::router::PAGE_SIZE);
    assert!(FETCH_LIMIT == engine_core::router::FETCH_LIMIT);
};

/// ⇧ 状态机：off / single（下个字母大写后自动复位）/ lock（持续大写）。
/// 镜像 Android `EngineController.ShiftState` 的三态语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShiftState {
    #[default]
    Off,
    Single,
    Lock,
}

/// 引擎 + 候选分页状态。
pub struct CandidateState {
    pub(crate) engine: Engine,
    /// 当前页（0 起）。
    pub(crate) page: usize,
    /// 上次操作后的 buffer 快照；变化时页码归零。
    pub(crate) buffer_snapshot: String,
    /// ⇧ 状态机（镜像 Android EngineController.shiftState）。
    pub(crate) shift_state: ShiftState,
    /// 上一次按下的分流结论：(键值, 是否直通)。抬起按同一结论回复，使「按下放行 →
    /// 抬起也放行」成立（见 input_method::handle_key 的可打印分支与退格/回车分支：
    /// 那两个按下分支会把缓冲改空，抬起再判「当前」缓冲就不对称了）。
    pub(crate) last_printable: Option<(u32, bool)>,
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
            shift_state: ShiftState::Off,
            last_printable: None,
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

    /// 引擎层标点表：`Some` = 该键归标点表管（文本要上屏），`None` = 交回路由直通。
    /// 与 `input_key` 同一套收尾 —— 命中时引擎可能先 flush 缓冲，页码得跟着归零。
    pub fn input_punct(&mut self, ch: char) -> Option<String> {
        let out = self.engine.input_punct(ch);
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
        // ⇧ 只在 English 有意义，判据与 Android `EngineController.switchMode` 的
        // `if (m != ENGLISH) resetShift()` 逐字一致：**离开** English 才清。
        // 前端 ⇧ 三态与引擎侧是两份状态，引擎清了不够 —— 英文空缓冲直传的大小写由
        // ShiftState 决定，Lock 残留会让再次进入 English 后打出的全是大写。
        if mode != Mode::English {
            self.shift_state = ShiftState::Off;
        }
        self.reset_page_if_buffer_changed();
    }

    /// 全角 ↔ 半角，返回切换后的状态（状态栏显示用）。语义全在引擎层
    /// （[`Engine::toggle_fullwidth`]），本层只做转发 —— **键位不在本层**：
    /// 与 `toggle_symbol` 同一条（键位不在本层）：两轨一律 `Shift+Space`，且必须
    /// 在**调引擎之前**判掉 —— `router.rs` 对 Ctrl/Alt/Meta 在最前面就直通，而
    /// `Shift+Space` 会走到 `KEY_SPACE` 分支且**该分支不看 Shift 位**（表现为「选首
    /// 候选」）。键位占用表见 `vk.rs` 的 `fullwidth_hotkey` 与 C++ 侧同名函数。
    pub fn toggle_fullwidth(&mut self) -> bool {
        self.engine.toggle_fullwidth()
    }

    pub fn set_shift(&mut self, on: bool) {
        self.engine.set_shift(on);
        // shift 不改变 buffer（reset_page_if_buffer_changed 不会触发），但
        // 可能改变候选集；将页码钳制到当前 page_count 边界，防 page 越界。
        self.set_page(self.page);
    }

    // ---- ⇧ 状态机（镜像 Android EngineController.shiftTap/ShiftLongPress/consumeSingleShift）----

    pub fn shift_state(&self) -> ShiftState {
        self.shift_state
    }

    /// 单击：Off→Single（引擎 shift 开）；Single/Lock→Off（引擎 shift 关）。
    pub fn shift_tap(&mut self) {
        self.shift_state = if self.shift_state == ShiftState::Off {
            ShiftState::Single
        } else {
            ShiftState::Off
        };
        self.set_shift(self.shift_state != ShiftState::Off);
    }

    /// 长按：Lock（持续大写）。
    pub fn shift_long_press(&mut self) {
        self.shift_state = ShiftState::Lock;
        self.set_shift(true);
    }

    /// single 态消费后复位（lock 不受影响）。
    pub fn consume_single_shift(&mut self) {
        if self.shift_state == ShiftState::Single {
            self.shift_state = ShiftState::Off;
            self.set_shift(false);
        }
    }

    /// 提交当前页第 `index` 个候选（页内索引，0 起）。越界返回空串。
    pub fn select(&mut self, index: usize) -> String {
        let fetched = self.fetched();
        self.select_from(&fetched, index)
    }

    /// 页内索引 → 全局下标（**换算只此一份**），在调用方已抓好的候选表上提交。
    /// 数字选词（`digit_select`）复用自己那次抓取，不重排整表。
    ///
    /// **越界判据是页内的**（本页候选数）：全量判据会把页内越界（本页 8 项时点第 9 项）
    /// 换算成**次页**下标 ⇒ 提交用户看不见的候选（门禁：`candidate_tests.rs` /
    /// `logic_candidate_tests.rs` 的 `select_beyond_page_returns_empty`）。页内/全量越界一律空串。
    pub(crate) fn select_from(&mut self, fetched: &[Candidate], index: usize) -> String {
        // 本页候选数（口径与 `candidates()` 的 skip/take 逐字等价：饱和减法 + min）
        let page_len = fetched
            .len()
            .saturating_sub(self.page * PAGE_SIZE)
            .min(PAGE_SIZE);
        let out = if index < page_len {
            let global = self.page * PAGE_SIZE + index;
            self.engine.select_from(fetched, global)
        } else {
            String::new()
        };
        self.reset_page_if_buffer_changed();
        out
    }

    /// 批量抓取（FETCH_LIMIT 内；不设上限时 `truncate` 是空操作，排序仍全量）。
    pub(crate) fn fetched(&self) -> Vec<Candidate> {
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

// 单测独立成文件（`#[path]` 引入）以保持本文件 <500 行：候选分页状态机测试（键路由测试
// 见 `input_method` 模块）。配对：`candidate_tests.rs`（本轨）↔ `logic_candidate_tests.rs`
// （tsf 轨）；两轨**分层不同**（本轨的 buffer 编辑方法在本文件，tsf 在 `logic_input_method.rs`），
// 两份测试的调用面因此不同构 —— 别按「逐行同构」去"修"。
#[cfg(test)]
#[path = "candidate_tests.rs"]
mod candidate_tests;
// 候选上限门禁独立成文件（`#[path]` 引入）以保持本文件 <500 行；两轨同源不合并，
// 配对：`candidate_limits_tests.rs`（Linux 轨）↔ `logic_limits_tests.rs`（Windows 轨）。
#[cfg(test)]
#[path = "candidate_limits_tests.rs"]
mod limits_tests;
