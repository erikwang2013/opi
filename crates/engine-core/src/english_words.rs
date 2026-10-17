// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 英文单词联想（用户裁决 2026-10-18「把词典整合成词库，在英文模式下调用匹配」）。
//!
//! **数据不是牛津词典**：`data/raw/en_words.tsv` 是 Google Books Ngrams v3 英文
//! 1-gram 的前 10000 词（orgtre/google-books-ngram-frequency，**CC-BY 3.0**，pin 与
//! 署名见 `data/raw/LICENSES.md` 与 `scripts/gen_en_dict.py`）。牛津词典（OUP）有版权、
//! 本仓无授权，**不能**抄；换数据只需替换 TSV（本模块不认词源），逻辑不动。
//!
//! 表三列 `词形\t查询键\t词频`（与 `trad_chars.tsv` 同一条「产物已入库、include_str!
//! 嵌入、运行期零 IO」先例）：
//! - **第一列是规范词形**（`China` / `café` / `I` —— 专名与大写词形的 178 条在这里）；
//! - 第二列是小写 ASCII 查询键（`china` / `cafe`）——匹配一律按它；
//! - 第三列是缩放后的词频（生成器保证全表**严格递减**，故按它排序无并列）。
//!
//! 索引在首次使用时解析 + 按查询键排序（`OnceLock`，之后只读）；每次查询 =
//! 二分找前缀区间 + 只对命中项按词频排序，**不重排全表**。

use crate::candidates::{Candidate, CandidateKind};
use std::sync::OnceLock;

/// `词形\t查询键\t词频`（生成器写的，勿手改；许可与来源见 `data/raw/LICENSES.md`）。
const TABLE: &str = include_str!("../../../data/raw/en_words.tsv");

struct Word {
    /// 规范词形（候选上屏的文本）。
    text: &'static str,
    /// 小写 ASCII 查询键（前缀匹配用）。
    key: &'static str,
    freq: u32,
}

fn words() -> &'static [Word] {
    static WORDS: OnceLock<Vec<Word>> = OnceLock::new();
    WORDS.get_or_init(|| {
        let mut v: Vec<Word> = TABLE
            .lines()
            .filter_map(|line| {
                let mut cols = line.split('\t');
                Some(Word {
                    text: cols.next()?,
                    key: cols.next()?,
                    freq: cols.next()?.parse().ok()?,
                })
            })
            .collect();
        // 生成器已按查询键升序；这里再排一次，改生成器也不会让二分查失效（同 trad_chars）。
        v.sort_unstable_by(|a, b| a.key.cmp(b.key));
        v
    })
}

/// 以 `buffer` 为前缀的词，按词频降序，最多 `limit` 条。
///
/// 大小写不敏感（缓冲降小写后比查询键）；显示用规范词形，但**缓冲首字母大写**时
/// 规范小写词形跟着首字母大写（用户按 ⇧ 的语义不能丢）。空缓冲 → 无候选。
pub(crate) fn candidates(buffer: &str, limit: usize) -> Vec<Candidate> {
    if buffer.is_empty() || limit == 0 {
        return Vec::new();
    }
    let prefix = buffer.to_ascii_lowercase();
    let all = words();
    let start = all.partition_point(|w| w.key < prefix.as_str());
    let mut hits: Vec<&Word> = all[start..]
        .iter()
        .take_while(|w| w.key.starts_with(&prefix))
        .collect();
    hits.sort_unstable_by_key(|a| std::cmp::Reverse(a.freq));
    hits.truncate(limit);
    let capitalize = buffer
        .as_bytes()
        .first()
        .is_some_and(|b| b.is_ascii_uppercase());
    hits.into_iter()
        .map(|w| {
            let mut text = w.text.to_string();
            // 只对**小写**规范词形做首字母大写：专名（China/I）本来就是规范写法。
            if capitalize
                && text
                    .as_bytes()
                    .first()
                    .is_some_and(|b| b.is_ascii_lowercase())
            {
                text[..1].make_ascii_uppercase();
            }
            Candidate {
                text,
                kind: CandidateKind::English,
                score: w.freq as u64,
            }
        })
        .collect()
}
