// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 用户词条（学习记录）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserWord {
    pub text: String,
    pub freq: u32,
}

/// 导出 JSON 的顶层结构。`version` **不是预留、是在用**：[`Learner::import_json`] 里
/// `version != 1` 直接拒绝（Android 经 `exportUserWords` / `importUserWords` 在用）。
/// 云同步 2026-09-28 已裁决**不做**（原注释「为将来云同步预留」已过期）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserWordExport {
    pub version: u32,
    pub words: Vec<UserWord>,
}

/// 导入词表条数上限。导入是信任边界（文件来自设备存储/恢复备份），
/// 上限挡住畸形文件把内存打爆。
pub const MAX_IMPORT_WORDS: usize = 100_000;

pub struct Learner {
    enabled: bool,
    user_freq: BTreeMap<String, u32>,
    user_words: BTreeSet<String>,
}

impl Learner {
    pub fn new(enabled: bool) -> Self {
        Learner {
            enabled,
            user_freq: BTreeMap::new(),
            user_words: BTreeSet::new(),
        }
    }

    /// 记录一次选词。disabled 时为 no-op。
    pub fn record_selection(&mut self, text: &str) {
        if !self.enabled {
            return;
        }
        // saturating：导入的词频可来自外部数据（最大 u32::MAX），+1 不得 panic。
        let slot = self.user_freq.entry(text.to_string()).or_insert(0);
        *slot = slot.saturating_add(1);
        self.user_words.insert(text.to_string());
    }

    /// 删除自造词（同时清掉频次）。
    pub fn remove_word(&mut self, text: &str) {
        self.user_freq.remove(text);
        self.user_words.remove(text);
    }

    pub fn clear(&mut self) {
        self.user_freq.clear();
        self.user_words.clear();
    }

    /// 用户词频查询（无记录返回 0）。
    pub fn freq_of(&self, text: &str) -> u32 {
        self.user_freq.get(text).copied().unwrap_or(0)
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// 导出为用户可读/可迁移的 JSON。
    pub fn export_json(&self) -> String {
        let words = self
            .user_words
            .iter()
            .map(|w| UserWord {
                text: w.clone(),
                freq: self.user_freq.get(w).copied().unwrap_or(0),
            })
            .collect();
        serde_json::to_string(&UserWordExport { version: 1, words }).unwrap()
    }

    /// 从 [`Learner::export_json`] 的产物导入用户词（纯逻辑，不读文件——读文件由调用方负责，
    /// engine-core 无 IO 依赖）。
    ///
    /// 三条刻意定下的语义：
    /// - **合并**，不是替换：文件里的词并入内存，内存里多出来的词原样保留；重复词频次
    ///   **取 max**，不是累加 —— Android 每次启动都会读同一个文件，累加会让频次随启动
    ///   次数无界膨胀把静态词频彻底压垮，取 max 则导入幂等。
    /// - **与 `enabled` 无关**：`enabled` 只关「记录新选词」，用户自己的数据照样载入
    ///   （`freq_of` 本来就恒参与排序，与 enabled 无关）；若按 no-op 处理，用户关掉学习
    ///   再打开时词库会凭空消失。
    /// - **全有或全无**：版本不符 / JSON 非法 / 缺字段 / 空 text / 超过
    ///   [`MAX_IMPORT_WORDS`] 一律返回 Err **且不改动内存**，调用方可安全重试。
    ///
    /// 返回导入的词条数（= 文件里的 words 条数，重复也计入）。
    pub fn import_json(&mut self, json: &str) -> Result<usize, String> {
        let parsed: UserWordExport =
            serde_json::from_str(json).map_err(|e| format!("用户词表解析失败: {e}"))?;
        if parsed.version != 1 {
            return Err(format!("用户词表版本不支持: {}（只认 1）", parsed.version));
        }
        if parsed.words.len() > MAX_IMPORT_WORDS {
            return Err(format!(
                "用户词表过大: {} 条（上限 {MAX_IMPORT_WORDS}）",
                parsed.words.len()
            ));
        }
        if let Some(bad) = parsed.words.iter().find(|w| w.text.is_empty()) {
            return Err(format!("用户词表含空词条（freq={}）", bad.freq));
        }
        // 校验全部通过后才落内存，保证「全有或全无」。
        // user_words 与 user_freq 必须同时更新，否则 export_json 会漏词。
        let count = parsed.words.len();
        for w in parsed.words {
            self.user_freq
                .entry(w.text.clone())
                .and_modify(|f| *f = (*f).max(w.freq))
                .or_insert(w.freq);
            self.user_words.insert(w.text);
        }
        Ok(count)
    }
}

// 单测独立成文件（`#[path]` 引入）以保持本文件 <500 行，与 candidates_tests.rs 同惯例。
#[cfg(test)]
#[path = "learner_tests.rs"]
mod tests;
