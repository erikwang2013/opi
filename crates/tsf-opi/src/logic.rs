//! TSF 逻辑层：候选翻页状态机 + 键路由（语义对照 A4 行为表，即 Android
//! `KeyRouter`/`EngineController`，与 Linux 轨 fcitx5-opi 同源）。
//!
//! 纯 Rust、无 windows/COM 类型，可独立单测；C2 的 TSF 胶水做键码映射。
//! 键码约定：可打印字符 = Unicode 码点（与 fcitx5 轨一致，如 'a'=97）；
//! 特殊键 = Windows VK 码（TSF 键事件的 wParam 同源），见下方常量。
//! 键状态位沿用 fcitx5 轨的位约定（内部契约，C2 自 TSF 侧换算）。
//!
//! TSF 现实的适配：TSF 下按键总是先经服务处理，无 fcitx5 式"直通客户端"
//! 概念；空缓冲退格/回车、Ctrl/Alt 组合等返回 `Unhandled`，由 C2 决定
//! 是否交应用（不拦截则键自然流入应用）。

use engine_core::Engine;
use engine_core::candidates::Candidate;
use engine_core::composer::Mode;

/// 每页候选数（与 Android 候选栏一致）。
pub const PAGE_SIZE: usize = 8;
/// 一次抓取的候选批量上限（对应 Android 侧 fetchLimit=64）。
pub const FETCH_LIMIT: usize = 64;

// ---------- 特殊键：Windows VK 码（wParam，与 TSF 键事件同源） ----------

/// VK_BACK（退格）。
pub const KEY_BACK_SPACE: u32 = 0x08;
/// VK_TAB。
pub const KEY_TAB: u32 = 0x09;
/// VK_RETURN（回车）。
pub const KEY_RETURN: u32 = 0x0d;
/// VK_ESCAPE。
pub const KEY_ESCAPE: u32 = 0x1b;
/// VK_PRIOR（PageUp → 上一页候选）。
pub const KEY_PAGE_UP: u32 = 0x21;
/// VK_NEXT（PageDown → 下一页候选）。
pub const KEY_PAGE_DOWN: u32 = 0x22;
/// VK_DELETE。
pub const KEY_DELETE: u32 = 0x2e;
/// VK_SHIFT（左右 ⇧ 同为 0x10，与 fcitx5 轨的 SHIFT_L/SHIFT_R 二码不同）。
pub const KEY_SHIFT: u32 = 0x10;
/// 空格。ASCII 码点 0x20，与 Unicode 一致。
pub const KEY_SPACE: u32 = 0x20;

/// 物理 Shift 被按住。
pub const KEY_STATE_SHIFT: u32 = 1 << 0;
/// CapsLock 锁定。
pub const KEY_STATE_CAPS_LOCK: u32 = 1 << 1;
/// 物理 Ctrl 被按住。
pub const KEY_STATE_CTRL: u32 = 1 << 2;
/// 物理 Alt 被按住。
pub const KEY_STATE_ALT: u32 = 1 << 3;
/// 键释放事件。
pub const KEY_STATE_RELEASED: u32 = 1 << 26;
/// 键重复事件。
pub const KEY_STATE_REPEAT: u32 = 1 << 27;
/// 长按事件。
pub const KEY_STATE_LONG_PRESSED: u32 = 1 << 28;

/// 按键处理结果（TSF 语义：无"直通"概念，键由服务/胶水决定是否交应用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyOutcome {
    /// 立即提交文本（英文直传、空缓冲空格、缓冲/候选提交等），C2 插入文档。
    Commit(String),
    /// composition 已变化（缓冲/候选/页码），C2 需刷新 composition + 候选窗。
    CompositionChanged,
    /// 键被服务消费但无状态变化（⇧ 状态机、释放/重复事件等），无需刷新。
    Consumed,
    /// 本层不处理（空缓冲退格/回车、Ctrl/Alt 组合、Tab/Esc、符号等），
    /// 由 C2 决定是否把键交应用（不拦截则自然流入）。
    Unhandled,
}

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

    // ---------- 键路由（对照 A4 行为表 = KeyRouter.kt + EngineController） ----------

    /// 处理一次按键事件（C2 的 TSF KeyDown/KeyUp 路由入口）。
    ///
    /// 路由表（对照 KeyRouter.kt 逐条，见模块文档）：
    /// - 英文模式空缓冲：字母直传（提交）；shift 非 OFF → 转大写并消费 single。
    /// - 其余情况字母入引擎缓冲（`controller.input`）。
    /// - 空格：缓冲非空 → 引擎提交首候选；空缓冲 → 提交 " "。
    /// - 回车：缓冲非空 → 提交首候选；空缓冲 → Unhandled（交应用）。
    /// - 退格：缓冲非空 → 引擎按码点删；空缓冲 → Unhandled（交应用）。
    /// - ⇧ 键：状态机 Off→Single→Off（长按 → Lock）。
    /// - 拼音模式有候选时数字 1..=9 按页内索引选词。
    /// - Ctrl/Alt 组合键一律 Unhandled（系统快捷键，不拦截）。
    pub fn input_key(&mut self, keyval: u32, key_state: u32) -> KeyOutcome {
        // Ctrl/Alt 组合键（系统快捷键）不拦截，交应用处理。
        if key_state & (KEY_STATE_CTRL | KEY_STATE_ALT) != 0 {
            return KeyOutcome::Unhandled;
        }
        let released = key_state & KEY_STATE_RELEASED != 0;
        match keyval {
            KEY_BACK_SPACE | KEY_DELETE => {
                if released {
                    KeyOutcome::Consumed
                } else {
                    self.handle_backspace()
                }
            }
            KEY_SPACE => {
                if released {
                    KeyOutcome::Consumed
                } else {
                    self.handle_space()
                }
            }
            KEY_RETURN => {
                if released {
                    KeyOutcome::Consumed
                } else {
                    self.handle_enter()
                }
            }
            KEY_SHIFT => self.handle_shift(key_state),
            KEY_PAGE_UP => {
                if released {
                    // 释放事件：翻页已在按下时完成，无状态变化 → 不刷新候选窗
                    KeyOutcome::Consumed
                } else {
                    self.prev_page();
                    // 页码变化 → 候选窗需刷新
                    KeyOutcome::CompositionChanged
                }
            }
            KEY_PAGE_DOWN => {
                if released {
                    KeyOutcome::Consumed
                } else {
                    self.next_page();
                    KeyOutcome::CompositionChanged
                }
            }
            KEY_TAB | KEY_ESCAPE => KeyOutcome::Unhandled,
            _ => match char::from_u32(keyval) {
                Some(c) if c.is_ascii() => self.handle_printable(c),
                _ => KeyOutcome::Unhandled,
            },
        }
    }

    /// 退格：缓冲非空 → 引擎删（引擎按码点删）；空缓冲 → 交应用。
    fn handle_backspace(&mut self) -> KeyOutcome {
        if self.buffer().is_empty() {
            KeyOutcome::Unhandled
        } else {
            self.backspace();
            KeyOutcome::CompositionChanged
        }
    }

    /// 空格：缓冲非空 → 引擎提交；空缓冲 → 提交 " "（镜像 `handleSpace`）。
    fn handle_space(&mut self) -> KeyOutcome {
        if self.buffer().is_empty() {
            KeyOutcome::Commit(" ".to_string())
        } else {
            commit_or_changed(self.input_space())
        }
    }

    /// 回车：缓冲非空 → 提交首候选；空缓冲 → 交应用（镜像 `handleEnter`）。
    fn handle_enter(&mut self) -> KeyOutcome {
        if self.buffer().is_empty() {
            KeyOutcome::Unhandled
        } else {
            commit_or_changed(self.select(0))
        }
    }

    /// ⇧ 键：单击切换状态机（Off→Single→Off），长按 → Lock；释放/重复忽略。
    /// 镜像 `EngineController.shiftTap/shiftLongPress`（按住并释放 = 一次 tap）。
    fn handle_shift(&mut self, key_state: u32) -> KeyOutcome {
        let released = key_state & KEY_STATE_RELEASED != 0;
        let repeat = key_state & KEY_STATE_REPEAT != 0;
        if !released && !repeat {
            if key_state & KEY_STATE_LONG_PRESSED != 0 {
                self.shift_long_press();
            } else {
                self.shift_tap();
            }
        }
        KeyOutcome::Consumed
    }

    /// 可见 ASCII 字符按模式分流。
    fn handle_printable(&mut self, c: char) -> KeyOutcome {
        match self.mode() {
            Mode::Pinyin => {
                if c.is_ascii_digit() {
                    return self.digit_select(c);
                }
                // 字母/撇号入引擎缓冲；其余符号交应用
                if c.is_ascii_alphabetic() || c == '\'' {
                    commit_or_changed(self.input_char(c))
                } else {
                    KeyOutcome::Unhandled
                }
            }
            Mode::English => {
                if self.buffer().is_empty() {
                    // 直传路径：shift 非 OFF → 转大写并消费 single（镜像 `handleKey`）
                    if self.shift_state() != ShiftState::Off {
                        self.consume_single_shift();
                        KeyOutcome::Commit(c.to_ascii_uppercase().to_string())
                    } else {
                        KeyOutcome::Commit(c.to_string())
                    }
                } else if c.is_ascii_alphabetic() {
                    // 缓冲非空：字母入引擎（composer 按引擎 shift 决定大小写）
                    commit_or_changed(self.input_char(c))
                } else {
                    KeyOutcome::Unhandled
                }
            }
            Mode::Number | Mode::Symbol => KeyOutcome::Unhandled,
        }
    }

    /// 拼音模式有候选时按数字选词（页内索引：'1'→第 0 个候选）；否则交应用。
    /// 无对应候选（如 '9' 超出、'0'）不消费，交应用输入该数字。
    fn digit_select(&mut self, c: char) -> KeyOutcome {
        if self.mode() == Mode::Pinyin && !self.buffer().is_empty() && !self.candidates().is_empty()
        {
            let Some(d) = c.to_digit(10) else {
                return KeyOutcome::Unhandled;
            };
            let Some(idx) = d.checked_sub(1) else {
                return KeyOutcome::Unhandled;
            };
            let text = self.select(idx as usize);
            // 越界（如 '9' 超出候选数）不消费，交应用输入该数字
            if text.is_empty() {
                KeyOutcome::Unhandled
            } else {
                KeyOutcome::Commit(text)
            }
        } else {
            KeyOutcome::Unhandled
        }
    }
}

/// 引擎输出空串 → composition 变化（无提交文本）；非空 → 提交。
fn commit_or_changed(out: String) -> KeyOutcome {
    if out.is_empty() {
        KeyOutcome::CompositionChanged
    } else {
        KeyOutcome::Commit(out)
    }
}

// 单测独立成文件（`#[path]` 引入）以保持各文件 <500 行：logic_tests.rs
// 为键路由测试，logic_candidate_tests.rs 为候选分页状态机测试。
#[cfg(test)]
#[path = "logic_candidate_tests.rs"]
mod candidate_tests;
#[cfg(test)]
#[path = "logic_tests.rs"]
mod tests;
