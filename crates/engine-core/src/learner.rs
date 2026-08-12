use std::collections::{BTreeMap, BTreeSet};
use serde::{Deserialize, Serialize};

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

pub struct Learner {
    enabled: bool,
    user_freq: BTreeMap<String, u32>,
    user_words: BTreeSet<String>,
}

impl Learner {
    pub fn new(enabled: bool) -> Self {
        Learner { enabled, user_freq: BTreeMap::new(), user_words: BTreeSet::new() }
    }

    /// 记录一次选词。disabled 时为 no-op。
    pub fn record_selection(&mut self, text: &str) {
        if !self.enabled {
            return;
        }
        *self.user_freq.entry(text.to_string()).or_insert(0) += 1;
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
