// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 集成测试的共享夹具（2026-10-14，M4 3b：`wubi.rs` 加完「自动上屏开关 + 拼音进入
//! 方式」两条门禁后顶破 500 行，把夹具搬出来而不是删注释 —— 先例
//! `crates/opi-ffi/tests/common/mod.rs`，同一套 `mod common;` 写法）。
//!
//! `tests/common/mod.rs` 不会被当独立测试目标编译（目录名不是 `*.rs`），故这里没有
//! `#[test]`，且**每个夹具都要 `pub`**（否则用它的测试文件看不见）。现在只有
//! `tests/wubi.rs` 用；真出现第二个用它的文件时，不用再复制夹具 —— 这层共享正是
//! 搬出来的第二个理由。

// 每个测试目标各自编译本模块，只用其中一部分夹具；未用到的不是死代码。
#![allow(dead_code)]

use engine_core::dictionary::{Dictionary, InMemoryDictionary};
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Entry, Scheme, WubiStyle};

/// 符号表用**空表**：五笔门禁测的是码表装配，符号数据不该参与断言
/// （唯一的例外是判别「符号层前缀命中也要挡自动上屏」那条，它自建符号表）。
pub fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

/// 五笔码表夹具。每条都挂着一个判据（别随手改频点）：
/// - `a` 同码多词（工/戈）—— 一级简码的常见形态；
/// - `abc` 同码多词、`abcd` 唯一 —— 四码自动上屏的两面；
/// - `aaaa` 四码同码多词 —— 「同码多词就不自动」的用例；
/// - `efgh` 精确 + `efghi` 延伸 —— 「有更长的码 ⇒ 整档不止一条 ⇒ 不自动」；
/// - `mnopq` 只有延伸码（`mnop` 无精确条目）—— 「唯一命中但是延伸 ⇒ 不自动」；
/// - `ni` 与拼音夹具的 `ni` 同串 —— 混输「同一层合并」的用例。
pub fn wubi_dict() -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    for (code, word, freq) in [
        ("a", "工", 900u32),
        ("a", "戈", 800),
        ("ab", "式", 700),
        ("abc", "节", 600),
        ("abc", "世", 550),
        ("abcd", "范", 500),
        ("aaaa", "恭", 900),
        ("aaaa", "靴", 800),
        ("efgh", "森", 400),
        ("efghi", "麓", 300),
        ("mnopq", "奕", 200),
        ("ni", "尼", 700),
    ] {
        d.insert(code, word, freq);
    }
    d
}

/// 拼音主库夹具（混输的拼音侧；也是「空槽落普通路径」用例的主库）。
pub fn pinyin_dict() -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    for (py, w, f) in [
        ("ni", "你", 5000u32),
        ("nihao", "你好", 6000),
        ("hao", "好", 7000),
    ] {
        d.insert(py, w, f);
    }
    d
}

/// 纯五笔引擎（默认风格 = Pure，默认进入方式 = DirectLetters）。
pub fn engine() -> Engine {
    let mut e = Engine::new(Box::new(pinyin_dict()), no_symbols(), false);
    e.set_wubi_dict(Some(Box::new(wubi_dict())));
    e.set_scheme(Scheme::Wubi);
    e
}

/// 混输引擎（同一对词典，只切风格）。
pub fn mixed_engine() -> Engine {
    let mut e = engine();
    e.set_wubi_style(WubiStyle::Mixed);
    e
}

pub fn type_str(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

/// 只取文本，丢分数/类别（门禁断言的都是用户看得见的东西）。
pub fn texts(e: &Engine) -> Vec<String> {
    e.candidates(9).into_iter().map(|c| c.text).collect()
}

/// 联想门禁的替身词库。`prefixes` / `suffixes` 按 root **精确查表**（模拟
/// 词排序索引 / 语义小表，不做字符串扫描）——「新根无命中」的负例因此可控。
#[derive(Default)]
pub struct AssocDict {
    /// (pinyin, word, freq)
    pub entries: Vec<(String, String, u32)>,
    /// (root, [(word, weight)])
    pub prefixes: Vec<(String, Vec<(String, u32)>)>,
    /// (root, [(suffix, rank)])
    pub suffixes: Vec<(String, Vec<(String, u32)>)>,
}

impl AssocDict {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn word(mut self, pinyin: &str, word: &str, freq: u32) -> Self {
        self.entries.push((pinyin.into(), word.into(), freq));
        self
    }

    pub fn prefix(mut self, root: &str, words: &[(&str, u32)]) -> Self {
        self.prefixes.push((
            root.into(),
            words.iter().map(|(w, f)| ((*w).into(), *f)).collect(),
        ));
        self
    }

    pub fn suffix(mut self, root: &str, suffixes: &[(&str, u32)]) -> Self {
        self.suffixes.push((
            root.into(),
            suffixes.iter().map(|(s, r)| ((*s).into(), *r)).collect(),
        ));
        self
    }
}

impl Dictionary for AssocDict {
    fn query(&self, pinyin: &str, limit: usize) -> Vec<Entry> {
        let mut out: Vec<Entry> = self
            .entries
            .iter()
            .filter(|(py, _, _)| py.starts_with(pinyin))
            .map(|(py, w, f)| Entry {
                word: w.clone(),
                freq: *f,
                pinyin_len: py.len(),
            })
            .collect();
        out.sort_by(|a, b| b.freq.cmp(&a.freq).then(a.word.cmp(&b.word)));
        out.truncate(limit);
        out
    }

    fn visit_entries(&self, f: &mut dyn FnMut(&str, &str, u32)) {
        for (py, w, fr) in &self.entries {
            f(py, w, *fr);
        }
    }

    fn words_with_prefix(&self, prefix: &str) -> Vec<(String, u32)> {
        self.prefixes
            .iter()
            .find(|(r, _)| r == prefix)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    }

    fn association_suffixes(&self, root: &str) -> Vec<(String, u32)> {
        self.suffixes
            .iter()
            .find(|(r, _)| r == root)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn max_freq(&self) -> u64 {
        self.entries.iter().map(|e| e.2 as u64).max().unwrap_or(0)
    }
}
