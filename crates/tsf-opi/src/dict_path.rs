// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 词库解析：环境变量 → DLL 同目录 → `%LOCALAPPDATA%\opi\` → 内置回退词库。
//!
//! 本模块**平台中立**（无 windows 类型）且路径解析是**纯函数**（输入全由参数给，
//! 不读环境、不碰文件系统）—— 与 `vk.rs`/`dll.rs` 同理：判定若留在
//! `#[cfg(target_os = "windows")]` 的胶水里，本机（Linux）门禁就永远覆盖不到。
//! Win32 那一步（取本 DLL 所在目录）在 `com_server::dll_dir`，只有那一处在
//! Windows 侧。
//!
//! 分发（**本仓库现状**，别把大文件塞进 git）：`luna.opid` 是本地重编产物，
//! 被 `data/generated/.gitignore` 忽略；入库的是各平台的**分发副本**
//! （`android/app/src/main/assets/luna.opid`）。Windows 侧同理：打包时把
//! `data/generated/luna.opid` 拷到本 DLL 同目录，或让用户放到
//! `%LOCALAPPDATA%\opi\luna.opid`（本模块的第三候选）。本仓库目前**没有**
//! Windows 打包脚本，故没有任何构建期拷贝动作 —— 这两个候选就是全部通路。
//!
//! 已知缺口（**未实现，不是 bug 隐藏**）：`trad.opid` 不在候选里 ——
//! `TsfLogic` 只持一个词库（引擎层无简繁双库入口），繁体模式无从装载。

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use engine_core::dictionary::Dictionary;
use engine_data::LoadError;

/// 词库文件名（镜像 fcitx5 轨 `data_dir::DICT_FILE_NAME` 与
/// Android `EngineLoader.FILE_NAME`）。
pub const DICT_FILE_NAME: &str = "luna.opid";

/// 环境变量：**直接给词库文件全路径**（开发/排障用覆盖，不做目录语义 ——
/// 目录语义要靠文件名约定猜，全路径没有歧义）。空串等同未设置。
pub const DICT_ENV_VAR: &str = "OPI_DICT_PATH";

/// 候选来源（顺序即优先级；日志与测试用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// `OPI_DICT_PATH` 给的全路径。
    Env,
    /// 本 DLL 所在目录。
    DllDir,
    /// `%LOCALAPPDATA%\opi\`（镜像 XDG 轨的 `~/.local/share/opi/`）。
    LocalAppData,
}

impl Origin {
    /// 日志里的来源名（与 `DICT_ENV_VAR` 同源，避免两处写死）。
    pub fn describe(self) -> &'static str {
        match self {
            Origin::Env => DICT_ENV_VAR,
            Origin::DllDir => "DLL 同目录",
            Origin::LocalAppData => "%LOCALAPPDATA%\\opi",
        }
    }
}

/// 一个候选：路径 + 来源（来源只为日志，不参与判定）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub path: PathBuf,
    pub origin: Origin,
}

/// 装载结果。**永不失败** —— 全候选不可用时落到内置回退词库（35 词），
/// 与 `TsfLogic::load(None)` 同一条路（TSF 服务不能因为词库缺失就创建不出来，
/// 那会让用户看到"输入法整个不在"，比词库小更糟）。
pub struct DictLoad {
    pub dict: Box<dyn Dictionary>,
    /// 装上的候选；`None` = 全败，用的是内置回退词库。
    pub source: Option<Candidate>,
    /// 逐候选结论（不存在 / 不可读），供调用方记日志；成功装载时**不含**胜出项。
    pub notes: Vec<String>,
}

impl DictLoad {
    /// 一行日志（格式留在本模块：`com_server` 只有 `eprintln!` 一行，
    /// 那句在 Windows 侧跑不到，格式逻辑放那边就等于没测过）。
    pub fn log_line(&self) -> String {
        match &self.source {
            Some(c) => format!("词库 {}（来源：{}）", c.path.display(), c.origin.describe()),
            None => format!("无可装载词库（{}）→ 内置回退词库", self.notes.join("；")),
        }
    }
}

/// 候选路径表，按优先级排列。**纯函数**：不读环境、不碰文件系统，输入全由参数给
/// （`env` 为 `OPI_DICT_PATH` 的值，`local_app_data` 为 `%LOCALAPPDATA%` 的值）。
///
/// 优先级理由：DLL 同目录是**随包发的那一份**（版本与插件同批，升级即生效），
/// 排在三者中间；`%LOCALAPPDATA%\opi\`（小写 `opi`，对齐 XDG 轨的
/// `~/.local/share/opi` 与 spec `2026-08-14-opi-multi-platform-design.md`）
/// 让用户/安装器能换词库而不动 Program Files。
pub fn candidates(
    env: Option<&OsStr>,
    dll_dir: Option<&Path>,
    local_app_data: Option<&OsStr>,
) -> Vec<Candidate> {
    let mut out = Vec::new();
    // 空串等同未设置（`set VAR=` 是常见清空操作，别当成"当前目录下的空文件名"）。
    if let Some(e) = env.filter(|e| !e.is_empty()) {
        out.push(Candidate {
            path: PathBuf::from(e),
            origin: Origin::Env,
        });
    }
    if let Some(d) = dll_dir {
        out.push(Candidate {
            path: d.join(DICT_FILE_NAME),
            origin: Origin::DllDir,
        });
    }
    if let Some(l) = local_app_data.filter(|l| !l.is_empty()) {
        out.push(Candidate {
            path: PathBuf::from(l).join("opi").join(DICT_FILE_NAME),
            origin: Origin::LocalAppData,
        });
    }
    out
}

/// `OPI_DICT_PATH` 的值；空串或未设置 → `None`（与 XDG 轨"空串等同未设置"一致）。
pub fn env_dict_path() -> Option<OsString> {
    std::env::var_os(DICT_ENV_VAR).filter(|v| !v.is_empty())
}

/// 依次尝试候选并装载。不存在 → 下一个；**存在但坏 → 记一条再下一个**
/// （损坏 ≠ 不存在，见 `notes`）；全失败 → 内置回退词库。**不 panic**。
///
/// 唯一可能的 panic 在 `fallback_dict()` 内部（编译期随包提交的 35 词词库损坏），
/// 与任何用户文件无关 —— 那属于"仓库坏了"，engine-data 已有意如此。
pub fn load_dict(
    env: Option<&OsStr>,
    dll_dir: Option<&Path>,
    local_app_data: Option<&OsStr>,
) -> DictLoad {
    let mut notes = Vec::new();
    for c in candidates(env, dll_dir, local_app_data) {
        match engine_data::load_mmap(&c.path) {
            Ok(d) => {
                return DictLoad {
                    dict: Box::new(d),
                    source: Some(c),
                    notes,
                };
            }
            // 不存在 → 进下一候选，不记（只装了一份词库是正常情况）。
            Err(LoadError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                notes.push(format!(
                    "{}（{}）不存在",
                    c.path.display(),
                    c.origin.describe()
                ));
            }
            // 存在但坏/不可读 → **先留证再进下一候选**。fcitx5 轨踩过的坑正是
            // 「文件在、但坏了 → 引擎完全不出字，无回退无报错」；这里两头都不占：
            // 不静默（原因进 notes → 日志），也不因此让整台输入法没词库（下一候选
            // 或内置回退顶上）。
            Err(e) => {
                notes.push(format!(
                    "{}（{}）不可用: {e:?}",
                    c.path.display(),
                    c.origin.describe()
                ));
            }
        }
    }
    DictLoad {
        dict: Box::new(engine_data::fallback_dict()),
        source: None,
        notes,
    }
}

// 单测独立成文件（`#[path]` 引入）以保持各文件 <500 行：dict_path_tests.rs
#[cfg(test)]
#[path = "dict_path_tests.rs"]
mod tests;
