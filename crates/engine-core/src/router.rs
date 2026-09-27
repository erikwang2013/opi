// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 平台中立的键路由（iOS / macOS 主路径，经 C ABI `opi_key_event` 暴露）。
//!
//! **为什么在这里**：这张路由表已被抄了两份 —— `crates/fcitx5-opi/src/input_method.rs`
//! 与 `crates/tsf-opi/src/logic.rs` 逐行同构（同源 = Android `KeyRouter` +
//! `EngineController`）；Apple 两平台再各写一份就是第四、五份。本模块是**平台中立的
//! 那一份**：纯逻辑、无 IO、无平台类型（engine-core 的定位），两轨与 Apple 的差别
//! 只剩键码映射（键码表见 [`crate::keys`]）。语义以那两轨为准，本模块不自创。
//!
//! 本轮**不改**两轨去用本模块（收敛是后续的事）；本模块只服务 Apple 两平台。

use crate::candidates::Candidate;
use crate::composer::Mode;
use crate::engine::Engine;
use crate::keys::*;

/// 每页候选数（与 Android 候选栏、两轨 `PAGE_SIZE` 一致）。
pub const PAGE_SIZE: usize = 8;
/// 一次抓取的候选批量上限：**不设上限**。曾是 64（对齐 Android fetchLimit），后果是
/// [`KeyRouter::page_count`] 封顶 8 页：实测 luna 下 y=8006 / yi=2884 条命中，用户只能
/// 翻到前 64 条（0.8%–23%），而第 64 与第 65 名的静态词频只差 0.01%（无断崖）。
/// 上限也不省成本：`rank_and_pick` 无论 limit 多大都全量收集 + 排序（见该模块
/// 「不能下推 limit 到词库」），截断只是扔掉已经算好的尾巴。
pub const FETCH_LIMIT: usize = usize::MAX;

/// 按键处理结果。对应 C ABI 的 `action`：`PassThrough=0`、`EngineHandled=1`、
/// `Input=2`（此时 text 有效）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    /// 未消费，交系统/客户端处理（空缓冲的退格/回车、Ctrl/Alt/⌘ 组合、Tab/Esc、
    /// 拼音态的非字母符号等）。
    PassThrough,
    /// 已被本层消费，无提交文本（缓冲/候选/页码变化、⇧ 状态机、释放事件等）；
    /// 调用方需刷新 composition / 候选栏。
    EngineHandled,
    /// 提交文本到客户端（英文直传、空缓冲空格直传、缓冲/候选提交等）。
    Input(String),
}

/// ⇧ 状态机：off / single（下个字母大写后自动复位）/ lock（持续大写）。
/// 镜像 Android `EngineController.ShiftState` 的三态语义（与两轨同名枚举同源）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShiftState {
    #[default]
    Off,
    Single,
    Lock,
}

/// 引擎 + 候选分页 + ⇧ 状态机 + 按键抬起结论（平台中立路由的唯一状态对象）。
///
/// 结构对应两轨的 `CandidateState` / `TsfLogic`：**持有引擎**，路由语义逐条同构。
/// 状态随引擎生命周期走（调用方按需重建），不持有任何平台类型。
pub struct KeyRouter {
    engine: Engine,
    /// 当前页（0 起）。
    page: usize,
    /// 上次操作后的 buffer 快照；变化时页码归零。
    buffer_snapshot: String,
    /// ⇧ 状态机（镜像 Android `EngineController.shiftState`）。
    shift_state: ShiftState,
    /// 上一次按下的分流结论：(键值, 是否直通)。抬起按同一结论回复，
    /// 使「按下放行 → 抬起也放行」成立（见 `key_event` 的可打印分支与退格/回车分支：
    /// 那两个按下分支会把缓冲改空，抬起再判「当前」缓冲就不对称了）。
    last_printable: Option<(u32, bool)>,
}

impl KeyRouter {
    /// 用已装载的引擎构造路由（词库装载由上层的 `Engine::new` 负责，
    /// engine-core 不依赖 engine-data）。
    pub fn new(engine: Engine) -> Self {
        let mut r = KeyRouter {
            engine,
            page: 0,
            buffer_snapshot: String::new(),
            shift_state: ShiftState::Off,
            last_printable: None,
        };
        r.refresh_snapshot();
        r
    }

    /// 引擎只读访问（FFI 出口转发用：候选栏、缓冲、模式等直接查引擎）。
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// 引擎可变访问（FFI 出口转发用）。
    ///
    /// **绕过路由改引擎，改完必须调用 [`Self::reset_page_if_buffer_changed`]** ——
    /// 否则页码停在过期值上，随后的数字选词/回车提交会用它算全局下标、**选错候选**
    /// （`opi_input_key` 重填出与快照相同的 buffer 时，路由内部的判据也察觉不到）。
    /// 键路由自身（`key_event`）不需要手动调，它在每个改动点内联做了。
    pub fn engine_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }

    fn refresh_snapshot(&mut self) {
        self.buffer_snapshot = self.engine.buffer().to_string();
    }

    /// buffer 与快照不一致 → 页码归零（与两轨语义一致），再刷新快照。
    ///
    /// 两轨里这一步由状态对象自己的方法在末尾内联调用（外部拿不到状态）；本层的
    /// raw ABI 出口（`opi_input_key`/`opi_backspace`）直接改引擎，故公开给它们显式调用。
    pub fn reset_page_if_buffer_changed(&mut self) {
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

    /// 引擎侧 ⇧（前端三态见 `shift_tap`/`shift_long_press`）。
    /// 公开给 `opi_set_shift` 出口 —— 它可能改变候选集，须把页码钳回边界
    /// （两轨的 `CandidateState::set_shift` 同样公开且同样钳制）。
    pub fn set_shift(&mut self, on: bool) {
        self.engine.set_shift(on);
        // shift 不改变 buffer（reset_page_if_buffer_changed 不会触发），但
        // 可能改变候选集；将页码钳制到当前 page_count 边界，防 page 越界。
        self.set_page(self.page);
    }

    /// 切换模式：**离开** English 才清前端 ⇧（判据与 Android `EngineController.switchMode`
    /// 的 `if (m != ENGLISH) resetShift()` 逐字一致）。
    ///
    /// 前端 ⇧ 三态与引擎侧是两份状态，引擎清了不够 —— 英文空缓冲直传的大小写由
    /// `ShiftState` 决定，Lock 残留会让再次进入 English 后打出的全是大写。
    /// C ABI 的 `opi_switch_mode` 必须走本方法（而不是直接打引擎），否则 Apple 侧
    /// 就有这个 bug。
    pub fn switch_mode(&mut self, mode: Mode) {
        self.engine.switch_mode(mode);
        if mode != Mode::English {
            self.shift_state = ShiftState::Off;
        }
        self.reset_page_if_buffer_changed();
    }

    // ---- 候选分页（与两轨同构） ----

    /// 批量抓取（分页的唯一数据源：`page_count`/`candidates`/`select` 都从这一份算）。
    pub(crate) fn fetched(&self) -> Vec<Candidate> {
        self.engine.candidates(FETCH_LIMIT)
    }

    /// 当前页候选文本（最多 PAGE_SIZE 条）。**分页只此一份** —— 经 `opi_candidates_page`
    /// 出口给 UI，前端因此不需要知道 PAGE_SIZE（自己切全局列表 = 常量抄第二份）。
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

    /// 总页数（无候选 → 0）。公开给 `opi_page_count` 出口（UI 的「共 N 页」）——
    /// 与 [`Self::candidates`] 同源，两者不会各算一套。
    pub fn page_count(&self) -> usize {
        self.fetched().len().div_ceil(PAGE_SIZE)
    }

    /// 直接跳到第 `p` 页（0 起），钳制到 [0, page_count-1]；返回实际页码。
    fn set_page(&mut self, p: usize) -> usize {
        let count = self.page_count();
        self.page = if count == 0 { 0 } else { p.min(count - 1) };
        self.page
    }

    /// 提交当前页第 `index` 个候选（**页内**索引，0 起）。越界返回空串、buffer 不变。
    /// 回车提交（`handle_enter`）与 C 出口 `opi_select_page` 都走这里 ——
    /// 前端因此不需要知道 PAGE_SIZE。
    pub fn select(&mut self, index: usize) -> String {
        let fetched = self.fetched();
        self.select_from(&fetched, index)
    }

    /// 页内索引 → 全局下标（**换算只此一份**），在调用方已抓好的候选表上提交。
    /// 数字选词（`digit_select`）复用自己那次抓取，不重排整表。
    pub(crate) fn select_from(&mut self, fetched: &[Candidate], index: usize) -> String {
        let global = self.page * PAGE_SIZE + index;
        let out = self.engine.select_from(fetched, global);
        self.reset_page_if_buffer_changed();
        out
    }

    // ---- 键路由（对照两轨的 handle_key / input_key，逐条同构） ----

    /// 处理一次按键事件。
    ///
    /// 路由表（两轨逐字同构，见模块头注释）：
    /// - Ctrl/Alt/⌘ 组合：直通（系统快捷键，不拦截）。
    /// - 英文模式空缓冲：字母直传；⇧ 非 OFF → 转大写并消费 single。
    /// - 其余情况字母/撇号入引擎缓冲。
    /// - 空格：缓冲非空 → 引擎提交；空缓冲 → 直传 " "。
    /// - 回车：缓冲非空 → 提交首候选；空缓冲 → 直通。
    /// - 退格/Delete：缓冲非空 → 引擎按码点删；空缓冲 → 直通。
    /// - ⇧：Off→Single→Off（长按 → Lock）；PageUp/PageDown：翻页（钳制）；Tab/Esc/方向键：直通。
    /// - 标点：查引擎层标点表（`Engine::input_punct`）—— 中文模式出中文标点/全角，
    ///   其余模式按各自分支走（英文直传、数字直通、符号模式原样插入）。
    /// - 数字：拼音/繁体/符号且有候选时按**页内**索引选词，页内越界/无候选 → 直通。
    /// - 符号模式：字母数字入引擎缓冲当**关键字**（符号候选由此搜出），其余直通。
    /// - 退格/回车/可打印键的**抬起按按下的结论回复**（`last_printable` 单槽）。
    pub fn key_event(&mut self, keyval: u32, states: u32) -> KeyAction {
        // Ctrl/Alt/⌘ 组合键（系统快捷键）一律直通，不拦截。
        if states & (KEY_STATE_CTRL | KEY_STATE_ALT | KEY_STATE_META) != 0 {
            return KeyAction::PassThrough;
        }
        let released = states & KEY_STATE_RELEASED != 0;
        match keyval {
            KEY_BACK_SPACE | KEY_DELETE => {
                if released {
                    // 抬起按**按下时记下的**结论回复（`last_printable`）——不能判「当前」
                    // 缓冲：按下分支 handle_backspace 会把它改空（删掉最后一个字符），
                    // 届时再判 buffer().is_empty() 就会放行一个按下时拦下的键 →
                    // 客户端收到无 keydown 的 keyup（依赖键状态的控件卡键）。
                    match self.last_printable {
                        Some((k, true)) if k == keyval => KeyAction::PassThrough,
                        // 键值不匹配（记录被另一个键顶掉）或从未按下 → 按引擎接管拦下
                        _ => KeyAction::EngineHandled,
                    }
                } else {
                    let action = self.handle_backspace();
                    self.last_printable = Some((keyval, matches!(action, KeyAction::PassThrough)));
                    action
                }
            }
            // 空格在**可打印段**（0x20，见 keys.rs）——不是 TSF 轨的 `SPECIAL_BASE|0x20`。
            // 照 TSF 抄会把空格落进下面的非 ASCII 分支，表现为「拼音打一半按空格
            // 不提交候选、只输出一个空格」。
            KEY_SPACE => {
                if released {
                    KeyAction::EngineHandled
                } else {
                    self.handle_space()
                }
            }
            KEY_RETURN => {
                if released {
                    // 同退格：抬起按按下记下的结论回复（handle_enter 提交后会清空缓冲）
                    match self.last_printable {
                        Some((k, true)) if k == keyval => KeyAction::PassThrough,
                        _ => KeyAction::EngineHandled,
                    }
                } else {
                    let action = self.handle_enter();
                    self.last_printable = Some((keyval, matches!(action, KeyAction::PassThrough)));
                    action
                }
            }
            KEY_SHIFT => self.handle_shift(states),
            // 翻页直接走 `set_page`（钳制到 [0, 页数-1]）：原先的 prev/next 包装
            // 与它逐字相同却又少一道钳制，返回值两个调用点都没接。
            KEY_PAGE_UP => {
                if !released {
                    self.set_page(self.page.saturating_sub(1));
                }
                KeyAction::EngineHandled
            }
            KEY_PAGE_DOWN => {
                if !released {
                    self.set_page(self.page + 1);
                }
                KeyAction::EngineHandled
            }
            KEY_TAB | KEY_ESCAPE | KEY_UP | KEY_DOWN | KEY_LEFT | KEY_RIGHT => {
                KeyAction::PassThrough
            }
            _ => match char::from_u32(keyval) {
                // 抬起必须判：本函数上面每个特殊键分支都判了 `released`，可打印
                // 分支此前漏判，导致同一个字符被第二次送进引擎 —— 拼音缓冲翻倍
                // （"ni"→"nnii"）、英文模式重复提交（"a"→"aa"）。
                // 抬起时的结论用按下时记下的（self.last_printable），与按下同判：
                // 可打印键里有直通字符（拼音/繁体的非字母符号、无候选或越界的数字、
                // Number 模式下的全部可见 ASCII、Symbol 模式下的控制符/非 ASCII →
                // handle_printable 返回 PassThrough），按下放行、抬起拦下会让客户端
                // 收到 keydown 收不到 keyup（依赖键状态的控件卡键）。
                // 记结论而非复刻 handle_printable 的分流判定：判定只有一处，不会漂移。
                Some(c) if c.is_ascii() => {
                    if released {
                        match self.last_printable {
                            Some((k, true)) if k == keyval => KeyAction::PassThrough,
                            // 键值不匹配（记录被另一个键顶掉）或从未按下 → 按引擎接管拦下
                            _ => KeyAction::EngineHandled,
                        }
                    } else {
                        let action = self.handle_printable(c);
                        self.last_printable =
                            Some((keyval, matches!(action, KeyAction::PassThrough)));
                        action
                    }
                }
                // 非 ASCII：特殊键漏了 match 臂时落到这里（键码带 SPECIAL_BASE，
                // 必为补充平面字符），一律交系统 —— 绝不变成垃圾字符。
                _ => KeyAction::PassThrough,
            },
        }
    }

    /// 退格：缓冲非空 → 引擎删（引擎按码点删）；空缓冲 → 直通。
    fn handle_backspace(&mut self) -> KeyAction {
        if self.buffer().is_empty() {
            KeyAction::PassThrough
        } else {
            self.engine.backspace();
            self.reset_page_if_buffer_changed();
            KeyAction::EngineHandled
        }
    }

    /// 空格：缓冲非空 → 引擎提交；空缓冲 → 直传 " "（镜像 `handleSpace`）。
    fn handle_space(&mut self) -> KeyAction {
        if self.buffer().is_empty() {
            KeyAction::Input(" ".to_string())
        } else {
            let out = self.engine.input_space();
            self.reset_page_if_buffer_changed();
            commit_or_handled(out)
        }
    }

    /// 回车：缓冲非空 → 提交首候选；空缓冲 → 直通（镜像 `handleEnter`）。
    fn handle_enter(&mut self) -> KeyAction {
        if self.buffer().is_empty() {
            KeyAction::PassThrough
        } else {
            commit_or_handled(self.select(0))
        }
    }

    /// ⇧ 键：单击切换状态机（Off→Single→Off），长按 → Lock；释放/重复忽略。
    /// 镜像 `EngineController.shiftTap/shiftLongPress`（按住并释放 = 一次 tap）。
    fn handle_shift(&mut self, states: u32) -> KeyAction {
        let released = states & KEY_STATE_RELEASED != 0;
        let repeat = states & KEY_STATE_REPEAT != 0;
        if !released && !repeat {
            if states & KEY_STATE_LONG_PRESSED != 0 {
                self.shift_long_press();
            } else {
                self.shift_tap();
            }
        }
        KeyAction::EngineHandled
    }

    /// 可见 ASCII 字符按模式分流。
    fn handle_printable(&mut self, c: char) -> KeyAction {
        // 标点表（引擎层，四端同一条路）：中文模式出中文标点/全角，其余模式下不映射。
        // 放在模式分派**之前**：标点不属于任何模式的关键字/缓冲语义。
        if let Some(text) = self.engine.input_punct(c) {
            self.reset_page_if_buffer_changed();
            return commit_or_handled(text);
        }
        match self.mode() {
            // 繁体模式与拼音同构：字母/撇号入缓冲、数字选词、走引擎。
            Mode::Pinyin | Mode::Traditional => {
                if c.is_ascii_digit() {
                    return self.digit_select(c);
                }
                // 字母/撇号入引擎缓冲；其余符号直通（Android 面板直传的对应物）
                if c.is_ascii_alphabetic() || c == '\'' {
                    let out = self.engine.input_key(c);
                    self.reset_page_if_buffer_changed();
                    commit_or_handled(out)
                } else {
                    KeyAction::PassThrough
                }
            }
            Mode::English => {
                if self.buffer().is_empty() {
                    // 直传路径：shift 非 OFF → 转大写并消费 single（镜像 `handleKey`）
                    if self.shift_state() != ShiftState::Off {
                        self.consume_single_shift();
                        KeyAction::Input(c.to_ascii_uppercase().to_string())
                    } else {
                        KeyAction::Input(c.to_string())
                    }
                } else if c.is_ascii_alphabetic() {
                    // 缓冲非空：字母入引擎（composer 按引擎 shift 决定大小写）
                    let out = self.engine.input_key(c);
                    self.reset_page_if_buffer_changed();
                    commit_or_handled(out)
                } else {
                    KeyAction::PassThrough
                }
            }
            Mode::Number => KeyAction::PassThrough,
            // 符号模式（B3）：可见 ASCII 当关键字入缓冲（dun → 、）；数字先当选词键（与拼音一致）；空格/控制符/非 ASCII 直通。
            Mode::Symbol if c.is_ascii_digit() => self.digit_select(c),
            Mode::Symbol if c.is_ascii_graphic() => {
                let out = self.engine.input_key(c);
                self.reset_page_if_buffer_changed();
                commit_or_handled(out)
            }
            Mode::Symbol => KeyAction::PassThrough,
        }
    }

    /// 拼音/繁体/符号模式有候选时按数字选词（页内索引：'1'→第 0 个候选）；否则直通。
    /// 无对应候选（本页没有第 9 项、'0'）不消费，交客户端处理。
    fn digit_select(&mut self, c: char) -> KeyAction {
        // 本页候选数。**越界判据必须在页内**：`select` 的「越界返回空串」是全量列表的
        // 越界，页内越界（本页只有 8 项时按 '9'）会被它换算成次页下标 —— 提交用户
        // 屏幕上看不见的候选。见 tests/router_invariants.rs 的
        // digit_beyond_page_must_not_commit_hidden_candidate。
        // 同一份抓取供两处用（页内计数 + 选中）：旧路径 `candidates()` 抓一次、
        // `select()` → `engine.select` 又抓一次 —— **每次数字选词把整表排两遍**。
        // 计数口径与 `candidates()` 的 skip/take 逐字等价（饱和减法 + min）。
        let fetched = self.fetched();
        let page_len = fetched
            .len()
            .saturating_sub(self.page * PAGE_SIZE)
            .min(PAGE_SIZE);
        if self.mode().digit_selects_candidates() && !self.buffer().is_empty() && page_len > 0 {
            let Some(d) = c.to_digit(10) else {
                return KeyAction::PassThrough;
            };
            let Some(idx) = d.checked_sub(1) else {
                return KeyAction::PassThrough;
            };
            let idx = idx as usize;
            if idx >= page_len {
                return KeyAction::PassThrough;
            }
            // 与 `select` 同一套换算与收尾，只是复用上面那次抓取
            let text = self.select_from(&fetched, idx);
            // 引擎没给出该候选（页内判据下不可达，保留为最后一道防线）同样不消费
            if text.is_empty() {
                KeyAction::PassThrough
            } else {
                KeyAction::Input(text)
            }
        } else {
            KeyAction::PassThrough
        }
    }
}

/// 引擎输出空串 → 无提交；非空 → 提交到客户端。
fn commit_or_handled(out: String) -> KeyAction {
    if out.is_empty() {
        KeyAction::EngineHandled
    } else {
        KeyAction::Input(out)
    }
}

// 单测独立成文件（`#[path]` 引入）以保持本文件 <500 行，与两轨的
// input_method_tests.rs / logic_tests.rs 同惯例。符号模式（B3）的用例在
// tests/symbol_mode.rs（只用公开 API，故不必在此挂 `#[path]` 模块）。
#[cfg(test)]
#[path = "router_release_tests.rs"]
mod release_tests;
#[cfg(test)]
#[path = "router_tests.rs"]
mod tests;
