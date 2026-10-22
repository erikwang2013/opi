// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `OnTestKeyDown` 的「这一键引擎会不会消费」判定（从 `logic.rs` 拆出守 500 行）。
//!
//! TSF 的 `OnTestKeyDown` 是**最终决定**：答 TRUE 后再在 `OnKeyDown` 里答 FALSE
//! 不会把键还给应用（2026-10-22 真机报障「F1–F12/媒体键无效」的根因）。所以
//! 测试阶段必须与引擎的实际分流一致；本模块把那个分流做成**只读**判定，
//! 不复用 `input_key`（它会改缓冲、页码、引号交替等状态）。
//!
//! 只覆盖「会不会消费」；怎么消费仍由 `input_key` 唯一决定。两处若漂移，症状是
//! 某个键被吞（答 TRUE 后 Unhandled）或漏进应用（答 FALSE 后本可处理）—— 前者更
//! 常见，故判定偏保守：拿不准就答 FALSE，让应用先拿到键。

use engine_core::composer::Mode;

use crate::logic::{
    KEY_BACK_SPACE, KEY_DELETE, KEY_ESCAPE, KEY_PAGE_DOWN, KEY_PAGE_UP, KEY_RETURN, KEY_SHIFT,
    KEY_SPACE, KEY_TAB, PAGE_SIZE, TsfLogic,
};
use crate::vk::NO_KEYVAL;

impl TsfLogic {
    /// `OnTestKeyDown` 是否认领这一键（TRUE = 引擎会在 `OnKeyDown` 里消费它）。
    ///
    /// 特殊键按**当前状态**判：空缓冲的退格/回车、非浏览态的 Esc、无组字的
    /// PageUp/PageDown/Tab 都交给应用 —— 这些键在 `input_key` 里本来就走
    /// `Unhandled`，测试阶段再认领只会把它们弄丢。
    pub fn claim_on_test(&self, keycode: u32) -> bool {
        match keycode {
            NO_KEYVAL | KEY_TAB => false,
            KEY_BACK_SPACE | KEY_DELETE | KEY_RETURN | KEY_PAGE_UP | KEY_PAGE_DOWN => {
                self.has_composing()
            }
            KEY_ESCAPE => self.is_symbol_browsing(),
            KEY_SHIFT | KEY_SPACE => true,
            _ => char::from_u32(keycode).is_some_and(|c| self.would_consume_char(c)),
        }
    }

    /// 可打印键的只读分流（逐条镜像 `input_method.rs` 的 `handle_printable`）。
    fn would_consume_char(&self, c: char) -> bool {
        // `handle_printable` 只接 ASCII；非 ASCII 码点走 `_ => Unhandled`。
        if !c.is_ascii() {
            return false;
        }
        // 符号浏览：1–9 / - / = 归浏览；Esc 在 claim_on_test 单独判；其余键先退出
        // 浏览再走正常路由，所以继续往下判即可。下面的标点预览读的是浏览缓冲
        // （「符号」标题，非空）⇒ `.`/`'` 可能保守答 FALSE —— 只会放给应用，不会吞键。
        if self.is_symbol_browsing() {
            match c {
                '1'..='9' | '-' | '=' => return true,
                _ => {}
            }
        }
        // 组字中 `-`/`=` 翻页（在标点表之前，见 handle_printable）。
        if matches!(self.mode(), Mode::Pinyin | Mode::Traditional)
            && !self.buffer().is_empty()
            && matches!(c, '-' | '=')
        {
            return true;
        }
        // 标点表 / 机械全角：只读预览（不翻引号状态、不改 after_digit）。
        if self.engine.punct_would_apply(c) {
            return true;
        }
        match self.mode() {
            Mode::Pinyin | Mode::Traditional => {
                c.is_ascii_alphabetic() || c == '\'' || self.digit_would_select(c)
            }
            Mode::English => {
                // 携带缓冲（中→英保留的拼音）与普通英文都直传；专业模式按下面的
                // 前缀联想分流。
                if self.engine.buffer_is_carried() || !self.english_pro() {
                    return true;
                }
                if c.is_ascii_alphabetic() {
                    return true;
                }
                if c.is_ascii_digit() && !self.buffer().is_empty() {
                    return self.digit_would_select(c);
                }
                if self.buffer().is_empty() {
                    return true; // 空缓冲的非字母可打印键：直传
                }
                c.is_ascii_punctuation()
            }
            // 全角开 ⇒ 数字也出全角（直传）；关 ⇒ 全部 Unhandled。
            Mode::Number => self.fullwidth() && c.is_ascii_graphic(),
            // 符号模式：数字只用于选词（无候选时 Unhandled），其余可见 ASCII 入引擎。
            Mode::Symbol if c.is_ascii_digit() => self.digit_would_select(c),
            Mode::Symbol => c.is_ascii_graphic(),
        }
    }

    /// 数字键是否会选中本页候选（`digit_select` 的只读版）。
    fn digit_would_select(&self, c: char) -> bool {
        if !self.mode().digit_selects_candidates() || self.buffer().is_empty() {
            return false;
        }
        let Some(index) = c.to_digit(10).and_then(|d| d.checked_sub(1)) else {
            return false;
        };
        let page_len = self
            .fetched()
            .len()
            .saturating_sub(self.page * PAGE_SIZE)
            .min(PAGE_SIZE);
        (index as usize) < page_len
    }
}

#[cfg(test)]
#[path = "logic_claim_tests.rs"]
mod tests;
