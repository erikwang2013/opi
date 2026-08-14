/// 输入模式。V1 固定四模式，双拼/五笔经 InputScheme 扩展（V2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Pinyin,
    English,
    Number,
    Symbol,
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
        Composer { session: Session::default() }
    }

    /// 处理一次击键，返回效果与新的会话快照。
    pub fn input_key(&mut self, ch: char) -> (KeyEffect, Session) {
        use KeyEffect::*;
        let effect = match self.session.mode {
            Mode::Pinyin => {
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
                if ch.is_ascii_alphabetic() {
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
                if ch.is_ascii_digit() {
                    self.session.buffer.push(ch);
                    Updated
                } else {
                    Ignored
                }
            }
            Mode::Symbol => Ignored,
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

    /// 切换模式会清空缓冲。
    pub fn switch_mode(&mut self, mode: Mode) -> Session {
        self.session.mode = mode;
        self.session.buffer.clear();
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

    #[test]
    fn symbol_mode_ignores_all() {
        let mut c = Composer::new();
        c.switch_mode(Mode::Symbol);
        let (eff, _) = c.input_key('a');
        assert_eq!(eff, KeyEffect::Ignored);
        assert_eq!(c.session().buffer, "");
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
