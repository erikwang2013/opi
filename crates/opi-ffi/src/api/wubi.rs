// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **五笔旋钮一族**（M4 第二波，2026-10-14）：`loadWubi` / 风格 / 混输进入方式 /
//! 候选个数 / 回车行为 / 自动上屏 / 编码提示 —— 两个 ABI 面（JNI `jni_wubi.rs` +
//! C `cabi_wubi.rs`）共用这一份，上接 `engine_core::engine_wubi` 的设置位与
//! `Engine::set_wubi_dict`。
//!
//! 单独一个文件是 500 行硬规矩（同 `api/dict.rs` 的分家思路）。
//!
//! ## 整数码（**协议**，四端同名同值，改它 = 改协议）
//!
//! | 出口 | 取值 |
//! |---|---|
//! | `wubiStyle` | 0=纯五笔 1=五笔拼音混输（**越界忽略**） |
//! | `wubiMixedEntry` | 0=直接字母 1=z键（**越界忽略**；只在混输下有语义） |
//! | `wubiReturnStyle` | 0=清空 1=上屏原文（**越界忽略**；消费在四端路由，见引擎侧文档） |
//! | `wubiCandidateLimit` | 1..=9，**越界钳到区间**（与双拼布局同纪律：坏值钳到确定值） |
//!
//! 缺省值一律来自 `Engine` 侧的 `Default`（纯五笔 / 直接字母 / 清空 / 自动上屏开 /
//! 个数 5）—— 本层**不另写一套默认**，免得两处漂。
//!
//! ## 与 `scheme` 的关系
//!
//! 本族只写**运行期旋钮**，不换方案：要用五笔得先 `setScheme(5)`（协议表见
//! `engine_core::scheme::Scheme::code`）。码表没装（`loadWubi` 没调或失败）时，
//! 方案走了普通路径 —— 引擎如实降级，不假装有候选（`engine_wubi.rs` 头的裁决）。

use engine_core::{WubiMixedEntry, WubiReturnStyle, WubiStyle};

use super::Api;

/// 风格整数码 → 枚举。越界 → None（写侧忽略）。
pub fn wubi_style_from_int(v: i32) -> Option<WubiStyle> {
    match v {
        0 => Some(WubiStyle::Pure),
        1 => Some(WubiStyle::Mixed),
        _ => None,
    }
}

/// 枚举 → 整数码（与 [`wubi_style_from_int`] 逐值对应；读侧用）。
pub fn wubi_style_to_int(s: WubiStyle) -> i32 {
    match s {
        WubiStyle::Pure => 0,
        WubiStyle::Mixed => 1,
    }
}

/// 混输进入方式整数码 → 枚举。越界 → None。
pub fn wubi_mixed_entry_from_int(v: i32) -> Option<WubiMixedEntry> {
    match v {
        0 => Some(WubiMixedEntry::DirectLetters),
        1 => Some(WubiMixedEntry::ZKey),
        _ => None,
    }
}

pub fn wubi_mixed_entry_to_int(e: WubiMixedEntry) -> i32 {
    match e {
        WubiMixedEntry::DirectLetters => 0,
        WubiMixedEntry::ZKey => 1,
    }
}

/// 回车行为整数码 → 枚举。越界 → None。
pub fn wubi_return_from_int(v: i32) -> Option<WubiReturnStyle> {
    match v {
        0 => Some(WubiReturnStyle::Clear),
        1 => Some(WubiReturnStyle::CommitRaw),
        _ => None,
    }
}

pub fn wubi_return_to_int(s: WubiReturnStyle) -> i32 {
    match s {
        WubiReturnStyle::Clear => 0,
        WubiReturnStyle::CommitRaw => 1,
    }
}

impl Api {
    /// 装载五笔码表（JNI `loadWubi` / C `opi_load_wubi`）。与 `install_trad` /
    /// `install_stroke` 同一条**严格**语义：`load_mmap`（坏路径**不**回落内置 ——
    /// 内置是 35 词拼音库，装成五笔码表语义错误）⇒ 返回 Err（调用方按 false 处理，
    /// 方案回退普通路径）。引擎未装载时同样 Err。
    pub fn load_wubi(&mut self, path: &str) -> Result<(), String> {
        let dict = engine_data::load_mmap(std::path::Path::new(path))
            .map_err(|e| format!("load wubi {}: {e:?}", path))?;
        self.router.engine_mut().set_wubi_dict(Some(Box::new(dict)));
        Ok(())
    }

    /// 风格写侧（`WubiStyle`；不清缓冲 —— 见引擎侧文档）。
    pub fn set_wubi_style(&mut self, style: WubiStyle) {
        self.router.engine_mut().set_wubi_style(style);
    }

    pub fn wubi_style(&self) -> WubiStyle {
        self.router.engine().wubi_style()
    }

    /// 混输进入方式写侧（`WubiMixedEntry`）。
    pub fn set_wubi_mixed_entry(&mut self, entry: WubiMixedEntry) {
        self.router.engine_mut().set_wubi_mixed_entry(entry);
    }

    pub fn wubi_mixed_entry(&self) -> WubiMixedEntry {
        self.router.engine().wubi_mixed_entry()
    }

    /// 候选个数写侧。**钳制在引擎侧**（1..=9）—— 本层不重复钳一遍（两处写必漂）。
    pub fn set_wubi_candidate_limit(&mut self, n: usize) {
        self.router.engine_mut().set_wubi_candidate_limit(n);
    }

    pub fn wubi_candidate_limit(&self) -> usize {
        self.router.engine().wubi_candidate_limit()
    }

    /// 回车行为写侧（设置位；消费在四端路由，见 `WubiReturnStyle` 头注）。
    pub fn set_wubi_return_style(&mut self, style: WubiReturnStyle) {
        self.router.engine_mut().set_wubi_return_style(style);
    }

    pub fn wubi_return_style(&self) -> WubiReturnStyle {
        self.router.engine().wubi_return_style()
    }

    /// 四码唯一自动上屏开关写侧（缺省开）。
    pub fn set_wubi_auto_commit(&mut self, on: bool) {
        self.router.engine_mut().set_wubi_auto_commit(on);
    }

    pub fn wubi_auto_commit(&self) -> bool {
        self.router.engine().wubi_auto_commit()
    }

    /// 编码提示（读侧）：当前缓冲可接续的键，**拼成一个串**（调用方按字符拆）。
    /// 非五笔方案 / 缓冲为空 / 无接续键 ⇒ 空串。⚠️ 引擎侧是 O(26×前缀查询)/次调用
    /// 且逐击键全量重算 —— 宿主**别在每帧渲染里调**（缓冲变化时取一次即可）。
    pub fn wubi_next_keys(&self) -> String {
        self.router
            .engine()
            .wubi_next_keys()
            .into_iter()
            .collect::<String>()
    }
}
