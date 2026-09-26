// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! TSF 逻辑层：候选翻页状态机 + 键路由（语义对照 A4 行为表，即 Android
//! `KeyRouter`/`EngineController`，与 Linux 轨 fcitx5-opi 同源）。
//!
//! 纯 Rust、无 windows/COM 类型，可独立单测；C2 的 TSF 胶水做键码映射。
//! 键码约定（可打印字符 = Unicode 码点，如 'a'=97；特殊键 = `SPECIAL_BASE |
//! Windows VK`；键状态位沿用 fcitx5 轨位约定）与路由表见子模块 `input_method`
//! 头注释 —— 两段键空间不相交是硬约束，成因见 `input_method::SPECIAL_BASE`。
//!
//! TSF 现实的适配：TSF 下按键总是先经服务处理，无 fcitx5 式"直通客户端"
//! 概念；空缓冲退格/回车、Ctrl/Alt 组合等返回 `Unhandled`，由 C2 决定
//! 是否交应用（不拦截则键自然流入）。
//!
//! 划分与 Linux 轨对称（两轨逐行同构，测试文件不合并）：本文件 = fcitx5 轨的
//! `candidate.rs`（状态 + 分页），子模块 `input_method`（`logic_input_method.rs`）
//! = fcitx5 轨的 `input_method.rs`（键路由）。

use engine_core::Engine;
use engine_core::candidates::Candidate;
use engine_core::composer::Mode;

/// 每页候选数（与 Android 候选栏一致）。
pub const PAGE_SIZE: usize = 8;
/// 一次抓取的候选批量上限（对应 Android 侧 fetchLimit=64）。
pub const FETCH_LIMIT: usize = 64;

/// ⇧ 状态机：off / single（下个字母大写后自动复位）/ lock（持续大写）。
/// 镜像 Android `EngineController.ShiftState` 的三态语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShiftState {
    #[default]
    Off,
    Single,
    Lock,
}

/// 引擎 + 候选分页 + ⇧ 状态机（TSF 逻辑层唯一状态对象）。
pub struct TsfLogic {
    pub(crate) engine: Engine,
    /// 当前页（0 起）。
    pub(crate) page: usize,
    /// 上次操作后的 buffer 快照；变化时页码归零。
    pub(crate) buffer_snapshot: String,
    /// ⇧ 状态机（镜像 Android EngineController.shiftState）。
    pub(crate) shift_state: ShiftState,
    /// 上一次可打印键按下的分流结论：(键值, 是否放行)。抬起按同一结论回复，
    /// 使「按下放行 → 抬起也放行」成立（见 input_key 的可打印分支）。
    pub(crate) last_printable: Option<(u32, bool)>,
}

impl TsfLogic {
    /// 装载引擎。`None`/空串 → 内置回退词库（35 词）；非空路径 →
    /// load_or_fallback 原样语义（坏路径返回 Err）。与 opi-ffi 及
    /// fcitx5-opi 的 load 语义一致（学习默认开）。
    pub fn load(path: Option<&str>) -> Result<Self, String> {
        let dict: Box<dyn engine_core::dictionary::Dictionary> = match path {
            Some(p) if !p.is_empty() => {
                engine_data::load_or_fallback(Some(std::path::Path::new(p)))?
            }
            _ => Box::new(engine_data::fallback_dict()),
        };
        let symbols = engine_core::symbols::SymbolEngine::builtin();
        let mut s = TsfLogic {
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

    /// 单字符入引擎（路由内部用）。返回引擎输出（如英文模式已提交文本）。
    fn input_char(&mut self, ch: char) -> String {
        let out = self.engine.input_key(ch);
        self.reset_page_if_buffer_changed();
        out
    }

    fn input_space(&mut self) -> String {
        let out = self.engine.input_space();
        self.reset_page_if_buffer_changed();
        out
    }

    fn backspace(&mut self) {
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

    fn set_shift(&mut self, on: bool) {
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

// 键路由（KeyOutcome + KEY_*/KEY_STATE_* 常量 + input_key/handle_* ）在子模块，
// 对应 fcitx5 轨的 input_method.rs；`pub use` 保持 `crate::logic::KEY_*` 等
// 既有路径不变（vk.rs / tsf.rs 直接引用）。
#[path = "logic_input_method.rs"]
pub mod input_method;
pub use input_method::*;

// 单测独立成文件（`#[path]` 引入）以保持各文件 <500 行：logic_candidate_tests.rs
// 为候选分页状态机测试（键路由测试见 input_method 模块）。
#[cfg(test)]
#[path = "logic_candidate_tests.rs"]
mod candidate_tests;
