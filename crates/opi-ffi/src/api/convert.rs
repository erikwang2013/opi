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
