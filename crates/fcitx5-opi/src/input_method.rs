//! 按键路由骨架（B1）。
//!
//! B2 将在此实现完整路由表（拼音键/候选选择/翻页/功能键）；
//! B1 只提供结构占位：`handle_key` 返回动作枚举，ASCII 可见字符路由到
//! 引擎，其余忽略。动作枚举为 B2 的增长点（届时增加 Commit/Select/
//! PageUp/PageDown 等变体）。

use crate::candidate::CandidateState;

/// 按键处理结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    /// 已路由到引擎，携带引擎输出（如英文模式已提交文本，通常为空串）。
    Input(String),
    /// 未处理（B1 暂忽略，B2 起进入路由表）。
    Ignored,
}

/// 处理单字符键。B1 规则：仅 ASCII 可见字符进入引擎，其余忽略。
pub fn handle_key(state: &mut CandidateState, ch: char) -> KeyAction {
    if !ch.is_ascii() {
        return KeyAction::Ignored;
    }
    KeyAction::Input(state.input_key(ch))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> CandidateState {
        CandidateState::load(None).expect("fallback load")
    }

    #[test]
    fn ascii_routes_to_engine() {
        let mut s = state();
        assert_eq!(handle_key(&mut s, 'a'), KeyAction::Input(String::new()));
        assert_eq!(s.buffer(), "a");
    }

    #[test]
    fn non_ascii_is_ignored() {
        let mut s = state();
        assert_eq!(handle_key(&mut s, '你'), KeyAction::Ignored);
        assert_eq!(s.buffer(), "");
        // 空格属 ASCII，B1 路由到引擎（空格 → input_space 在 B2 路由表中）
        assert_eq!(handle_key(&mut s, ' '), KeyAction::Input(String::new()));
    }
}
