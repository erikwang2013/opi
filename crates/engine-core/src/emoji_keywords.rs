// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 输入即得表情（2026-10-18 用户需求「增加输入可匹配表情」）：拼音/英文缓冲是某个
//! 关键字的前缀时，把对应 emoji 并入候选。
//!
//! 数据是入库产物 `data/raw/emoji_keywords.tsv`（`emoji\t关键字,逗号分隔`，
//! `scripts/gen_emoji_keywords.py` 生成；来源/许可见该脚本与 `data/raw/LICENSES.md`），
//! `include_str!` 编译期嵌入、运行期零 IO；索引首次查询时建一次。
//!
//! 与 `symbols.rs` 的 emoji 搜索**是两条路**：那边是符号面板的关键字表（1F600–1F64F
//! 的 80 条 + Unicode 名兜底），只为符号模式/单键反馈服务；本表是**输入匹配**的
//! 常用表情子集（条数以数据文件为准），带拼音别名，并且只在文本候选之后追加。两条路会命中同一
//! 个 emoji，去重按「去掉 VS16 后的文本」做（`♥` 与 `♥️` 是同一个表情）。
//!
//! ## 排序与位置（为什么不并进 `rank_and_pick`）
//!
//! 候选排序是全仓敏感面（三档、精确加成、学习权重都在 `candidates.rs`）。本层只做
//! **受控追加**：调用方拿到文本候选后追加，`limit` 已满就一个也不加 ⇒ 任何文本候选
//! 都不可能被 emoji 顶掉（`nihao` / `hello` 的首位门禁见 tests/emoji_match.rs）。
//! 同组内排序键 = (是否精确匹配, 关键字长度, 关键字序号, 文件序)：精确 > 前缀，
//! 前缀里覆盖得更完整的（关键字更短）优先，其余按表内顺序（= 面板常用序）。
//!
//! 单键（缓冲 < 2 字符）不触发：单字母会命中一大片 emoji，是噪声；单键反馈仍由
//! `symbols.search_emoji` 负责（既有行为，未改）。

use crate::candidates::{Candidate, CandidateKind};
use std::collections::HashSet;
use std::sync::OnceLock;

/// `emoji\tkeyword1,keyword2,...`（生成器写的，勿手改）。
const TABLE: &str = include_str!("../../../data/raw/emoji_keywords.tsv");

struct Entry {
    text: &'static str,
    keywords: Vec<&'static str>,
}

fn entries() -> &'static [Entry] {
    static ENTRIES: OnceLock<Vec<Entry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        TABLE
            .lines()
            .filter_map(|line| {
                let (text, keywords) = line.split_once('\t')?;
                Some(Entry {
                    text,
                    keywords: keywords.split(',').filter(|k| !k.is_empty()).collect(),
                })
            })
            .collect()
    })
}

/// 缓冲命中的全部 emoji（未截断；同一 emoji 至多一条）。调用方负责 limit 与去重。
fn candidates(buffer: &str) -> Vec<Candidate> {
    // 拼音的分词符不是关键字的一部分：`wei'xiao` 与 `weixiao` 同义。
    let prefix = buffer.to_ascii_lowercase().replace('\'', "");
    // 单键不触发；9/14 键的数字缓冲与其它非字母串也不猜（边界见模块头）。
    if prefix.chars().count() < 2 || !prefix.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Vec::new();
    }
    // (精确?0:1, 关键字长度, 关键字序号, 条目序)
    let mut hits: Vec<(u8, usize, usize, usize)> = Vec::new();
    for (ei, entry) in entries().iter().enumerate() {
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
    hits.into_iter()
        .map(|(_, _, _, ei)| Candidate {
            text: entries()[ei].text.to_string(),
            kind: CandidateKind::Emoji,
            score: 0, // 与符号候选同一条约定：没有词频，靠追加位置排序
        })
        .collect()
}

/// emoji 的跨源去重键：`♥` 与 `♥️` 是同一个表情，VS16 不参与比较。
fn norm(text: &str) -> String {
    text.chars().filter(|&c| c != '\u{FE0F}').collect()
}

/// 在已有候选**之后**追加命中的 emoji，受 `limit` 约束（满了就不加，绝不顶掉文本）。
pub(crate) fn append_to(out: &mut Vec<Candidate>, buffer: &str, limit: usize) {
    if out.len() >= limit {
        return;
    }
    let mut seen: HashSet<String> = out
        .iter()
        .filter(|c| matches!(c.kind, CandidateKind::Emoji | CandidateKind::Symbol))
        .map(|c| norm(&c.text))
        .collect();
    for c in candidates(buffer) {
        if out.len() >= limit {
            break;
        }
        if seen.insert(norm(&c.text)) {
            out.push(c);
        }
    }
}
