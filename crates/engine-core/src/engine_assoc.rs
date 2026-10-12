// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **联想一族**（2026-10-12 从 `engine.rs` 拆出：那边加联想上下文后越 500 行门禁；
//! 与 engine_punct.rs / engine_readings.rs / engine_segment.rs 同一套先例）。
//!
//! 语义与清空时机全在 `Engine::assoc_pinyin` 的字段注上；这里只有**记录**那一半
//! （提交候选时把「上文拼音」写下）。拆分代价：字段升 `pub(crate)`（crate 内，不泄漏）。

use crate::composer::Mode;
use crate::engine::Engine;
use crate::scheme::Scheme;

impl Engine {
    /// 记联想上文（见 [`Self::assoc_pinyin`] 的字段注）：第 `index` 条候选被提交时调用。
    ///
    /// - 9/14 键：取**逐条读数**（缓冲是数字串，读数才准）；读数缓存必须与当前缓冲
    ///   配对（不配对说明它是上一批候选的读数 —— 宁可清空也不记错的）。
    /// - 26 键：缓冲本身（精确命中时逐字正确；前缀扩展/回退档只是它的前缀 —— 已知近似）。
    /// - 非拼音/繁体模式（英文/数字/符号）：清空。
    pub(crate) fn note_assoc_on_select(&mut self, index: usize, text: &str) {
        if !matches!(
            self.composer.session().mode,
            Mode::Pinyin | Mode::Traditional
        ) {
            self.assoc_pinyin.clear();
            return;
        }
        let buf = self.composer.session().buffer.clone();
        self.assoc_pinyin = match self.scheme() {
            Scheme::Nine | Scheme::Fourteen => {
                let cached = self.scheme_readings.borrow();
                if cached.0 == buf {
                    cached
                        .1
                        .get(index)
                        .map(|r| r.replace('\'', ""))
                        .unwrap_or_default()
                } else {
                    String::new()
                }
            }
            // 26 键：缓冲 ≈ 词的拼音 —— 但**从联想档选中的**词，缓冲只是尾巴
            // （上文「你好」+ 输入 ma 选「你好吗」，缓冲是 `ma`，键却是 `nihaoma`）。
            // 那一档的拼接串是**现成**的：查出选中的词在不在 `上文+缓冲` 的整串命中里，
            // 在 ⇒ 上文换成拼接串（精确）；不在 ⇒ 用缓冲（前缀扩展档的近似，见字段注）。
            _ => {
                if self.assoc_pinyin.is_empty() {
                    buf
                } else {
                    let joined = format!("{}{}", self.assoc_pinyin, buf);
                    let from_assoc = self
                        .active_dict()
                        .query_prefix_top(&joined, crate::dictionary::PREFIX_MATCH_CAP)
                        .iter()
                        .any(|e| e.word == text);
                    if from_assoc { joined } else { buf }
                }
            }
        };
    }
}
