// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 按键路由（B2）：镜像 Android `KeyRouter`/`EngineController` 的行为表。
//!
//! 路由表（对照 KeyRouter.kt 逐条）：
//! - 英文模式空缓冲：字母直传客户端；shift 非 OFF → 转大写并消费 single
//!   （镜像 `handleKey` 直传分支）。
//! - 其余情况字母入引擎缓冲（镜像 `controller.input`）。
//! - 空格：缓冲非空 → `input_space()` 提交；空缓冲 → 直传 " "（镜像 `handleSpace`）。
//! - 回车：缓冲非空 → `select(0)` 提交；空缓冲 → 直通客户端（镜像 `handleEnter`）。
//! - 退格：缓冲非空 → 引擎按码点删；空缓冲 → 直通客户端（镜像 `handleBackspace`）。
//! - ⇧ 键：状态机 Off→Single→Off（长按 → Lock），镜像
//!   `EngineController.shiftTap/shiftLongPress/consumeSingleShift`。
//! - 拼音模式有候选时数字 1..=9 按页内索引选词（候选栏点击的键盘等价物）。
//! - 符号模式：可见 ASCII 入引擎缓冲当**关键字**（符号候选由此搜出），其余直通。
//! - Ctrl/Alt 组合键一律直通（系统快捷键，不拦截）。
//!
//! 本模块为纯 Rust、无 fcitx5 类型依赖：`handle_key` 接收裸 `u32` 键值 +
//! 修饰位，便于单测。键值/修饰位与 fcitx5 的映射见下方常量注释。

use engine_core::composer::Mode;

use crate::candidate::{CandidateState, PAGE_SIZE, ShiftState};

// ---------- fcitx5 键值（xkbcommon keysym，fcitx5 Key 同源） ----------

/// 空格。ASCII 码点 0x20，与 Unicode 一致。
pub const KEY_SPACE: u32 = 0x20;
/// 退格键。
pub const KEY_BACK_SPACE: u32 = 0xff08;
/// Tab。
pub const KEY_TAB: u32 = 0xff09;
/// 回车。
pub const KEY_RETURN: u32 = 0xff0d;
/// Esc。
pub const KEY_ESCAPE: u32 = 0xff1b;
/// 上一页（候选翻页，对应 PageUp）。
pub const KEY_PAGE_UP: u32 = 0xff55;
/// 下一页（候选翻页，对应 PageDown）。
pub const KEY_PAGE_DOWN: u32 = 0xff56;
/// 左 Shift。
pub const KEY_SHIFT_L: u32 = 0xffe1;
/// 右 Shift。
pub const KEY_SHIFT_R: u32 = 0xffe2;
/// Delete。
pub const KEY_DELETE: u32 = 0xffff;

// ---------- fcitx5 KeyState 位（与 fcitx5 5.1.x `fcitx::KeyState` 一致） ----------

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

/// 按键处理结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    /// 提交文本到客户端（英文直传、空缓冲空格直传、候选/缓冲提交等）。
    Input(String),
    /// 已路由到引擎（buffer/候选变化），无提交文本；fcitx5 视为已消费。
    EngineHandled,
    /// 未消费，转交客户端应用处理（空缓冲时的退格/回车、Ctrl/Alt 组合等）。
    PassThrough,
}

/// 处理一次按键事件（fcitx5 `keyEvent` 的路由入口）。
///
/// `keyval` 为 fcitx5 键值（ASCII 段与 Unicode 码点一致，如 'a'=97），
/// `states` 为 `KeyState` 修饰位（见上方常量）。字母键的 `keyval` 优先取
/// xkb 符号（物理 shift 已应用，如 Shift+a → 'A'）；若实现侧给的是小写
/// 键值 + SHIFT 位，本路由同样能正确处理。
pub fn handle_key(state: &mut CandidateState, keyval: u32, states: u32) -> KeyAction {
    // Ctrl/Alt 组合键（系统快捷键）一律直通，不拦截。
    if states & (KEY_STATE_CTRL | KEY_STATE_ALT) != 0 {
        return KeyAction::PassThrough;
    }
    let released = states & KEY_STATE_RELEASED != 0;
    match keyval {
        KEY_BACK_SPACE | KEY_DELETE => {
            if released {
                // 抬起按**按下时记下的**结论回复（`last_printable`）——不能判「当前」
                // 缓冲：按下分支 handle_backspace 会把它改空（删掉最后一个字符），
                // 届时再判 buffer().is_empty() 就会放行一个按下时拦下的键 →
                // 客户端收到无 keydown 的 keyup（依赖键状态的游戏/编辑器卡键）。
                match state.last_printable {
                    Some((k, true)) if k == keyval => KeyAction::PassThrough,
                    // 键值不匹配（记录被另一个键顶掉）或从未按下 → 按引擎接管拦下
                    _ => KeyAction::EngineHandled,
                }
            } else {
                let action = handle_backspace(state);
                state.last_printable = Some((keyval, matches!(action, KeyAction::PassThrough)));
                action
            }
        }
        KEY_SPACE => {
            if released {
                KeyAction::EngineHandled
            } else {
                handle_space(state)
            }
        }
        KEY_RETURN => {
            if released {
                // 同退格：抬起按按下记下的结论回复（handle_enter 提交后会清空缓冲）
                match state.last_printable {
                    Some((k, true)) if k == keyval => KeyAction::PassThrough,
                    _ => KeyAction::EngineHandled,
                }
            } else {
                let action = handle_enter(state);
                state.last_printable = Some((keyval, matches!(action, KeyAction::PassThrough)));
                action
            }
        }
        KEY_SHIFT_L | KEY_SHIFT_R => handle_shift(state, states),
        KEY_PAGE_UP => {
            if !released {
                state.prev_page();
            }
            KeyAction::EngineHandled
        }
        KEY_PAGE_DOWN => {
            if !released {
                state.next_page();
            }
            KeyAction::EngineHandled
        }
        KEY_TAB | KEY_ESCAPE => KeyAction::PassThrough,
        _ => match char::from_u32(keyval) {
            // 抬起必须判：本函数上面每个特殊键分支都判了 `released`，可打印
            // 分支此前漏判，导致同一个字符被第二次送进引擎 —— 拼音缓冲翻倍
            // （"ni"→"nnii"）、英文模式重复提交（"a"→"aa"）。
            // 抬起时的结论用按下时记下的（state.last_printable），与按下同判：
            // 可打印键里有直通字符（拼音/繁体的非字母符号、无候选或越界的数字、
            // Number 模式下的全部可见 ASCII、Symbol 模式下的控制符/非 ASCII → handle_printable 返回
            // PassThrough），按下放行、抬起拦下会让客户端收到 keydown 收不到
            // keyup（依赖键状态的游戏/编辑器卡键）。
            // 记结论而非复刻 handle_printable 的分流判定：判定只有一处，不会漂移。
            Some(c) if c.is_ascii() => {
                if released {
                    match state.last_printable {
                        Some((k, true)) if k == keyval => KeyAction::PassThrough,
                        // 键值不匹配（记录被另一个键顶掉）或从未按下 → 按引擎接管拦下
                        _ => KeyAction::EngineHandled,
                    }
                } else {
                    let action = handle_printable(state, c);
                    state.last_printable = Some((keyval, matches!(action, KeyAction::PassThrough)));
                    action
                }
            }
            _ => KeyAction::PassThrough,
        },
    }
}

/// 退格：缓冲非空 → 引擎删（引擎按码点删）；空缓冲 → 直通客户端。
fn handle_backspace(state: &mut CandidateState) -> KeyAction {
    if state.buffer().is_empty() {
        KeyAction::PassThrough
    } else {
        state.backspace();
        KeyAction::EngineHandled
    }
}

/// 空格：缓冲非空 → 引擎提交；空缓冲 → 直传 " "（镜像 `handleSpace`）。
fn handle_space(state: &mut CandidateState) -> KeyAction {
    if state.buffer().is_empty() {
        KeyAction::Input(" ".to_string())
    } else {
        commit_or_handled(state.input_space())
    }
}

/// 回车：缓冲非空 → 提交首候选；空缓冲 → 直通客户端（镜像 `handleEnter`）。
fn handle_enter(state: &mut CandidateState) -> KeyAction {
    if state.buffer().is_empty() {
        KeyAction::PassThrough
    } else {
        commit_or_handled(state.select(0))
    }
}

/// ⇧ 键：单击切换状态机（Off→Single→Off），长按 → Lock；释放/重复忽略。
/// 镜像 `EngineController.shiftTap/shiftLongPress`（按住并释放 = 一次 tap）。
fn handle_shift(state: &mut CandidateState, states: u32) -> KeyAction {
    let released = states & KEY_STATE_RELEASED != 0;
    let repeat = states & KEY_STATE_REPEAT != 0;
    if !released && !repeat {
        if states & KEY_STATE_LONG_PRESSED != 0 {
            state.shift_long_press();
        } else {
            state.shift_tap();
        }
    }
    KeyAction::EngineHandled
}

/// 可见 ASCII 字符按模式分流。
fn handle_printable(state: &mut CandidateState, c: char) -> KeyAction {
    // 标点表（引擎层，四端同一条路）：中文模式出中文标点/全角，其余模式下不映射。
    // 放在模式分派**之前**：标点不属于任何模式的关键字/缓冲语义。
    if let Some(text) = state.input_punct(c) {
        return commit_or_handled(text);
    }
    match state.mode() {
        // 繁体模式与拼音同构：字母/撇号入缓冲、数字选词、走引擎。
        Mode::Pinyin | Mode::Traditional => {
            if c.is_ascii_digit() {
                return digit_select(state, c);
            }
            // 字母/撇号入引擎缓冲；其余符号直通客户端（Android 面板直传的对应物）
            if c.is_ascii_alphabetic() || c == '\'' {
                commit_or_handled(state.input_key(c))
            } else {
                KeyAction::PassThrough
            }
        }
        Mode::English => {
            if state.buffer().is_empty() {
                // 直传路径：shift 非 OFF → 转大写并消费 single（镜像 `handleKey`）
                if state.shift_state() != ShiftState::Off {
                    state.consume_single_shift();
                    KeyAction::Input(c.to_ascii_uppercase().to_string())
                } else {
                    KeyAction::Input(c.to_string())
                }
            } else if c.is_ascii_alphabetic() {
                // 缓冲非空：字母入引擎（composer 按引擎 shift 决定大小写）
                commit_or_handled(state.input_key(c))
            } else {
                KeyAction::PassThrough
            }
        }
        Mode::Number => KeyAction::PassThrough,
        // 符号模式（B3）：可见 ASCII 当关键字入缓冲（dun → 、）；数字**先**当选词键
        // （裁决 2026-09-27，与拼音一致）；空格/控制符/非 ASCII 直通。
        Mode::Symbol if c.is_ascii_digit() => digit_select(state, c),
        Mode::Symbol if c.is_ascii_graphic() => commit_or_handled(state.input_key(c)),
        Mode::Symbol => KeyAction::PassThrough,
    }
}

/// 拼音/繁体/符号模式有候选时按数字选词（页内索引：'1'→第 0 个候选）；否则直通。
/// 无对应候选（本页没有第 9 项、'0'）不消费，交客户端处理。
fn digit_select(state: &mut CandidateState, c: char) -> KeyAction {
    // 本页候选数。**越界判据必须在页内**：`select` 的「越界返回空串」是全量列表的
    // 越界，页内越界（本页只有 8 项时按 '9'）会被它换算成次页下标 —— 提交用户
    // 屏幕上看不见的候选。三轨同形，见 engine-core 侧的钉
    // tests/router_invariants.rs 的 digit_beyond_page_must_not_commit_hidden_candidate。
    // 同一份抓取供两处用（页内计数 + 选中）：旧路径 `candidates()` 抓一次、
    // `select()` → `engine.select` 又抓一次 —— **每次数字选词把整表排两遍**。
    // 计数口径与 `candidates()` 的 skip/take 逐字等价（饱和减法 + min）。
    let fetched = state.fetched();
    let page_len = fetched
        .len()
        .saturating_sub(state.page * PAGE_SIZE)
        .min(PAGE_SIZE);
    if state.mode().digit_selects_candidates() && !state.buffer().is_empty() && page_len > 0 {
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
        let text = state.select_from(&fetched, idx);
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

/// 引擎输出空串 → 无提交；非空 → 提交到客户端。
fn commit_or_handled(out: String) -> KeyAction {
    if out.is_empty() {
        KeyAction::EngineHandled
    } else {
        KeyAction::Input(out)
    }
}

// 单测独立成文件（input_method_tests.rs，`#[path]` 引入）以保持本文件 <500 行。
#[cfg(test)]
#[path = "input_method_punct_tests.rs"]
mod punct_tests;
#[cfg(test)]
#[path = "input_method_release_tests.rs"]
mod release_tests;
#[cfg(test)]
#[path = "input_method_symbol_tests.rs"]
mod symbol_tests;
#[cfg(test)]
#[path = "input_method_tests.rs"]
mod tests;
