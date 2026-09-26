// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 键路由：`TsfLogic` 的按键分流（路由表见 `input_key` 的文档）。
//! 本文件 = fcitx5 轨的 `input_method.rs`，父模块 `logic` = fcitx5 轨的
//! `candidate.rs`（状态 + 分页）；两轨逐行同构：方法名与顺序逐条对应。
//!
//! 键码约定：可打印字符 = Unicode 码点（与 fcitx5 轨一致，如 'a'=97）；
//! 特殊键 = `SPECIAL_BASE | Windows VK`（TSF 键事件 wParam 同源），见下方常量。
//! **两段空间不相交**——这是硬约束，成因见 `SPECIAL_BASE`。
//! 键状态位沿用 fcitx5 轨的位约定（内部契约，C2 自 TSF 侧换算）。
//!
//! TSF 现实的适配：TSF 下按键总是先经服务处理，无 fcitx5 式"直通客户端"
//! 概念；空缓冲退格/回车、Ctrl/Alt 组合等返回 `Unhandled`，由 C2 决定
//! 是否交应用（不拦截则键自然流入）。

use engine_core::composer::Mode;

use crate::logic::{ShiftState, TsfLogic};

// ---------- 特殊键：键码 = SPECIAL_BASE | Windows VK（wParam，与 TSF 键事件同源） ----------

/// 特殊键键码的基址。**特殊键空间与「可打印字符 = Unicode 码点」必须不相交。**
///
/// 裸用 VK 会撞 ASCII：VK_PRIOR=0x21=`!`、VK_NEXT=0x22=`"`、VK_DELETE=0x2E=`.`、
/// VK_SPACE=0x20=' '（恰好同义）。后果不是"多认了一个键"而是**普通字符被吃掉**：
/// 拼音缓冲非空时敲 `.` 会走退格分支删掉拼音字母，敲 `!`/`"` 会翻候选页。
///
/// 取 0x1_0000（补充平面）：编码后的键码即便将来漏了 match 臂、掉进可打印分支，
/// `char::from_u32` 得到的也是非 ASCII 的补充平面字符，会被 `Some(c) if c.is_ascii()`
/// 挡回 `_ => Unhandled` 放行给应用，**不会静默变成垃圾字符**（`vk.rs` 的
/// `special_key_space_stays_out_of_ascii` 用不变式钉住这一点）。
/// **`is_ascii()` 不等于「可打印」**：0x08/0x09/0x0D/0x1B 等控制符也为真 —— 旧约定下
/// 这些键漏了 match 臂时，英文模式空缓冲会 `Commit` 一个控制符进文档（不止是吞掉一个键）。
/// 低 16 位留 VK 原值，解码见 `vk::special_vk`。
pub const SPECIAL_BASE: u32 = 0x1_0000;

/// VK_BACK（退格）。
pub const KEY_BACK_SPACE: u32 = SPECIAL_BASE | 0x08;
/// VK_TAB。
pub const KEY_TAB: u32 = SPECIAL_BASE | 0x09;
/// VK_RETURN（回车）。
pub const KEY_RETURN: u32 = SPECIAL_BASE | 0x0d;
/// VK_ESCAPE。
pub const KEY_ESCAPE: u32 = SPECIAL_BASE | 0x1b;
/// VK_PRIOR（PageUp → 上一页候选）。
pub const KEY_PAGE_UP: u32 = SPECIAL_BASE | 0x21;
/// VK_NEXT（PageDown → 下一页候选）。
pub const KEY_PAGE_DOWN: u32 = SPECIAL_BASE | 0x22;
/// VK_DELETE。
pub const KEY_DELETE: u32 = SPECIAL_BASE | 0x2e;
/// VK_SHIFT（左右 ⇧ 同为 0x10，与 fcitx5 轨的 SHIFT_L/SHIFT_R 二码不同）。
pub const KEY_SHIFT: u32 = SPECIAL_BASE | 0x10;
/// 空格（VK_SPACE=0x20，与 ASCII 空格同值）。
pub const KEY_SPACE: u32 = SPECIAL_BASE | 0x20;

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

impl TsfLogic {
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
                    // 抬起须与按下同判（见 handle_backspace）：按下放行、抬起拦下会让
                    // 应用收到 keydown 收不到 keyup，依赖键状态的游戏/编辑器会卡键。
                    // 只在需要时取缓冲 —— buffer() 会分配 String，提到 match 之前等于给
                    // 每个按键都加一次分配。可打印分支的反向不对称是既有的有意取舍。
                    if self.buffer().is_empty() {
                        KeyOutcome::Unhandled
                    } else {
                        KeyOutcome::Consumed
                    }
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
                    // 同退格：抬起与按下同判（见 handle_enter）
                    if self.buffer().is_empty() {
                        KeyOutcome::Unhandled
                    } else {
                        KeyOutcome::Consumed
                    }
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
                // 抬起必须判：本函数上面每个特殊键分支都判了 `released`，可打印
                // 分支此前漏判，导致同一个字符被第二次送进引擎 —— 拼音缓冲翻倍
                // （"ni"→"nnii"）、英文模式重复提交（"a"→"aa"）。
                // 抬起时的结论用按下时记下的（self.last_printable），与按下同判：
                // 可打印键里有放行字符（拼音/繁体的非字母符号、无候选或越界的数字、
                // Number/Symbol 模式下的全部可见 ASCII → handle_printable 返回
                // Unhandled），按下放行、抬起拦下会让应用收到 keydown 收不到 keyup。
                // 记结论而非复刻 handle_printable 的分流判定：判定只有一处，不会漂移。
                Some(c) if c.is_ascii() => {
                    if released {
                        match self.last_printable {
                            Some((k, true)) if k == keyval => KeyOutcome::Unhandled,
                            // 键值不匹配（记录被另一个键顶掉）或从未按下 → 按消费拦下
                            _ => KeyOutcome::Consumed,
                        }
                    } else {
                        let outcome = self.handle_printable(c);
                        self.last_printable =
                            Some((keyval, matches!(outcome, KeyOutcome::Unhandled)));
                        outcome
                    }
                }
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
            // 繁体模式与拼音同构：字母/撇号入缓冲、数字选词、走引擎。
            Mode::Pinyin | Mode::Traditional => {
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
        if matches!(self.mode(), Mode::Pinyin | Mode::Traditional)
            && !self.buffer().is_empty()
            && !self.candidates().is_empty()
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

// 单测独立成文件（`#[path]` 引入）以保持各文件 <500 行：logic_tests.rs 为键
// 路由测试，logic_release_tests.rs 为可打印键「按下/抬起同判」回归测试。
#[cfg(test)]
#[path = "logic_release_tests.rs"]
mod release_tests;
#[cfg(test)]
#[path = "logic_tests.rs"]
mod tests;
