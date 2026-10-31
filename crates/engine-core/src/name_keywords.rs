// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 输入符号/标点/颜文字的**名称**（2026-10-31 用户需求）：中文模式打拼音名、
//! 英文模式打英文名，把对应文本并入候选。数据是 `data/raw/name_keywords.tsv`
//! （`scripts/gen_name_keywords.py` 生成；颜文字来自 `kaomoji.tsv`），编译期嵌入、
//! 运行期零 IO；索引首次查询时建一次。
//!
//! 与两条既有路的边界：
//! * `symbols.rs` 的符号表管**符号面板搜索**与符号模式；本表补它没有的常用标点
//!   （全角括号/逗号/引号等）与颜文字。
//! * `emoji_keywords.rs` 管表情；本表不重复表情条目。
//!
//! 位置与 limit 同 emoji 层：只追加在文本候选之后，limit 满了就一个也不加 ——
//! 绝不顶掉 `nihao` / `hello` 这类文本首候选。匹配是**关键字前缀**（`zuokuo` 命中
//! `zuokuohao`），大小写不敏感，缓冲里的 `'` 分词符忽略；单键不触发。

use crate::candidates::{Candidate, CandidateKind};
use crate::composer::Mode;
use std::collections::HashSet;
use std::sync::OnceLock;

const TABLE: &str = include_str!("../../../data/raw/name_keywords.tsv");

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lang {
    Zh,
    En,
    Both,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Punct,
    Symbol,
    Kaomoji,
}

struct Entry {
    text: &'static str,
    keywords: Vec<&'static str>,
    lang: Lang,
    kind: Kind,
}

fn entries() -> &'static [Entry] {
    static ENTRIES: OnceLock<Vec<Entry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        TABLE
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .filter_map(|line| {
                let mut cols = line.split('\t');
                let text = cols.next()?;
                let _name = cols.next()?;
                let keywords = cols.next()?;
                let kind = match cols.next()? {
                    "punct" => Kind::Punct,
                    "symbol" => Kind::Symbol,
                    "kaomoji" => Kind::Kaomoji,
                    // 数据由生成器校验；坏行直接跳过，不让一行坏数据拖垮输入。
                    _ => return None,
                };
                let lang = match cols.next()? {
                    "zh" => Lang::Zh,
                    "en" => Lang::En,
                    "both" => Lang::Both,
                    _ => return None,
                };
                Some(Entry {
                    text,
                    keywords: keywords.split(',').filter(|k| !k.is_empty()).collect(),
                    lang,
                    kind,
                })
            })
            .collect()
    })
}

fn allowed(lang: Lang, mode: Mode) -> bool {
    match mode {
        Mode::Pinyin | Mode::Traditional => matches!(lang, Lang::Zh | Lang::Both),
        Mode::English => matches!(lang, Lang::En | Lang::Both),
        Mode::Number | Mode::Symbol => false,
    }
}

/// 缓冲命中的全部条目（未截断；调用方负责 limit 与去重）。
fn candidates(buffer: &str, mode: Mode) -> Vec<Candidate> {
    let prefix = buffer.to_ascii_lowercase().replace('\'', "");
    if prefix.chars().count() < 2 || !prefix.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Vec::new();
    }
    // (精确?0:1, 关键字长度, 关键字序号, 条目序)
    let mut hits: Vec<(u8, usize, usize, usize)> = Vec::new();
    for (ei, entry) in entries().iter().enumerate() {
        if !allowed(entry.lang, mode) {
            continue;
        }
        let mut best: Option<(u8, usize, usize)> = None;
        for (ki, kw) in entry.keywords.iter().enumerate() {
            if let Some(rest) = kw.strip_prefix(prefix.as_str()) {
                let rank = (u8::from(!rest.is_empty()), kw.len(), ki);
                if best.is_none_or(|b| rank < b) {
                    best = Some(rank);
                }
            }
        }
        if let Some((exact, klen, ki)) = best {
            hits.push((exact, klen, ki, ei));
        }
    }
    hits.sort_unstable();
    // 颜文字一搜一大片（同一分类几十条），候选栏放不下；每类最多出 8 条。
    let mut kaomoji = 0usize;
    let mut out: Vec<Candidate> = Vec::new();
    for (_, _, _, ei) in hits {
        let entry = &entries()[ei];
        if entry.kind == Kind::Kaomoji {
            if kaomoji >= 8 {
                continue;
            }
            kaomoji += 1;
        }
        out.push(Candidate {
            text: entry.text.to_string(),
            kind: CandidateKind::Symbol,
            score: 0,
        });
    }
    out
}

/// 在已有候选**之后**追加命中的标点/符号/颜文字，受 `limit` 约束。
pub(crate) fn append_to(out: &mut Vec<Candidate>, buffer: &str, limit: usize, mode: Mode) {
    if out.len() >= limit {
        return;
    }
    let mut seen: HashSet<String> = out.iter().map(|c| c.text.clone()).collect();
    for c in candidates(buffer, mode) {
        if out.len() >= limit {
            break;
        }
        if seen.insert(c.text.clone()) {
            out.push(c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(buffer: &str, mode: Mode) -> Vec<String> {
        candidates(buffer, mode)
            .into_iter()
            .map(|c| c.text)
            .collect()
    }

    #[test]
    fn pinyin_names_match_punctuation_and_symbols() {
        assert!(texts("zuokuohao", Mode::Pinyin).contains(&"（".to_string()));
        assert!(texts("jiantou", Mode::Pinyin).contains(&"→".to_string()));
        assert!(texts("aixin", Mode::Pinyin).contains(&"♥".to_string()));
    }

    #[test]
    fn english_names_match_ascii_punctuation() {
        assert!(texts("leftbracket", Mode::English).contains(&"[".to_string()));
        assert!(texts("arrow", Mode::English).contains(&"→".to_string()));
        assert!(!texts("leftbracket", Mode::English).contains(&"【".to_string()));
    }

    #[test]
    fn chinese_names_do_not_leak_into_english_mode() {
        assert!(texts("zuokuohao", Mode::English).is_empty());
        assert!(texts("comma", Mode::Pinyin).contains(&"，".to_string()));
    }

    #[test]
    fn kaomoji_group_matches_but_is_capped() {
        let got = texts("kaixin", Mode::Pinyin);
        assert!(!got.is_empty());
        assert!(got.iter().any(|t| t.contains("∀") || t.contains("✿")));
        assert!(got.len() <= 8);
    }

    #[test]
    fn single_key_and_non_alnum_do_not_trigger() {
        assert!(texts("z", Mode::Pinyin).is_empty());
        assert!(texts("zuo'kuo", Mode::Pinyin).contains(&"（".to_string()));
        assert!(texts("左", Mode::Pinyin).is_empty());
    }
}
