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

use engine_core::EnterAction;
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
    /// - `PageUp`/`PageDown` 翻页；**组字中（拼音/繁体且缓冲非空）`-` 上一页、`=` 下一页**
    ///   （2026-10-09 用户报障补上；空缓冲不抢键，见 logic_page_tests.rs）。
    /// - 符号模式：可见 ASCII 入引擎缓冲当**关键字**（符号候选由此搜出），其余放行。
    /// - **符号浏览**（「符号大全」叠加态，2026-10-10）：浏览期间只有 `1`-`9`（激活浏览项：
    ///   类目下钻 / 符号上屏）、`-`/`=`（浏览翻页）、Esc（退出并吞掉）归浏览；**其余一律先
    ///   退出浏览恢复真缓冲、再按正常路由处理那个键** —— 绝不让「符号」标题落进正常路由
    ///   （契约 ②，见 engine-core/src/engine_symbol_browse.rs 头）。括号键 `0`、PageUp/Down、
    ///   Shift 等都在「其余」里（字面执行 lead 裁决的键集，别自作主张扩）。
    /// - Ctrl/Alt 组合键一律 Unhandled（系统快捷键，不拦截）—— 进入/退出浏览由宿主直接
    ///   调状态方法（见 logic.rs 那块注释；Windows 侧本回合无快捷键接线）。
    pub fn input_key(&mut self, keyval: u32, key_state: u32) -> KeyOutcome {
        // Ctrl/Alt 组合键（系统快捷键）不拦截，交应用处理。
        if key_state & (KEY_STATE_CTRL | KEY_STATE_ALT) != 0 {
            return KeyOutcome::Unhandled;
        }
        let released = key_state & KEY_STATE_RELEASED != 0;
        // 符号浏览（契约 ②，见 engine_symbol_browse.rs 头）：**按下**时只有 `1`-`9`、
        // `-`/`=`、Esc 归浏览；其余键先把浏览退出（恢复真缓冲）再落进下面的正常路由 ——
        // 「退出」放在这里（全部按键之前）而不是各分支里，单点免得某个键漏掉。
        // 抬起不在此处理：各键的 press/release 对称由下方 last_printable 机制管。
        if !released && self.is_symbol_browsing() {
            // 三个提前 return 的分支必须记 `last_printable`：不记的话，抬起会命中
            // **上一次**同名键留下的记录（可能 true）→ 放行一个按下时被拦下的 keyup
            // （与退格/回车那两处的理由逐字相同）。浏览态一律不直通。
            match keyval {
                // `1`-`9`：激活浏览项（页内索引，与候选数字键同一套换算）
                0x31..=0x39 => {
                    let outcome = self.browse_digit(keyval);
                    self.last_printable = Some((keyval, matches!(outcome, KeyOutcome::Unhandled)));
                    return outcome;
                }
                0x2d => {
                    self.prev_page(); // `-` 浏览上一页
                    self.last_printable = Some((keyval, false));
                    return KeyOutcome::CompositionChanged;
                }
                0x3d => {
                    self.next_page(); // `=` 浏览下一页
                    self.last_printable = Some((keyval, false));
                    return KeyOutcome::CompositionChanged;
                }
                // Esc：归下面的 KEY_ESCAPE 分支（退出 + 吞掉；press/release 对称在那）
                KEY_ESCAPE => {}
                // 其余（含空格/回车/退格/字母/`0`/PageUp/Down/Shift）：先退出浏览，
                // 恢复真缓冲后按正常路由处理这个键（缓冲判据全部看真缓冲）。
                _ => self.exit_symbol_browse(),
            }
        }
        match keyval {
            KEY_BACK_SPACE | KEY_DELETE => {
                if released {
                    // 抬起按**按下时记下的**结论回复（`last_printable`）——不能判「当前」
                    // 缓冲：按下分支 handle_backspace 会把它改空（删掉最后一个字符），
                    // 届时再判 buffer().is_empty() 就会放行一个按下时吞下的键 →
                    // 应用收到无 keydown 的 keyup（依赖键状态的游戏/编辑器卡键）。
                    match self.last_printable {
                        Some((k, true)) if k == keyval => KeyOutcome::Unhandled,
                        // 键值不匹配（记录被另一个键顶掉）或从未按下 → 按消费拦下
                        _ => KeyOutcome::Consumed,
                    }
                } else {
                    let outcome = self.handle_backspace();
                    self.last_printable = Some((keyval, matches!(outcome, KeyOutcome::Unhandled)));
                    outcome
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
                    // 同退格：抬起按按下记下的结论回复（handle_enter 提交后会清空缓冲）
                    match self.last_printable {
                        Some((k, true)) if k == keyval => KeyOutcome::Unhandled,
                        _ => KeyOutcome::Consumed,
                    }
                } else {
                    let outcome = self.handle_enter();
                    self.last_printable = Some((keyval, matches!(outcome, KeyOutcome::Unhandled)));
                    outcome
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
            KEY_TAB => KeyOutcome::Unhandled,
            // Esc（2026-10-10 从 KEY_TAB 里拆出）：浏览态 → 退出并**吞掉**（lead 裁决
            // 「Esc = exit and swallow」）；非浏览态 → 老行为 Unhandled（Tab 仍与 Esc 同档）。
            KEY_ESCAPE => {
                if released {
                    // 同退格：抬起按按下记下的结论回复（handle_escape 会退出浏览，
                    // 「当前」状态与按下时不同，只能复用记录）
                    match self.last_printable {
                        Some((k, true)) if k == keyval => KeyOutcome::Unhandled,
                        _ => KeyOutcome::Consumed,
                    }
                } else {
                    let outcome = self.handle_escape();
                    self.last_printable = Some((keyval, matches!(outcome, KeyOutcome::Unhandled)));
                    outcome
                }
            }
            _ => match char::from_u32(keyval) {
                // 抬起必须判：本函数上面每个特殊键分支都判了 `released`，可打印
                // 分支此前漏判，导致同一个字符被第二次送进引擎 —— 拼音缓冲翻倍
                // （"ni"→"nnii"）、英文模式重复提交（"a"→"aa"）。
                // 抬起时的结论用按下时记下的（self.last_printable），与按下同判：
                // 可打印键里有放行字符（拼音/繁体的非字母符号、无候选或越界的数字、
                // Number 模式下的全部可见 ASCII、Symbol 模式下的控制符/非 ASCII → handle_printable 返回
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
        if !self.has_composing() {
            KeyOutcome::Unhandled
        } else {
            self.backspace();
            KeyOutcome::CompositionChanged
        }
    }

    /// 空格：缓冲非空 → 引擎提交；空缓冲 → 提交 " "（镜像 `handleSpace`）。
    fn handle_space(&mut self) -> KeyOutcome {
        if !self.has_composing() {
            // 空格**不宽化**（有意）：全角在拼音模式默认开，宽它会把这个高频动作换成
            // U+3000 —— 见 `Engine::fullwidthize` 头注的「空格不宽化」。
            KeyOutcome::Commit(" ".to_string())
        } else {
            commit_or_changed(self.input_space())
        }
    }

    /// 回车：缓冲非空 → **按引擎的判据**（`Engine::enter_action`：五笔读「回车行为」
    /// 设置位 —— 选首候选 / 清空 / 上屏原文；判据的单一实现在 `engine_enter.rs`）；
    /// 空缓冲 → 交应用（镜像 `handleEnter`）。
    fn handle_enter(&mut self) -> KeyOutcome {
        if !self.has_composing() {
            KeyOutcome::Unhandled
        } else if self.buffer().is_empty() {
            // 逐字态收尾（2026-10-17）：片段还在候选区，回车 = 提交它们（同空格）。
            commit_or_changed(self.input_space())
        } else {
            match self.engine.enter_action() {
                EnterAction::SelectFirst => commit_or_changed(self.select(0)),
                EnterAction::Clear => {
                    self.clear();
                    KeyOutcome::Consumed
                }
                EnterAction::CommitRaw(raw) => {
                    self.clear();
                    KeyOutcome::Commit(raw)
                }
            }
        }
    }

    /// Esc：浏览态 → 退出（恢复真缓冲）并**吞掉**（lead 裁决「Esc = exit and swallow」——
    /// 浏览是模态，Esc 不该同时把应用对话框也关掉）；非浏览态 → Unhandled（老行为，
    /// 与 Tab 同档）。不做「二级退一级」的两段式返回：本回合字面执行键集裁决。
    fn handle_escape(&mut self) -> KeyOutcome {
        if self.is_symbol_browsing() {
            self.exit_symbol_browse();
            KeyOutcome::Consumed
        } else {
            KeyOutcome::Unhandled
        }
    }

    // ⇧ 键的路由在 `logic_shift.rs`：handle_shift 与两套 Shift 状态（轻按 armed /
    // 屏上键盘三态）同在一处；上面的 `KEY_SHIFT =>` 臂直接调它。

    /// 可见 ASCII 字符按模式分流。
    fn handle_printable(&mut self, c: char) -> KeyOutcome {
        // 组字中的 `-` / `=` 翻页（2026-10-09 用户报障「桌面端减号、等号不能翻页候选词」）。
        // 与 PageUp/PageDown 同一动作；**必须排在标点表之前** —— 否则全角兜底会把 `-`
        // 当成标点，组字中按下即提交「首候选＋－」（实测：按 `=` 曾提交「词00＝」）。
        // 只在拼音/繁体（有候选页的模式）且缓冲非空时抢键：空缓冲走标点表/直通老路，
        // 符号/英文模式另有归属，不碰。与 fcitx5 轨逐行同构（input_method.rs）。
        if matches!(self.mode(), Mode::Pinyin | Mode::Traditional) && !self.buffer().is_empty() {
            match c {
                '-' => {
                    self.prev_page();
                    return KeyOutcome::CompositionChanged;
                }
                '=' => {
                    self.next_page();
                    return KeyOutcome::CompositionChanged;
                }
                _ => {}
            }
        }
        // 标点表（引擎层，四端同一条路）：中文模式出中文标点/全角，其余模式下不映射。
        // 放在模式分派**之前**：标点不属于任何模式的关键字/缓冲语义。
        if let Some(text) = self.input_punct(c) {
            return commit_or_changed(text);
        }
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
                // 携带缓冲（中→英保留的拼音）：保持旧直传 —— 字母进客户端、缓冲原样
                // 留着切回中文继续用（用户裁决 2026-10-09）；英文自己的缓冲才喂引擎
                // 做前缀匹配。⇧/全角语义与旧空缓冲直传同一条（直传绕过引擎）。
                if self.engine.buffer_is_carried() {
                    let text = if self.shift_state() != ShiftState::Off {
                        self.consume_single_shift();
                        c.to_ascii_uppercase().to_string()
                    } else {
                        c.to_string()
                    };
                    KeyOutcome::Commit(self.fullwidthize(&text))
                } else if !self.english_pro() {
                    // 普通模式（默认，镜像 fcitx5 轨）：所有键直出、不查词库、不进缓冲。
                    // 只有状态栏 `en/pro` 键切到专业模式后才走下面的前缀联想。
                    let text = if self.shift_state() != ShiftState::Off {
                        self.consume_single_shift();
                        c.to_ascii_uppercase().to_string()
                    } else {
                        c.to_string()
                    };
                    KeyOutcome::Commit(self.fullwidthize(&text))
                } else if c.is_ascii_alphabetic() {
                    // 空缓冲/英文自己的缓冲：字母入引擎（2026-10-18 单词联想，空缓冲不再
                    // 直传）；composer 按引擎 shift 决定大小写，single 用掉即复位。
                    let out = self.input_char(c);
                    self.consume_single_shift();
                    commit_or_changed(out)
                } else if c.is_ascii_digit() && !self.buffer().is_empty() {
                    // 英文缓冲有候选 ⇒ 页内选词；无候选 ⇒ `digit_select` 自己直传。
                    self.digit_select(c)
                } else if self.buffer().is_empty() {
                    // 空缓冲的非字母可打印键：旧直传（数字/标点原样进客户端，全角照旧）。
                    KeyOutcome::Commit(self.fullwidthize(&c.to_string()))
                } else if c.is_ascii_punctuation() {
                    // 缓冲非空：先按空格语义收尾，标点排在后面（与 `KeyRouter` 同构）。
                    commit_or_changed(self.input_space() + &c.to_string())
                } else {
                    KeyOutcome::Unhandled
                }
            }
            // 全角开 ⇒ 数字也出全角（同上裁决）。数字在引擎里是**直通**档（客户端自己插），
            // 所以要在直传点换成全角字符 —— 非 ASCII 图形字符（空格/汉字）恒等，照旧直通。
            Mode::Number if self.fullwidth() && c.is_ascii_graphic() => {
                KeyOutcome::Commit(self.fullwidthize(&c.to_string()))
            }
            Mode::Number => KeyOutcome::Unhandled,
            // 符号模式（B3）：可见 ASCII 当关键字入缓冲（dun → 、）；数字**先**当选词键
            // （裁决 2026-09-27，与拼音一致）；空格/控制符/非 ASCII 放行。
            Mode::Symbol if c.is_ascii_digit() => self.digit_select(c),
            Mode::Symbol if c.is_ascii_graphic() => commit_or_changed(self.input_char(c)),
            Mode::Symbol => KeyOutcome::Unhandled,
        }
    }

    /// 拼音/繁体/符号模式有候选时按数字选词（页内索引：'1'→第 0 个候选）；否则直通。
    /// 无对应候选（本页没有第 9 项、'0'）不消费，交客户端处理 —— 本函数**不自己数**页内
    /// 条数：页内越界由 `select_from` 的页内判据兜住（→ 空串 → 直通）。两处各数一遍正是
    /// 缺陷的来源（旧 `select` 只判**全量**越界：数字键走对了、点击候选走错了；用例见
    /// `candidate_tests.rs` / `logic_candidate_tests.rs` 的 `select_beyond_page_returns_empty`）。
    /// 同一份抓取供选中用：
    /// 旧路径 `candidates()` + `select()` 抓两次 ⇒ 每次数字选词排两遍。
    fn digit_select(&mut self, c: char) -> KeyOutcome {
        let fetched = self.fetched();
        if self.mode().digit_selects_candidates() && !self.buffer().is_empty() {
            let Some(idx) = c.to_digit(10).and_then(|d| d.checked_sub(1)) else {
                return KeyOutcome::Unhandled;
            };
            // 与 `select` 同一套换算与收尾，只是复用上面那次抓取
            let text = self.select_from(&fetched, idx as usize);
            // 空串且**没攒下逐字片段** = 页内没这一项（或无候选）→ 不消费，交客户端；
            // 逐字确认（2026-10-17）的延迟提交同样是「选中了」—— 数字键必须被消费。
            if text.is_empty() && self.engine.pending_text().is_empty() {
                KeyOutcome::Unhandled
            } else {
                commit_or_changed(text)
            }
        } else {
            KeyOutcome::Unhandled
        }
    }

    /// 浏览态数字键（`1`-`9`，区间由调用处保证）：页内索引激活浏览项 —— 类目下钻
    /// （无提交文本）/ 符号上屏。复用 `select_from` 的页内换算与收尾；激活语义在引擎层
    /// （契约 ③）。**永不直通**：下钻/越界都是空串，照样吞掉（浏览是模态，越界的数字
    /// 不该漏给应用）—— 空串经 commit_or_changed 落 CompositionChanged（下钻改了面板，
    /// 本轨要刷新）。配对：fcitx5-opi/src/input_method.rs 的同名函数。
    fn browse_digit(&mut self, keyval: u32) -> KeyOutcome {
        let idx = (keyval - 0x31) as usize;
        let fetched = self.fetched();
        commit_or_changed(self.select_from(&fetched, idx))
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
// 路由测试，logic_release_tests.rs 为可打印键「按下/抬起同判」回归测试，
// logic_browse_tests.rs 为符号浏览键路由门禁（配对 fcitx5 轨 input_method_browse_tests.rs）。
#[cfg(test)]
#[path = "logic_browse_tests.rs"]
mod browse_tests;
#[cfg(test)]
#[path = "logic_carried_tests.rs"]
mod carried_tests;
#[cfg(test)]
#[path = "logic_enter_tests.rs"]
mod enter_tests;
#[cfg(test)]
#[path = "logic_page_tests.rs"]
mod page_tests;
#[cfg(test)]
#[path = "logic_punct_tests.rs"]
mod punct_tests;
#[cfg(test)]
#[path = "logic_release_tests.rs"]
mod release_tests;
#[cfg(test)]
#[path = "logic_switches_tests.rs"]
mod switches_tests;
#[cfg(test)]
#[path = "logic_symbol_tests.rs"]
mod symbol_tests;
#[cfg(test)]
#[path = "logic_tests.rs"]
mod tests;
