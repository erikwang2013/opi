// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! engine-core：OPI 输入法纯逻辑内核（M1 全拼）。
//! 模块文件在 Task 1 以空占位创建，各任务逐个填充；re-export 随类型就位时添加。

pub mod bytes;
pub mod candidates;
pub mod composer;
pub mod dictionary;
pub mod engine;
#[path = "engine_punct.rs"]
mod engine_punct_impl; // 标点/全角一族（500 行门禁拆出，见文件头）
#[path = "engine_readings.rs"]
mod engine_readings_impl; // 9 键读数一族（500 行门禁拆出，见文件头）
pub mod fuzzy;
pub mod jianpin;
pub mod keys;
pub mod learner;
pub mod pinyin;
pub mod punctuation;
pub mod router;
pub mod scheme;
pub mod symbols;
pub mod trie;

pub use composer::{Composer, KeyEffect, Mode, Session};
pub use dictionary::{Dictionary, InMemoryDictionary};
pub use engine::Engine;
pub use learner::{Learner, UserWord, UserWordExport};
pub use router::{KeyAction, KeyRouter, ShiftState};
pub use scheme::Scheme;
pub use trie::Entry;
