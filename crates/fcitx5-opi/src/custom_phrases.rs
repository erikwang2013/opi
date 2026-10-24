// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 自定义短语的**宿主文件**读取（2026-10-24 从 `data_dir.rs` 拆出：那边加本块
//! 越了 500 行门禁）。
//!
//! 真源 = `<数据目录>/custom_phrases.json`（中文套）与 `custom_phrases_en.json`
//! （英文套，2026-10-24；桌面设置窗写；schema 见
//! `desktop/.../CustomPhraseCodec.kt`）。引擎收 `version + phrases`，启用态是独立
//! 写口 —— 本层只读文件 + 拆字段；推给引擎的编排在 `candidate_settings.rs`。
//! 失败语义与 TSF 轨 `custom_phrases.rs` 逐条一致：缺失/坏文件 = 启用 + 空表。

use std::fs;
use std::path::{Path, PathBuf};

use crate::data_dir::xdg_data_dir;

/// 中文套文件名（与桌面设置窗、TSF 轨逐字一致）。
pub const CUSTOM_PHRASES_FILE_NAME: &str = "custom_phrases.json";

/// 英文套文件名（与桌面设置窗、TSF 轨逐字一致）。
pub const ENGLISH_CUSTOM_PHRASES_FILE_NAME: &str = "custom_phrases_en.json";

/// 引擎合同 JSON 的空表（含 version；不含 enabled）。
pub(crate) const EMPTY_CUSTOM_PHRASES_JSON: &str = r#"{"version":1,"phrases":[]}"#;

/// 宿主文件解析结果。`json` 已剥掉 `enabled`（引擎不认那个键）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhraseState {
    pub enabled: bool,
    pub json: String,
}

impl Default for PhraseState {
    fn default() -> Self {
        Self {
            // 缺省关（2026-10-25 用户裁决；与引擎/Android 宿主文件同一缺省）。
            enabled: false,
            json: EMPTY_CUSTOM_PHRASES_JSON.to_string(),
        }
    }
}

/// 解析宿主文件。任何结构问题 → `None`（调用方按 [`PhraseState::default`] 处理）。
/// 校验口径：顶层对象、`version` 存在且为 1、`phrases` 是数组；`enabled` 缺省 true。
pub fn parse_custom_phrases(text: &str) -> Option<PhraseState> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let obj = value.as_object()?;
    let enabled = match obj.get("enabled") {
        None => false,
        Some(serde_json::Value::Bool(on)) => *on,
        Some(_) => return None, // 类型错 = 整份坏（与 Kotlin 侧同一姿态）
    };
    if obj.get("version").and_then(serde_json::Value::as_u64) != Some(1) {
        return None;
    }
    let phrases = obj.get("phrases")?;
    if !phrases.is_array() {
        return None;
    }
    // 只把引擎认识的键原样带过去；条目本身（trigger/text）由引擎校验。
    let json = serde_json::json!({"version": 1, "phrases": phrases}).to_string();
    Some(PhraseState { enabled, json })
}

/// 宿主文件路径（`$XDG_DATA_HOME/opi/custom_phrases.json`，见 [`xdg_data_dir`]）。
pub fn custom_phrases_path() -> PathBuf {
    xdg_data_dir().join(CUSTOM_PHRASES_FILE_NAME)
}

/// 读指定文件；缺失/读不出/不合法 → 默认值。
pub fn load_custom_phrases_file(path: &Path) -> PhraseState {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| parse_custom_phrases(&text))
        .unwrap_or_default()
}

/// 英文套宿主文件路径（与中文套同一目录、独立文件）。
pub fn english_custom_phrases_path() -> PathBuf {
    xdg_data_dir().join(ENGLISH_CUSTOM_PHRASES_FILE_NAME)
}

/// 从数据目录读中文套宿主文件（`applyOpiSettings` 的冷路径唯一入口）。
pub fn load_custom_phrases() -> PhraseState {
    load_custom_phrases_file(&custom_phrases_path())
}

/// 从指定数据目录读英文套（测试用；生产入口用 [`load_english_custom_phrases`]）。
pub(crate) fn load_english_custom_phrases_from(dir: &Path) -> PhraseState {
    load_custom_phrases_file(&dir.join(ENGLISH_CUSTOM_PHRASES_FILE_NAME))
}

/// 从数据目录读英文套宿主文件（失败语义与中文套相同）。
pub fn load_english_custom_phrases() -> PhraseState {
    load_english_custom_phrases_from(&xdg_data_dir())
}

impl crate::candidate::CandidateState {
    /// 英文短语实际可命中（开关开 + 表非空，2026-10-24）：此时普通英文模式也要把
    /// 字母送进引擎缓冲 —— 直出模式下字母已进文档，短语永远匹配不到。
    pub fn english_phrases_active(&self) -> bool {
        self.engine.english_custom_phrases_active()
    }
}

// 单测独立成文件（`#[path]` 引入）以保持各文件 <500 行。
#[cfg(test)]
#[path = "custom_phrases_tests.rs"]
mod tests;
