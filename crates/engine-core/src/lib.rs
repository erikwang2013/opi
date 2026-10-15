// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! engine-core：OPI 输入法纯逻辑内核（M1 全拼）。
//! 模块文件在 Task 1 以空占位创建，各任务逐个填充；re-export 随类型就位时添加。

pub mod bytes;
pub mod candidates;
#[path = "candidates_options.rs"]
mod candidates_options_impl; // 排序层开关 RankOptions（500 行门禁拆出，见文件头）
#[path = "candidates_scheme.rs"]
mod candidates_scheme_impl;
#[path = "candidates_segment.rs"]
mod candidates_segment_impl; // 分词回退一族（2026-10-14 从 candidates.rs 拆出，见文件头）
#[path = "candidates_tiers.rs"]
mod candidates_tiers_impl; // 候选三档（常用单字/词组/联想，2026-10-14 用户裁决；500 行门禁拆出）
pub mod composer;
pub mod correction;
mod dict_layers; // 导入词库的合并层（M3 2b）：Engine 主库槽位的具体类型，字段私有故模块不公开
pub mod dictionary;
pub mod engine;
#[path = "engine_candidates.rs"]
mod engine_candidates_impl; // 主候选装配一族（2026-10-16 从 engine.rs 拆出，见文件头）
#[path = "engine_dicts.rs"]
mod engine_dicts_impl; // 词典换装一族（trad/stroke/导入层）+ user_boost 重算（500 行门禁拆出）
#[path = "engine_enter.rs"]
mod engine_enter_impl; // 回车一族：缓冲非空时该做什么的单一判据（2026-10-14 接线）
#[path = "engine_punct.rs"]
mod engine_punct_impl; // 标点/全角一族（500 行门禁拆出，见文件头）
#[path = "engine_readings.rs"]
mod engine_readings_impl; // 9 键读数一族（500 行门禁拆出，见文件头）
#[path = "engine_segment.rs"]
mod engine_segment_impl; // 分词回退的收尾（500 行门禁拆出，见文件头）
#[path = "engine_shuangpin.rs"]
mod engine_shuangpin_impl; // 双拼一族：布局设置 + 候选分支（500 行门禁拆出，见文件头）
#[path = "engine_symbol_browse.rs"]
mod engine_symbol_browse_impl; // 符号浏览一族（500 行门禁拆出，见文件头）
#[path = "engine_user_words.rs"]
mod engine_user_words_impl; // 用户词与开关一族（500 行门禁拆出，见文件头）
#[path = "engine_wubi.rs"]
mod engine_wubi_impl; // 五笔一族：风格/候选个数/自动上屏/编码提示/回车设置位（500 行门禁拆出，见文件头）
pub mod fuzzy;
pub mod jianpin;
pub mod keys;
pub mod learner;
pub mod pinyin;
pub mod polyphone; // 多音字补读音纠偏层（2026-10-15，见文件头）
pub mod punctuation;
pub mod router;
#[path = "router_edit_keys.rs"]
mod router_edit_keys_impl; // 编辑键一族（退格/空格/回车，2026-10-14 从 router.rs 拆出）
pub mod scheme;
pub mod shuangpin;
pub mod shuangpin_tables;
pub mod symbol_browse;
pub mod symbols;
pub mod trad_chars;
pub mod trie;

pub use composer::{Composer, KeyEffect, Mode, Session};
pub use dictionary::{Dictionary, InMemoryDictionary};
pub use engine::Engine;
pub use engine_enter_impl::EnterAction;
pub use engine_wubi_impl::{
    WUBI_CANDIDATE_LIMIT_DEFAULT, WUBI_CANDIDATE_LIMIT_MAX, WubiMixedEntry, WubiReturnStyle,
    WubiStyle,
};
pub use learner::{Learner, UserWord, UserWordExport};
pub use router::{KeyAction, KeyRouter, ShiftState};
pub use scheme::Scheme;
pub use trie::Entry;
