// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 两个 ABI 共用的类型映射与文本/JSON 出口：engine_core 类型 → FFI 面类型。

use engine_core::candidates::{Candidate, CandidateKind};
use engine_core::composer::Mode;
use engine_core::symbols::{Block, SymbolEntry};

use super::Api;

/// JNI/C 共用的 0..=4 模式整数 ↔ Mode 转换（0=Pinyin 1=English 2=Number 3=Symbol 4=Traditional）。
pub fn mode_from_int(m: i32) -> Option<Mode> {
    match m {
        0 => Some(Mode::Pinyin),
        1 => Some(Mode::English),
        2 => Some(Mode::Number),
        3 => Some(Mode::Symbol),
        4 => Some(Mode::Traditional),
        _ => None,
    }
}

pub fn mode_to_int(m: Mode) -> i32 {
    match m {
        Mode::Pinyin => 0,
        Mode::English => 1,
        Mode::Number => 2,
        Mode::Symbol => 3,
        Mode::Traditional => 4,
    }
}

/// 候选文本列表（JNI/C 出口共用；kind/score UI 不用，仅文本）。
pub fn candidate_texts(api: &Api, limit: usize) -> Vec<String> {
    api.candidates(limit).into_iter().map(|c| c.text).collect()
}

/// 符号块内符号文本列表。
pub fn symbol_texts(api: &Api, id: u16) -> Vec<String> {
    api.symbols_in_block(id)
        .into_iter()
        .map(|s| s.text)
        .collect()
}

/// 符号搜索命中文本列表。
pub fn search_symbol_texts(api: &Api, keyword: &str) -> Vec<String> {
    api.search_symbols(keyword.to_string())
        .into_iter()
        .map(|s| s.text)
        .collect()
}

/// 表情面板「表情」页的**浏览**表：`data/raw/emoji.tsv`（OpenMoji 常用子集，
/// 689 条，9 组，含植物；排除旗帜/肤色变体/ZWJ 组合），**按面板分类序**。
///
/// 上一版取的是 UTS#51 `emoji-test.txt` 的完全限定全量（3963 条）。2026-10-18
/// 起改为 OpenMoji 常用子集 —— 浏览页要常用而非全量，筛选规则与源 pin 见
/// `scripts/gen_emoji_kaomoji.py` 头注。
/// ⚠️ symbols.tsv 的 emoji 列**不动**：单键候选反馈（`search_emoji`）仍走它。
///
/// 与 [`crate::api::Api`] 无关（纯静态数据）—— 参数去掉后调用点是 jni.rs 一处。
pub fn emoji_symbol_texts() -> Vec<String> {
    engine_core::symbols::SymbolEngine::builtin_emoji_texts()
        .into_iter()
        .map(str::to_owned)
        .collect()
}

/// emojiGroups JSON：`[{"count","name","start"}]`（**serde 字典序** —— 未开
/// preserve_order，消费方别按书写顺序写正则）—— 表情面板**分类条**用（2026-10-12
/// 用户裁决「中间是表情分类」；形状照 [`symbol_blocks_json`]）。
/// `start` 与 [`emoji_symbol_texts`] 的下标对齐：点分类 = 网格滚到 `start`。
/// 与 `Api` 无关（纯静态数据），同 `emoji_symbol_texts`。
pub fn emoji_groups_json() -> String {
    let arr: Vec<serde_json::Value> =
        engine_core::symbols::SymbolEngine::builtin_emoji_groups()
            .into_iter()
            .map(|(name, start, count)| {
                serde_json::json!({ "name": name, "start": start, "count": count })
            })
            .collect();
    serde_json::to_string(&arr).unwrap_or_default()
}

/// 文本列表 → JSON 数组字符串。
pub fn texts_json(texts: &[String]) -> String {
    serde_json::to_string(texts).unwrap_or_default()
}

/// symbolBlocks JSON：`[{id,start,end,name,common}]`。
pub fn symbol_blocks_json(api: &Api) -> String {
    let blocks = api.symbol_blocks();
    let arr: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            serde_json::json!({
                "id": b.id,
                "start": b.start,
                "end": b.end,
                "name": b.name,
                "common": b.common,
            })
        })
        .collect();
    serde_json::to_string(&arr).unwrap_or_default()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiMode {
    Pinyin,
    Traditional,
    English,
    Number,
    Symbol,
}

impl From<Mode> for ApiMode {
    fn from(m: Mode) -> Self {
        match m {
            Mode::Pinyin => ApiMode::Pinyin,
            Mode::Traditional => ApiMode::Traditional,
            Mode::English => ApiMode::English,
            Mode::Number => ApiMode::Number,
            Mode::Symbol => ApiMode::Symbol,
        }
    }
}

impl From<ApiMode> for Mode {
    fn from(m: ApiMode) -> Self {
        match m {
            ApiMode::Pinyin => Mode::Pinyin,
            ApiMode::Traditional => Mode::Traditional,
            ApiMode::English => Mode::English,
            ApiMode::Number => Mode::Number,
            ApiMode::Symbol => Mode::Symbol,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiCandidateKind {
    Hanzi,
    English,
    Emoji,
    Symbol,
}

impl From<CandidateKind> for ApiCandidateKind {
    fn from(k: CandidateKind) -> Self {
        match k {
            CandidateKind::Hanzi => ApiCandidateKind::Hanzi,
            CandidateKind::English => ApiCandidateKind::English,
            CandidateKind::Emoji => ApiCandidateKind::Emoji,
            CandidateKind::Symbol => ApiCandidateKind::Symbol,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiCandidate {
    pub text: String,
    pub kind: ApiCandidateKind,
    pub score: u64,
}

impl From<Candidate> for ApiCandidate {
    fn from(c: Candidate) -> Self {
        ApiCandidate {
            text: c.text,
            kind: c.kind.into(),
            score: c.score,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiBlock {
    pub id: u16,
    pub start: u32,
    pub end: u32,
    pub name: String,
    pub common: bool,
}

impl From<Block> for ApiBlock {
    fn from(b: Block) -> Self {
        ApiBlock {
            id: b.id.0,
            start: b.start,
            end: b.end,
            name: b.name,
            common: b.common,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiSymbolEntry {
    pub text: String,
    pub name: String,
    pub keywords: Vec<String>,
    pub block: u16,
    pub emoji: bool,
}

impl From<SymbolEntry> for ApiSymbolEntry {
    fn from(s: SymbolEntry) -> Self {
        ApiSymbolEntry {
            text: s.text,
            name: s.name,
            keywords: s.keywords,
            block: s.block.0,
            emoji: s.emoji,
        }
    }
}
