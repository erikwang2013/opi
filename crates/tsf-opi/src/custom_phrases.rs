// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 自定义短语的**宿主文件**读取（2026-10-24）：Windows 侧。
//!
//! 唯一真源 = `<opi 数据目录>/custom_phrases.json`（中文套）与
//! `custom_phrases_en.json`（英文套，2026-10-24；与 `settings.ini`、词库目录同处
//! `%LOCALAPPDATA%\opi`）：`{"version":1,"enabled":true,"phrases":[...]}`。两套文件、
//! 开关、引擎存储完全独立。引擎只收 `version + phrases`（`Engine::set_custom_phrases`
//! / `set_english_custom_phrases`），启用态是独立写口 —— 本模块把两者拆开推给引擎。
//!
//! 失败语义（与 Android `CustomPhraseStore.load` 一致）：**文件缺失/读不出/JSON
//! 不合法 = 默认值（启用 + 空表）**，绝不 panic。`phrases` 的条目是否合规则由引擎
//! 判（整表拒收时这里退回空表，等价于「坏文件 = 没配过」）。
//!
//! 生效时机：服务创建 / `Activate` / 控制面 `reload`（设置窗保存）—— 三处都在
//! `apply_settings` 之后紧跟 [`load`]/[`load_english`] 与两个 `apply_*_custom_phrases`。
//!
//! 配对：fcitx5 轨 `data_dir.rs` 的同名读取（两轨同形，路径推导各随平台）。

use std::path::{Path, PathBuf};

use engine_core::Engine;

use crate::logic::TsfLogic;

/// 中文套文件名（与桌面设置窗 `CUSTOM_PHRASES_FILE_NAME` 逐字一致）。
pub const FILE_NAME: &str = "custom_phrases.json";

/// 英文套文件名（与桌面设置窗 `CUSTOM_PHRASES_EN_FILE_NAME` 逐字一致）。
pub const EN_FILE_NAME: &str = "custom_phrases_en.json";

/// 引擎合同 JSON 的空表（含 version；不含 enabled）。
pub(crate) const EMPTY_JSON: &str = r#"{"version":1,"phrases":[]}"#;

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
            json: EMPTY_JSON.to_string(),
        }
    }
}

/// 解析宿主文件。任何结构问题 → `None`（调用方按 [`PhraseState::default`] 处理）。
/// 校验口径：顶层对象、`version` 存在且为 1、`phrases` 是数组；`enabled` 缺省 true。
pub fn parse(text: &str) -> Option<PhraseState> {
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

/// 读指定文件；缺失/读不出/不合法 → 默认值。
pub fn load_file(path: &Path) -> PhraseState {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| parse(&text))
        .unwrap_or_default()
}

/// 宿主文件路径：`dicts_dir` 的父目录（= `%LOCALAPPDATA%\opi`）+ [`FILE_NAME`]。
/// `dicts_dir` 由 `dict_path::dicts_dir` 推得（`<数据目录>/dicts`）；父目录取不到
/// （空路径等退化情形）就退回 `dicts_dir` 本身 —— 读不到只是空表，不 panic。
pub fn path_from_dicts_dir(dicts_dir: &Path) -> PathBuf {
    dicts_dir.parent().unwrap_or(dicts_dir).join(FILE_NAME)
}

/// 英文套路径：与中文套同一父目录，独立文件。
pub fn english_path_from_dicts_dir(dicts_dir: &Path) -> PathBuf {
    dicts_dir.parent().unwrap_or(dicts_dir).join(EN_FILE_NAME)
}

/// 按导入词库目录读中文套宿主文件（三处生产调用方的统一入口）。
pub fn load(dicts_dir: &Path) -> PhraseState {
    load_file(&path_from_dicts_dir(dicts_dir))
}

/// 按导入词库目录读英文套宿主文件（与 [`load`] 同一失败语义）。
pub fn load_english(dicts_dir: &Path) -> PhraseState {
    load_file(&english_path_from_dicts_dir(dicts_dir))
}

impl TsfLogic {
    /// 中文套整表推给引擎（启用态分开推）。引擎拒收时退回空表 + 启用 —— 与
    /// [`PhraseState::default`] 同一失败语义。短语参与候选，必须清抓取缓存。
    pub fn apply_custom_phrases(&mut self, state: PhraseState) {
        apply_to_engine(
            &mut self.engine,
            &state,
            Engine::set_custom_phrases,
            Engine::set_custom_phrases_enabled,
        );
        self.invalidate_fetch_cache();
    }

    /// 英文套整表推给引擎；失败语义与中文套相同，但只动英文槽位。
    pub fn apply_english_custom_phrases(&mut self, state: PhraseState) {
        apply_to_engine(
            &mut self.engine,
            &state,
            Engine::set_english_custom_phrases,
            Engine::set_english_custom_phrases_enabled,
        );
        self.invalidate_fetch_cache();
    }
}

fn apply_to_engine(
    engine: &mut Engine,
    state: &PhraseState,
    set: fn(&mut Engine, &str) -> Result<usize, String>,
    set_enabled: fn(&mut Engine, bool),
) {
    if set(engine, &state.json).is_err() {
        let _ = set(engine, EMPTY_JSON);
        // 坏表回退「关 + 空表」：缺省关（2026-10-25），别让坏文件把开关悄悄打开。
        set_enabled(engine, false);
        return;
    }
    set_enabled(engine, state.enabled);
}

// 单测独立成文件（`#[path]` 引入）以保持各文件 <500 行。
#[cfg(test)]
#[path = "logic_phrases_tests.rs"]
mod tests;
