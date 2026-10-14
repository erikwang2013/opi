// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「属性设置」的**读侧**（Windows）：`%LOCALAPPDATA%\opi\settings.ini`。
//!
//! 写侧是 Compose 设置窗（`desktop/src/main/kotlin/xyz/erik/opi/candidate/Settings.kt`，
//! `opi-candidates --settings`）。两端**共用同一批键名**（清单见 [`parse`]，逐键语义在
//! [`Settings`] 的字段注释里）—— 改键名 = 老配置文件全部失效，三处同改
//! （Kotlin 写侧 ⇄ 本文件 ⇄ fcitx5 的 `cpp/opi_config.h`）。
//! ⚠️ 其中 `DisabledDicts` 是**名单**不是开关：空 = 全部启用（见 [`Settings`]）。
//!
//! 本模块**平台中立**（无 windows 类型）且是**纯函数**（路径与环境量都由参数给）——
//! 与 `dict_path.rs`/`vk.rs` 同理：解析判定若留在 `#[cfg(target_os = "windows")]` 的
//! 胶水里，本机（Linux）门禁就一条都覆盖不到，而它的失败面是**静默**的
//! （开关不生效，用户以为改了）。
//!
//! 【生效时机，如实记】本端**没有在线刷新通道**：设置窗落盘后，值在下一次
//! `ITfTextInputProcessor::Activate`（= 切到别的应用再切回来）时读进来生效。
//! 升级路径：设置窗与候选窗是同一条命名管道的两端，加一条 `{"type":"reload"}`
//! 消息即可即时生效 —— 那要同时改线协议两侧（`candidate_io.rs` 与
//! `desktop/.../Protocol.kt`），本轮未做。

use std::ffi::OsStr;
use std::path::PathBuf;

/// 设置文件名（与 Compose 写侧约定一致：`<LOCALAPPDATA>/opi/settings.ini`）。
pub const SETTINGS_FILE_NAME: &str = "settings.ini";

/// 四个开关 + 模糊音逐对列表。默认值 = **引擎既有行为**（全开）—— 与 Kotlin 写侧、
/// fcitx5 读侧的默认一字不差：三处默认值不同的表现是「没配过 = 随机一种行为」。
///
/// `fuzzy_pairs`（2026-10-13 加）：逗号分隔的组名串（规范见
/// `engine_core::fuzzy::pair_names`）。**空串 = 未设置 ⇒ 回落到旧的 `fuzzy` 布尔**
/// （老配置一字不动）；非空 ⇒ 以它为准（`fuzzy` 位仅作旧读侧的兼容投影，由设置面
/// 同步写「任一组开着」）。加字段后本结构**不再是 `Copy`**（String 字段）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub learn: bool,
    pub chinese_punct: bool,
    pub fuzzy: bool,
    pub fuzzy_pairs: String,
    pub jianpin: bool,
    /// 拼音纠错（2026-10-13；键 `Correction`）。
    pub correction: bool,
    /// 双拼布局（2026-10-13；键 `ShuangpinLayout`）：**方案名**（`自然码`/`微软`/
    /// 小鹤/智能ABC/紫光/拼音加加 —— 与引擎 `shuangpin_tables.rs` 的表同名，桌面写侧
    /// 同此）。**空串 = 未设置 ⇒ 引擎保持既有布局**（自然码 = 默认）；名字认不得同样
    /// 不动引擎（写坏一个名字不该静默改用户的布局）。
    pub shuangpin_layout: String,
    /// 拼音方式（2026-10-14；键 `PinyinMethod`）：**方案名**（`全拼`/`双拼` —— 名字表
    /// 见 `engine_core::Scheme::name`，桌面写侧同此）。**空串 = 未设置 ⇒ 引擎保持既有
    /// 方案**（全拼）；名字认不得同样不动引擎。
    pub pinyin_method: String,
    /// 导入词库的**停用名单**（2026-10-14；键 `DisabledDicts`）：逗号分隔的**词库名**
    /// （= 文件名去 `.tsv`），原样收，切分与去空白在 `extra_dicts::parse_disabled`。
    /// **空串 = 全部启用**——⚠️ 与上面的 `fuzzy_pairs` **反着来**（那里空 = 回落布尔）：
    /// 名单型选项的两种空语义别串。
    pub disabled_dicts: String,
    /// 五笔（2026-10-14，M4 第二波）：方案 / 风格 / 混输进入方式 / 候选个数 / 回车 /
    /// 自动上屏。名字表 = `engine_core::WubiStyle::name` 那一族（**唯一真源**在
    /// engine-core），认不得/空串一律**不动引擎**（与 pinyin_method 同一条姿态）。
    /// 方案只认 "86" / "98"（认不得按 86 —— 与 fcitx5 轨 `opiWubiSync` 同口径）。
    pub wubi_plan: String,
    pub wubi_style: String,
    pub wubi_mixed_entry: String,
    pub wubi_candidate_limit: i64,
    pub wubi_return: String,
    pub wubi_auto_commit: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            learn: true,
            chinese_punct: true,
            fuzzy: true,
            fuzzy_pairs: String::new(), // 空 = 未设置（回落到 fuzzy 布尔）
            jianpin: true,
            correction: true,
            shuangpin_layout: String::new(), // 空 = 未设置（引擎保持默认布局）
            pinyin_method: String::new(),    // 空 = 未设置（引擎保持全拼）
            disabled_dicts: String::new(),   // 空 = 全部启用（不是"未设置"）
            wubi_plan: String::new(),        // 空 = 未设置（按 86）
            wubi_style: String::new(),       // 空 = 未设置（纯五笔）
            wubi_mixed_entry: String::new(), // 空 = 未设置（直接字母）
            wubi_candidate_limit: 0,         // 0/负 = 未设置（引擎缺省 5）
            wubi_return: String::new(),      // 空 = 未设置（清空）
            wubi_auto_commit: true,          // 缺省开（对齐搜狗）
        }
    }
}

/// 解析 ini 文本。**永不失败**：手改坏的配置文件只该让坏掉的那几项回默认，
/// 不该让输入法不出字。
///
/// 容错口径（与 Kotlin 写侧/读侧同一套，单测在 `settings_tests.rs`）：
/// 键名区分大小写（写侧就写这四个）、值大小写不敏感、`#` 起行内注释、
/// 未知键忽略（向前兼容：新版本写的文件在旧版本上照读）。
pub fn parse(text: &str) -> Settings {
    let mut out = Settings::default();
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let key = k.trim();
        if key == "DisabledDicts" {
            // 字符串键（2026-10-14）：值原样收（`#` 注释与空白已在上文剥过）。
            // 名字里的空白由 `parse_disabled` 逐段 trim —— 这里不先切（切分口径只留一份）。
            out.disabled_dicts = v.trim().to_string();
            continue;
        }
        if key == "WubiPlan" || key == "WubiStyle" || key == "WubiMixedEntry" || key == "WubiReturn"
        {
            // 字符串键（2026-10-14，M4 第二波）：值原样收，语义由 apply_settings 换算
            // （认不得的名字一律不动引擎 —— 与 PinyinMethod/ShuangpinLayout 同姿态）。
            match key {
                "WubiPlan" => out.wubi_plan = v.trim().to_string(),
                "WubiStyle" => out.wubi_style = v.trim().to_string(),
                "WubiMixedEntry" => out.wubi_mixed_entry = v.trim().to_string(),
                _ => out.wubi_return = v.trim().to_string(),
            }
            continue;
        }
        if key == "WubiCandidateLimit" {
            // **唯一的数字键**（2026-10-14）：解析失败按「未设置」（0）—— 与「坏值回
            // 默认」同一档，别让它落到下面的布尔分支（那里会整条忽略，语义不同）。
            out.wubi_candidate_limit = v.trim().parse::<i64>().unwrap_or(0);
            continue;
        }
        if key == "PinyinMethod" {
            // 字符串键（2026-10-14）：值原样收（注释与空白已在上文剥过）。
            out.pinyin_method = v.trim().to_string();
            continue;
        }
        if key == "ShuangpinLayout" {
            // 字符串键（2026-10-13）：值原样收（`#` 注释与空白已在上文剥过）。
            out.shuangpin_layout = v.trim().to_string();
            continue;
        }
        if key == "FuzzyPairs" {
            // 字符串键（2026-10-13）：值原样收（`#` 注释与空白已在上文剥过）。
            out.fuzzy_pairs = v.trim().to_string();
            continue;
        }
        let Some(b) = parse_bool(v.trim()) else {
            continue;
        };
        match key {
            "Learn" => out.learn = b,
            "ChinesePunct" => out.chinese_punct = b,
            "Fuzzy" => out.fuzzy = b,
            "Jianpin" => out.jianpin = b,
            "Correction" => out.correction = b,
            "WubiAutoCommit" => out.wubi_auto_commit = b,
            _ => {} // 未知键：忽略（向前兼容），不报错
        }
    }
    out
}

fn parse_bool(v: &str) -> Option<bool> {
    match v.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// `%LOCALAPPDATA%\opi\settings.ini`（写侧 `Settings.kt` 的 Windows 分支同此）。
/// `local_appdata` 缺省/空串 → 相对路径 `opi/settings.ini`（与 `dict_path.rs`
/// 对缺环境变量的处理同口径：不 panic，退化成"读不到 → 全默认"）。
pub fn path_in(local_appdata: Option<&OsStr>) -> PathBuf {
    let base = local_appdata
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_default();
    base.join("opi").join(SETTINGS_FILE_NAME)
}

/// 读设置。**读不到 = 全默认**（不是错误：没配过就是没配过）。
pub fn load(local_appdata: Option<&OsStr>) -> Settings {
    match std::fs::read_to_string(path_in(local_appdata)) {
        Ok(text) => parse(&text),
        Err(_) => Settings::default(),
    }
}

// 单测独立成文件（`#[path]` 引入）以保持各文件 <500 行：settings_tests.rs
#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
