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

/// 导出 JSON 的顶层结构，version 为将来云同步的格式协商预留。
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_learner_ignores_selections() {
        let mut l = Learner::new(false);
        l.record_selection("好");
        assert_eq!(l.freq_of("好"), 0);
        assert_eq!(l.export_json(), r#"{"version":1,"words":[]}"#);
    }

    #[test]
    fn records_and_counts_selections() {
        let mut l = Learner::new(true);
        l.record_selection("好");
        l.record_selection("好");
        l.record_selection("号");
        assert_eq!(l.freq_of("好"), 2);
        assert_eq!(l.freq_of("号"), 1);
    }

    #[test]
    fn remove_word_drops_freq() {
        let mut l = Learner::new(true);
        l.record_selection("好");
        l.remove_word("好");
        assert_eq!(l.freq_of("好"), 0);
    }

    #[test]
    fn clear_empties_everything() {
        let mut l = Learner::new(true);
        l.record_selection("好");
        l.clear();
        assert_eq!(l.freq_of("好"), 0);
        assert_eq!(l.export_json(), r#"{"version":1,"words":[]}"#);
    }

    /// 导入 = 与内存状态**合并**，重复词取 max（不是累加）：同一份 JSON 重复导入
    /// （Android 每次启动都会读同一个文件）必须幂等，否则频次无界膨胀。
    #[test]
    fn import_json_merges_by_max_freq() {
        let mut l = Learner::new(true);
        l.record_selection("好"); // 内存里已有 1 次
        let json = r#"{"version":1,"words":[{"text":"好","freq":5},{"text":"号","freq":3}]}"#;
        assert_eq!(l.import_json(json).unwrap(), 2);
        assert_eq!(l.freq_of("好"), 5, "取 max(1,5)");
        assert_eq!(l.freq_of("号"), 3, "新词按文件频次进内存");
        // 幂等：再导入一次不变
        assert_eq!(l.import_json(json).unwrap(), 2);
        assert_eq!(l.freq_of("好"), 5);
        // 两棵树一致：user_words 也含导入词，导出才有它们
        let parsed: UserWordExport = serde_json::from_str(&l.export_json()).unwrap();
        assert_eq!(parsed.words.len(), 2);
    }

    /// 导入的数据可比内存新（手机上删了自造词又恢复备份）：取 max 而非只增不减。
    #[test]
    fn import_json_never_lowers_existing_freq() {
        let mut l = Learner::new(true);
        for _ in 0..9 {
            l.record_selection("好");
        }
        l.import_json(r#"{"version":1,"words":[{"text":"好","freq":2}]}"#)
            .unwrap();
        assert_eq!(l.freq_of("好"), 9);
    }

    /// enabled=false 只关「记录新选词」，不关「载入用户自己的数据」：
    /// freq_of 本来就参与排序（与 enabled 无关），导入被判为 no-op 会让用户
    /// 关掉学习后再打开时词库凭空消失。
    #[test]
    fn import_json_applies_even_when_disabled() {
        let mut l = Learner::new(false);
        assert_eq!(
            l.import_json(r#"{"version":1,"words":[{"text":"好","freq":2}]}"#)
                .unwrap(),
            1
        );
        assert_eq!(l.freq_of("好"), 2);
        l.record_selection("好"); // 仍不记录
        assert_eq!(l.freq_of("好"), 2);
    }

    #[test]
    fn import_json_rejects_malformed_and_wrong_version() {
        let mut l = Learner::new(true);
        for bad in [
            "",
            "not json",
            "{}",                                              // 缺 version/words
            r#"{"version":1}"#,                                // 缺 words
            r#"{"version":2,"words":[]}"#,                     // 版本不符
            r#"{"version":1,"words":[{"text":"好"}]}"#,        // 词条缺 freq
            r#"{"version":1,"words":[{"freq":3}]}"#,           // 词条缺 text
            r#"{"version":1,"words":[{"text":"","freq":3}]}"#, // 空词
        ] {
            assert!(l.import_json(bad).is_err(), "应拒绝：{bad}");
        }
        assert_eq!(
            l.export_json(),
            r#"{"version":1,"words":[]}"#,
            "失败不得部分导入"
        );
    }

    /// 超大词表直接拒绝：导入是信任边界（文件来自设备存储/恢复备份），
    /// 上限挡住畸形文件把内存打爆。
    #[test]
    fn import_json_rejects_oversized_word_list() {
        let mut l = Learner::new(true);
        let one = r#"{"text":"好","freq":1}"#;
        let body = std::iter::repeat_n(one, MAX_IMPORT_WORDS + 1)
            .collect::<Vec<_>>()
            .join(",");
        let json = format!(r#"{{"version":1,"words":[{body}]}}"#);
        assert!(l.import_json(&json).is_err());
        // 恰好等于上限则接受
        let body = std::iter::repeat_n(one, MAX_IMPORT_WORDS)
            .collect::<Vec<_>>()
            .join(",");
        let json = format!(r#"{{"version":1,"words":[{body}]}}"#);
        assert_eq!(l.import_json(&json).unwrap(), MAX_IMPORT_WORDS);
        assert_eq!(l.freq_of("好"), 1);
    }

    /// 导入频次可能已是 u32::MAX（外部数据），此后再选词不得溢出 panic。
    #[test]
    fn record_after_max_freq_import_saturates() {
        let mut l = Learner::new(true);
        l.import_json(r#"{"version":1,"words":[{"text":"好","freq":4294967295}]}"#)
            .unwrap();
        l.record_selection("好");
        assert_eq!(l.freq_of("好"), u32::MAX);
    }

    #[test]
    fn export_json_has_version_and_words() {
        let mut l = Learner::new(true);
        l.record_selection("好");
        l.record_selection("号");
        let json = l.export_json();
        let parsed: UserWordExport = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.words.len(), 2);
    }
}
