// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

/// 输入模式。V1 固定四模式（简繁共五种），双拼/五笔经 InputScheme 扩展（V2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Pinyin,
    /// 繁体模式：行为同 Pinyin（小写入缓冲、shift 无效），引擎层路由到 trad 词典。
    Traditional,
    English,
    Number,
    Symbol,
}

impl Mode {
    /// 数字键在这几个模式下是**选词键**（页内索引）：缓冲里是待检索的关键字/音节，
    /// 候选栏正是用户想要的。英文/数字模式不选词（缓冲里是原文，数字该进文档）。
    ///
    /// 定义在 `Mode` 上而不是三轨各写一份 `matches!`：那三份列表逐字重复、必漂；
    /// 且 `state.mode()` 比 `self.mode()` 长一字符就顶破 rustfmt 的宏参数宽度上限，
    /// 会逼其中一轨单独折行 —— 同构的源码形状跟着破掉。
    pub fn digit_selects_candidates(self) -> bool {
        matches!(self, Mode::Pinyin | Mode::Traditional | Mode::Symbol)
    }

    /// 进入该模式时**全角开关**的默认值：中文模式自动全角（用户裁决 2026-09-27
    /// 「除自动全角外，再加一个全角↔半角切换键」），其余模式半角。
    /// 切模式按模式默认值重置 —— 与 `switch_mode` 清 shift 同一条理由：跨模式残留的
    /// 粘滞态会让用户「切回来发现打字变成另一个样子」。
    ///
    /// 与上一条同样定义在 `Mode` 上：三轨 + 引擎各写一份 `matches!` 必漂。
    pub fn default_fullwidth(self) -> bool {
        matches!(self, Mode::Pinyin | Mode::Traditional)
    }
}

/// 拼音缓冲上限：乱码拼音（非合法音节序列）无候选时不再无限累积。
pub const MAX_BUFFER: usize = 16;

/// 一次击键的效果。提交由 Engine 层统一处理（空格键），Composer 只区分更新/忽略。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyEffect {
    /// 缓冲更新（未提交）。
    Updated,
    /// 按键被忽略（如拼音模式收到非字母）。
    Ignored,
}

/// 输入会话的不可变快照。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Session {
    pub mode: Mode,
    pub buffer: String,
    pub shift: bool,
}

pub struct Composer {
    session: Session,
}

impl Composer {
    pub fn new() -> Self {
        Composer {
            session: Session::default(),
        }
    }

    /// 处理一次击键，返回效果与新的会话快照。
    pub fn input_key(&mut self, ch: char) -> (KeyEffect, Session) {
        use KeyEffect::*;
        let effect = match self.session.mode {
            Mode::Pinyin | Mode::Traditional => {
                if self.session.buffer.chars().count() >= MAX_BUFFER {
                    Ignored
                } else if ch.is_ascii_lowercase() || ch == '\'' {
                    self.session.buffer.push(ch);
                    Updated
                } else if ch.is_ascii_uppercase() {
                    self.session.buffer.push(ch.to_ascii_lowercase());
                    Updated
                } else {
                    Ignored
                }
            }
            Mode::English => {
                // 与 Pinyin 同一上限：缓冲是 UI 的 preedit 来源，无上限则可无限增长。
                // Android 侧英文模式空缓冲直传、不进引擎，故当前不可达；但这是引擎
                // 的公开契约，其它前端（fcitx5/tsf）不该依赖调用方自觉。
                if self.session.buffer.chars().count() >= MAX_BUFFER {
                    Ignored
                } else if ch.is_ascii_alphabetic() {
                    if self.session.shift {
                        self.session.buffer.push(ch.to_ascii_uppercase());
                    } else {
                        self.session.buffer.push(ch);
                    }
                    Updated
                } else {
                    Ignored
                }
            }
            Mode::Number => {
                if self.session.buffer.chars().count() >= MAX_BUFFER {
                    Ignored
                } else if ch.is_ascii_digit() {
                    self.session.buffer.push(ch);
                    Updated
                } else {
                    Ignored
                }
            }
            // 符号模式：缓冲是**关键字**（dun → 、、comma → ，），由 SymbolEngine 的
            // 关键字索引搜候选 —— 与拼音缓冲同构，故同一条 MAX_BUFFER 上限。
            // 只收**字母数字**：生产表 583 条的关键字一律 `^[a-z0-9]+$`（实测），
            // 标点/空格/控制符/非 ASCII 都不是关键字 —— 收进来只会得到一个搜不出候选的
            // 死缓冲，而标点另有出路（标点表 → 原样交回调用方）。这条边界由
            // tests/punctuation.rs 的 every_symbol_keyword_is_alnum 与
            // tests/adversarial_input.rs 的期望表一起钉住。
            // 大小写不折叠：搜索侧 `to_lowercase` 已归一。
            Mode::Symbol => {
                if self.session.buffer.chars().count() >= MAX_BUFFER {
                    Ignored
                } else if ch.is_ascii_alphanumeric() {
                    self.session.buffer.push(ch);
                    Updated
                } else {
                    Ignored
                }
            }
        };
        (effect, self.session.clone())
    }

    pub fn backspace(&mut self) -> Session {
        self.session.buffer.pop();
        self.session.clone()
    }

    pub fn clear(&mut self) -> Session {
        self.session.buffer.clear();
        self.session.clone()
    }

    pub fn set_shift(&mut self, on: bool) -> Session {
        self.session.shift = on;
        self.session.clone()
    }

    /// 切换模式会清空缓冲与 shift（⇧ 是「下一个键」的粘滞态，跨模式残留会让
    /// 切回英文后打出的全是大写）。
    pub fn switch_mode(&mut self, mode: Mode) -> Session {
        self.session.mode = mode;
        self.session.buffer.clear();
        self.session.shift = false;
        self.session.clone()
    }

    /// 提交当前缓冲（不记录学习，由 Engine 层处理）。
    pub fn commit_buffer(&mut self) -> Session {
        self.session.buffer.clear();
        self.session.clone()
    }

    pub fn session(&self) -> &Session {
        &self.session
    }
}

impl Default for Composer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinyin_lowercases_letters_and_keeps_apostrophe() {
        let mut c = Composer::new();
        let (eff, s) = c.input_key('N');
        assert_eq!(eff, KeyEffect::Updated);
        assert_eq!(s.buffer, "n");
        let (_, s) = c.input_key('i');
        assert_eq!(s.buffer, "ni");
        let (_, s) = c.input_key('\'');
        assert_eq!(s.buffer, "ni'");
    }

    #[test]
    fn pinyin_ignores_digits_and_symbols() {
        let mut c = Composer::new();
        c.input_key('x');
        let (eff, s) = c.input_key('1');
        assert_eq!(eff, KeyEffect::Ignored);
        assert_eq!(s.buffer, "x");
    }

    #[test]
    fn traditional_accepts_letters_like_pinyin() {
        let mut c = Composer::new();
        c.switch_mode(Mode::Traditional);
        let (eff, s) = c.input_key('N');
        assert_eq!(eff, KeyEffect::Updated);
        assert_eq!(s.buffer, "n");
        let (eff, s) = c.input_key('\'');
        assert_eq!(eff, KeyEffect::Updated);
        assert_eq!(s.buffer, "n'");
    }

    #[test]
    fn english_respects_shift() {
        let mut c = Composer::new();
        c.switch_mode(Mode::English);
        c.set_shift(true);
        let (_, s) = c.input_key('a');
        assert_eq!(s.buffer, "A");
        c.set_shift(false);
        let (_, s) = c.input_key('b');
        assert_eq!(s.buffer, "Ab");
    }

    #[test]
    fn number_mode_only_digits() {
        let mut c = Composer::new();
        c.switch_mode(Mode::Number);
        let (eff, _) = c.input_key('a');
        assert_eq!(eff, KeyEffect::Ignored);
        let (_, s) = c.input_key('2');
        assert_eq!(s.buffer, "2");
        let (_, s) = c.input_key('0');
        assert_eq!(s.buffer, "20");
    }

    /// 前身是 `symbol_mode_ignores_all`（断言 Symbol 模式一律 `Ignored`）。**断言与理由
    /// 一起改了**：当时 `Mode::Symbol` 是空壳 —— 没有任何候选通路（`candidates.rs` 只放行
    /// 拼音/繁体），缓冲里放什么都不会产生候选，`Ignored` 是对「这个模式没有输入语义」的
    /// 诚实表达。符号模式补成真模式后，缓冲承载**关键字**（`dun` → 、、`comma` → ，），
    /// 由 `SymbolEngine` 的关键字索引搜候选，与拼音的缓冲同构。
    ///
    /// 被忽略的那一半一条没少，只是多放行了一类：空格（提交键，`Engine::input_key` 先拦）、
    /// 控制符、非 ASCII 仍全部 `Ignored`。
    #[test]
    fn symbol_mode_takes_printable_keywords() {
        let mut c = Composer::new();
        c.switch_mode(Mode::Symbol);
        for ch in ['d', 'u', 'n'] {
            assert_eq!(
                c.input_key(ch).0,
                KeyEffect::Updated,
                "{ch:?} 应进关键字缓冲"
            );
        }
        assert_eq!(c.session().buffer, "dun");
        // 标点不再进关键字缓冲（2026-09-27 标点表）：生产表关键字全是 ^[a-z0-9]+$，
        // 标点搜不出候选；它现在走「标点表 / 原样交回调用方」那条路，不再变死缓冲。
        for ch in [' ', '\t', '\u{7f}', '中', '，', '😄', ',', '.', '-'] {
            assert_eq!(
                c.input_key(ch).0,
                KeyEffect::Ignored,
                "{ch:?} 不是关键字，不得进缓冲"
            );
        }
        assert_eq!(c.session().buffer, "dun");
    }

    #[test]
    fn switch_mode_clears_buffer() {
        let mut c = Composer::new();
        c.input_key('n');
        c.switch_mode(Mode::English);
        assert_eq!(c.session().buffer, "");
        assert_eq!(c.session().mode, Mode::English);
    }

    #[test]
    fn switch_mode_clears_shift() {
        let mut c = Composer::new();
        c.switch_mode(Mode::English);
        c.set_shift(true);
        c.switch_mode(Mode::Number);
        c.switch_mode(Mode::English);
        assert!(
            !c.session().shift,
            "切模式须清 shift，否则 ⇧ 锁定态跨模式残留"
        );
        // 行为面：切回英文后打出的必须是小写
        let (_, s) = c.input_key('a');
        assert_eq!(s.buffer, "a");
    }

    #[test]
    fn backspace_removes_last_char() {
        let mut c = Composer::new();
        c.input_key('n');
        c.input_key('i');
        let s = c.backspace();
        assert_eq!(s.buffer, "n");
    }

    #[test]
    fn commit_buffer_clears() {
        let mut c = Composer::new();
        c.input_key('n');
        c.input_key('i');
        let s = c.commit_buffer();
        assert_eq!(s.buffer, "");
        assert_eq!(c.session().buffer, "");
    }

    #[test]
    fn clear_empties_buffer() {
        let mut c = Composer::new();
        c.input_key('n');
        let s = c.clear();
        assert_eq!(s.buffer, "");
    }
}
