// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 提交后的智能联想（2026-10-27，Spec §4.1–4.2）。
//!
//! 提交一个至少 2 个汉字的词后，预计算两条来源的续接项并存下来：
//! - 前缀类：词排序索引里以根开头的词，去掉根剩下的部分（最多 3 + 3 条）；
//! - 非前缀类：语义小表里该根的常用后缀（最多 2 条）。
//!
//! 生成只在提交时发生一次；显示路径（[`Engine::candidates`]）只克隆/截断。
//! 状态只活在**拼音/繁体 + 缓冲空 + 无逐字片段**，其余入口清空（Task 6 矩阵）。

use crate::candidates::{Candidate, CandidateKind};
use crate::composer::Mode;
use crate::engine::Engine;
use std::collections::BTreeMap;

/// 提交后联想：根 = 文档末尾的连续片段，items 已按显示顺序排好（最多 8 条）。
pub(crate) struct Association {
    /// Task 7 的链式根要读它；本任务的生产路径尚无读点（只有内联测试读）。
    #[allow(dead_code)]
    pub(crate) root: String,
    pub(crate) items: Vec<Candidate>,
}

impl Engine {
    /// 联想态：拼音/繁体、缓冲空、无逐字片段，且存在联想项。
    pub fn association_active(&self) -> bool {
        matches!(
            self.composer.session().mode,
            Mode::Pinyin | Mode::Traditional
        ) && self.composer.session().buffer.is_empty()
            && self.pending_text().is_empty()
            && self.association.is_some()
    }

    /// 收起联想；返回清之前是否存在（宿主据此决定要不要重推候选窗）。
    pub fn dismiss_association(&mut self) -> bool {
        self.association.take().is_some()
    }

    /// 按提交文本重建联想（Spec §4.2）。第一行清旧联想：任何前置失败都不得把
    /// 上一次的联想留在屏幕上。
    pub(crate) fn build_association(&mut self, root: &str) {
        self.association = None;
        let root = last_16_chars(root);
        if !matches!(
            self.composer.session().mode,
            Mode::Pinyin | Mode::Traditional
        ) || !self.composer.session().buffer.is_empty()
            || !self.pending_text().is_empty()
            || cjk_count(&root) < 2
        {
            return;
        }
        let root_len = root.chars().count();
        let dict = self.active_dict();

        // 前缀类：续接 = 去掉根之后的部分；同键（同词多读音）取最大权重。
        let mut conts: BTreeMap<String, u32> = BTreeMap::new();
        for (word, weight) in dict.words_with_prefix(&root) {
            if word == root {
                continue;
            }
            let cont: String = word.chars().skip(root_len).collect();
            if cont.is_empty() {
                continue;
            }
            let slot = conts.entry(cont).or_insert(0);
            *slot = (*slot).max(weight);
        }
        let mut prefix_sorted: Vec<(String, u32)> = conts.into_iter().collect();
        prefix_sorted.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.as_bytes().cmp(b.0.as_bytes())));

        let a: Vec<(String, u32)> = prefix_sorted.iter().take(3).cloned().collect();
        let b: Vec<(String, u32)> = prefix_sorted
            .iter()
            .skip(3)
            .filter(|(cont, _)| (2..=6).contains(&cont.chars().count()))
            .take(3)
            .cloned()
            .collect();
        let selected: Vec<String> = a.iter().chain(b.iter()).map(|(t, _)| t.clone()).collect();

        // 非前缀类：assoc 小表已按 (rank desc, len asc, bytes asc) 排序。
        let mut c: Vec<(String, u32)> = Vec::new();
        for (suffix, rank) in dict.association_suffixes(&root) {
            if c.len() == 2 {
                break;
            }
            if selected.iter().any(|t| t == &suffix)
                || selected
                    .iter()
                    .any(|t| t.starts_with(suffix.as_str()) || suffix.starts_with(t.as_str()))
            {
                continue;
            }
            c.push((suffix, rank));
        }

        let items: Vec<Candidate> = a
            .into_iter()
            .chain(b)
            .chain(c)
            .map(|(text, score)| Candidate {
                text,
                kind: CandidateKind::Hanzi,
                score: score as u64,
            })
            .collect();
        if items.is_empty() {
            return;
        }
        self.association = Some(Association { root, items });
    }
}

/// 根超过 16 字时截**最后** 16 字（仍与文档末尾连续）。
fn last_16_chars(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    chars[chars.len().saturating_sub(16)..].iter().collect()
}

/// 汉字判定：CJK 统一表意文字基本区（扩展区留待以后）。
fn cjk_count(s: &str) -> usize {
    s.chars()
        .filter(|c| ('\u{4E00}'..='\u{9FFF}').contains(c))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::{Dictionary, InMemoryDictionary};
    use crate::symbols::SymbolEngine;
    use crate::trie::Entry;

    #[test]
    fn root_is_capped_to_last_16_chars() {
        let last16 = "五六七八九十甲乙丙丁戊己庚辛壬癸";
        struct TruncDict(String);
        impl Dictionary for TruncDict {
            fn query(&self, _pinyin: &str, _limit: usize) -> Vec<Entry> {
                Vec::new()
            }
            fn words_with_prefix(&self, prefix: &str) -> Vec<(String, u32)> {
                (prefix == self.0)
                    .then(|| vec![(format!("{prefix}好"), 1)])
                    .unwrap_or_default()
            }
            fn len(&self) -> usize {
                0
            }
        }
        let mut e = Engine::new(
            Box::new(TruncDict(last16.to_string())),
            SymbolEngine::new(Vec::new(), Vec::new()),
            false,
        );
        e.build_association("一二三四五六七八九十甲乙丙丁戊己庚辛壬癸");
        let a = e.association.as_ref().expect("16 字根有命中，应建联想");
        assert_eq!(a.root, last16);
        assert_eq!(a.items[0].text, "好");
    }

    #[test]
    fn non_chinese_modes_do_not_build_association() {
        let mut e = Engine::new(
            Box::new(InMemoryDictionary::new()),
            SymbolEngine::new(Vec::new(), Vec::new()),
            false,
        );
        for mode in [Mode::English, Mode::Number, Mode::Symbol] {
            e.switch_mode(mode);
            e.build_association("你好");
            assert!(!e.association_active(), "{mode:?} 不得建联想");
        }
    }
}
