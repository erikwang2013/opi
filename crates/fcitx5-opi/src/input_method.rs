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
//! - `PageUp`/`PageDown` 翻页；**组字中（拼音/繁体且缓冲非空）`-` 上一页、`=` 下一页**
//!   （2026-10-09 用户报障补上；空缓冲不抢键，见 input_method_page_tests.rs）。
//! - 符号模式：可见 ASCII 入引擎缓冲当**关键字**（符号候选由此搜出），其余直通。
//! - **符号浏览**（「符号大全」叠加态，2026-10-10）：浏览期间只有 `1`-`9`（激活浏览项：
//!   类目下钻 / 符号上屏）、`-`/`=`（浏览翻页）、Esc（退出并吞掉）归浏览；**其余一律先
//!   退出浏览恢复真缓冲、再按正常路由处理那个键** —— 绝不让「符号」标题落进正常路由
//!   （契约 ②，见 engine-core/src/engine_symbol_browse.rs 头）。括号键 `0`、PageUp/Down、
//!   Shift 等都在「其余」里（字面执行 lead 裁决的键集，别自作主张扩）。
//! - Ctrl/Alt 组合键一律直通（系统快捷键，不拦截）—— 含插件热键 Ctrl+Shift+Z
//!   （进入/退出浏览在 C++ 侧 opi_fcitx5_symbol_browse，见 cabi.rs）。
//!
//! 本模块为纯 Rust、无 fcitx5 类型依赖：`handle_key` 接收裸 `u32` 键值 +
//! 修饰位，便于单测。键值/修饰位与 fcitx5 的映射见下方常量注释。

use engine_core::EnterAction;
use engine_core::composer::Mode;

use crate::candidate::{CandidateState, ShiftState};

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
    // 符号浏览（契约 ②，见 engine_symbol_browse.rs 头）：**按下**时只有 `1`-`9`、
    // `-`/`=`、Esc 归浏览；其余键先把浏览退出（恢复真缓冲）再落进下面的正常路由 ——
    // 「退出」放在这里（全部按键之前）而不是各分支里，单点免得某个键漏掉。
    // 抬起不在此处理：各键的 press/release 对称由下方 last_printable 机制管。
    if !released && state.is_symbol_browsing() {
        // 三个提前 return 的分支必须记 `last_printable`：不记的话，抬起会命中
        // **上一次**同名键留下的记录（可能 true）→ 放行一个按下时被拦下的 keyup
        // （与退格/回车那两处的理由逐字相同）。浏览态一律不直通。
        match keyval {
            // `1`-`9`：激活浏览项（页内索引，与候选数字键同一套换算）
            0x31..=0x39 => {
                let action = browse_digit(state, keyval);
                state.last_printable = Some((keyval, matches!(action, KeyAction::PassThrough)));
                return action;
            }
            0x2d => {
                state.prev_page(); // `-` 浏览上一页
                state.last_printable = Some((keyval, false));
                return KeyAction::EngineHandled;
            }
            0x3d => {
                state.next_page(); // `=` 浏览下一页
                state.last_printable = Some((keyval, false));
                return KeyAction::EngineHandled;
            }
            // Esc：归下面的 KEY_ESCAPE 分支（退出 + 吞掉；press/release 对称在那）
            KEY_ESCAPE => {}
            // 其余（含空格/回车/退格/字母/`0`/PageUp/Down/Shift）：先退出浏览，
            // 恢复真缓冲后按正常路由处理这个键（缓冲判据全部看真缓冲）。
            _ => state.exit_symbol_browse(),
        }
    }
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
        KEY_TAB => KeyAction::PassThrough,
        // Esc（2026-10-10 从 KEY_TAB 里拆出）：浏览态 → 退出并**吞掉**（lead 裁决
        // 「Esc = exit and swallow」）；非浏览态 → 老行为直通（Tab 仍与 Esc 同档）。
        KEY_ESCAPE => {
            if released {
                // 同退格：抬起按按下记下的结论回复（handle_escape 会退出浏览，
                // 「当前」状态与按下时不同，只能复用记录）
                match state.last_printable {
                    Some((k, true)) if k == keyval => KeyAction::PassThrough,
                    _ => KeyAction::EngineHandled,
                }
            } else {
                let action = handle_escape(state);
                state.last_printable = Some((keyval, matches!(action, KeyAction::PassThrough)));
                action
            }
        }
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
    if !state.has_composing() {
        KeyAction::PassThrough
    } else {
        state.backspace();
        KeyAction::EngineHandled
    }
}

/// 空格：缓冲非空 → 引擎提交；空缓冲 → 直传 " "（镜像 `handleSpace`）。
fn handle_space(state: &mut CandidateState) -> KeyAction {
    if !state.has_composing() {
        // 空格**不宽化**（有意）：全角在拼音模式默认开，宽它会把这个高频动作换成
        // U+3000 —— 见 `Engine::fullwidthize` 头注的「空格不宽化」。
        KeyAction::Input(" ".to_string())
    } else {
        commit_or_handled(state.input_space())
    }
}

/// 回车：缓冲非空 → **按引擎的判据**（`Engine::enter_action`：五笔读「回车行为」
/// 设置位 —— 选首候选 / 清空 / 上屏原文；判据的单一实现在 `engine_enter.rs`）；
/// 空缓冲 → 直通客户端（镜像 `handleEnter`）。
fn handle_enter(state: &mut CandidateState) -> KeyAction {
    if !state.has_composing() {
        KeyAction::PassThrough
    } else if state.buffer().is_empty() {
        // 逐字态收尾（2026-10-17）：片段还在候选区，回车 = 提交它们（同空格）。
        commit_or_handled(state.input_space())
    } else {
        match state.engine.enter_action() {
            EnterAction::SelectFirst => commit_or_handled(state.select(0)),
            EnterAction::Clear => {
                state.clear();
                KeyAction::EngineHandled
            }
            EnterAction::CommitRaw(raw) => {
                state.clear();
                KeyAction::Input(raw)
            }
        }
    }
}

/// Esc：浏览态 → 退出（恢复真缓冲）并**吞掉**（lead 裁决「Esc = exit and swallow」——
/// 浏览是模态，Esc 不该同时把客户端对话框也关掉）；非浏览态 → 直通（老行为，
/// 与 Tab 同档）。不做「二级退一级」的两段式返回：本回合字面执行键集裁决。
fn handle_escape(state: &mut CandidateState) -> KeyAction {
    if state.is_symbol_browsing() {
        state.exit_symbol_browse();
        KeyAction::EngineHandled
    } else {
        KeyAction::PassThrough
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
    // 组字中的 `-` / `=` 翻页（2026-10-09 用户报障「桌面端减号、等号不能翻页候选词」）。
    // 与 PageUp/PageDown 同一动作；**必须排在标点表之前** —— 否则全角兜底会把 `-`
    // 当成标点，组字中按下即提交「首候选＋－」（实测：按 `=` 曾提交「词00＝」）。
    // 只在拼音/繁体（有候选页的模式）且缓冲非空时抢键：空缓冲走标点表/直通老路
    // （不吞键的不变量见 input_method_punct_tests.rs），符号/英文模式另有归属，不碰。
    if matches!(state.mode(), Mode::Pinyin | Mode::Traditional) && !state.buffer().is_empty() {
        match c {
            '-' => {
                state.prev_page();
                return KeyAction::EngineHandled;
            }
            '=' => {
                state.next_page();
                return KeyAction::EngineHandled;
            }
            _ => {}
        }
    }
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
            // 携带缓冲（中→英保留的拼音）：保持旧直传 —— 字母进客户端、缓冲原样
            // 留着切回中文继续用（用户裁决 2026-10-09）；英文自己的缓冲才喂引擎
            // 做前缀匹配。⇧/全角语义与旧空缓冲直传同一条（直传绕过引擎）。
            if state.engine.buffer_is_carried() {
                let text = if state.shift_state() != ShiftState::Off {
                    state.consume_single_shift();
                    c.to_ascii_uppercase().to_string()
                } else {
                    c.to_string()
                };
                KeyAction::Input(state.fullwidthize(&text))
            } else if !state.english_pro() && !state.english_phrases_active() {
                // 普通模式（2026-10-19，默认）：所有键直出，不查词库、不进缓冲。
                // 只有状态栏 `en/pro` 键切到专业模式后才走下面的前缀联想。
                // 例外（2026-10-24）：英文短语可命中时必须缓冲，否则字母已进文档、
                // trigger 永远匹配不到（见 `english_phrases_active`）。
                let text = if state.shift_state() != ShiftState::Off {
                    state.consume_single_shift();
                    c.to_ascii_uppercase().to_string()
                } else {
                    c.to_string()
                };
                KeyAction::Input(state.fullwidthize(&text))
            } else if c.is_ascii_alphabetic() {
                // 空缓冲/英文自己的缓冲：字母入引擎（2026-10-18 单词联想，空缓冲不再
                // 直传）；composer 按引擎 shift 决定大小写，single 用掉即复位。
                let out = state.input_key(c);
                state.consume_single_shift();
                commit_or_handled(out)
            } else if c.is_ascii_digit() && !state.buffer().is_empty() {
                // 英文缓冲有候选 ⇒ 页内选词；无候选 ⇒ `digit_select` 自己直传。
                digit_select(state, c)
            } else if state.buffer().is_empty() {
                // 空缓冲的非字母可打印键：旧直传（数字/标点原样进客户端，全角照旧）。
                KeyAction::Input(state.fullwidthize(&c.to_string()))
            } else if c.is_ascii_punctuation() {
                // 缓冲非空：先按空格语义收尾，标点排在后面（与 `KeyRouter` 同构）。
                commit_or_handled(state.input_space() + &c.to_string())
            } else {
                KeyAction::PassThrough
            }
        }
        // 全角开 ⇒ 数字也出全角（同上裁决）。数字在引擎里是**直通**档（客户端自己插），
        // 所以要在直传点换成全角字符 —— 非 ASCII 图形字符（空格/汉字）恒等，照旧直通。
        Mode::Number if state.fullwidth() && c.is_ascii_graphic() => {
            KeyAction::Input(state.fullwidthize(&c.to_string()))
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
/// 无对应候选（本页没有第 9 项、'0'）不消费，交客户端处理 —— 本函数**不自己数**页内
/// 条数：页内越界由 `select_from` 的页内判据兜住（→ 空串 → 直通）。两处各数一遍正是
/// 缺陷的来源（旧 `select` 只判**全量**越界：数字键走对了、点击候选走错了；用例见
/// `candidate_tests.rs` / `logic_candidate_tests.rs` 的 `select_beyond_page_returns_empty`）。
/// 同一份抓取供选中用：
/// 旧路径 `candidates()` + `select()` 抓两次 ⇒ 每次数字选词排两遍。
fn digit_select(state: &mut CandidateState, c: char) -> KeyAction {
    let fetched = state.fetched();
    if state.mode().digit_selects_candidates() && !state.buffer().is_empty() {
        let Some(idx) = c.to_digit(10).and_then(|d| d.checked_sub(1)) else {
            return KeyAction::PassThrough;
        };
        // 与 `select` 同一套换算与收尾，只是复用上面那次抓取
        let text = state.select_from(&fetched, idx as usize);
        // 空串且**没攒下逐字片段** = 页内没这一项（或无候选）→ 不消费，交客户端；
        // 逐字确认（2026-10-17）的延迟提交同样是「选中了」—— 数字键必须被消费。
        if text.is_empty() && state.engine.pending_text().is_empty() {
            KeyAction::PassThrough
        } else {
            commit_or_handled(text)
        }
    } else {
        KeyAction::PassThrough
    }
}

/// 浏览态数字键（`1`-`9`，区间由调用处保证）：页内索引激活浏览项 —— 类目下钻
/// （无提交文本）/ 符号上屏。复用 `select_from` 的页内换算与收尾；激活语义在引擎层
/// （契约 ③）。**永不直通**：下钻/越界都是空串，照样吞掉（浏览是模态，越界的数字
/// 不该漏给客户端）。配对：tsf-opi/src/logic_input_method.rs 的同名函数。
fn browse_digit(state: &mut CandidateState, keyval: u32) -> KeyAction {
    let idx = (keyval - 0x31) as usize;
    let fetched = state.fetched();
    commit_or_handled(state.select_from(&fetched, idx))
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
#[path = "input_method_browse_tests.rs"]
mod browse_tests;
#[cfg(test)]
#[path = "input_method_carried_tests.rs"]
mod carried_tests;
#[cfg(test)]
#[path = "input_method_enter_tests.rs"]
mod enter_tests;
#[cfg(test)]
#[path = "input_method_page_tests.rs"]
mod page_tests;
#[cfg(test)]
#[path = "input_method_punct_tests.rs"]
mod punct_tests;
#[cfg(test)]
#[path = "input_method_release_tests.rs"]
mod release_tests;
#[cfg(test)]
#[path = "input_method_switches_tests.rs"]
mod switches_tests;
#[cfg(test)]
#[path = "input_method_symbol_tests.rs"]
mod symbol_tests;
#[cfg(test)]
#[path = "input_method_tests.rs"]
mod tests;
