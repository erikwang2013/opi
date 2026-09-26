// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! tsf-opi：Windows TSF（Text Services Framework）输入法插件（C1 逻辑层 + C2 TSF 胶水）。
//!
//! 结构镜像 Linux 轨（crates/fcitx5-opi）：纯 Rust 逻辑层 + 平台胶水分离。
//! 逻辑层（`logic`）纯 Rust、无 windows/COM 类型，全平台可编译可单测；
//! 键码约定：可打印字符 = Unicode 码点（与 fcitx5 轨一致），特殊键 = Windows VK 码；
//! TSF 胶水（`tsf`，仅 Windows 目标编译）把 TSF 键事件映射为
//! `logic::TsfLogic::input_key` 的参数，并按 `KeyOutcome` 驱动
//! composition/候选窗/文档插入（骨架，见 `tsf` 模块头注释）。

pub mod logic;

/// DLL 服务器资料（CLSID/GUID · 注册表键路径 · 模块级对象计数）。
/// 与 `vk.rs` 同样是**平台中立**的：`windows_core::GUID` 是无条件依赖，
/// 主机可构造、可格式化、可单测 —— 而这些值错了的后果全是静默的（见 `dll.rs`）。
pub mod dll;

/// Windows 目标专属：TSF COM 胶水（ITfTextInputProcessor / ITfKeyEventSink）。
/// Linux/其他主机不编译本模块（`windows` crate 依赖不进入主机构建路径），
/// 保证 `cargo test --workspace` 在 Linux 上全绿。
#[cfg(target_os = "windows")]
pub mod tsf;

/// Windows 目标专属：COM 服务器面（类工厂 + `Dll*` 导出 + regsvr32 注册）。
/// 与 `tsf.rs` 同样的 cfg 门：`windows` 依赖不进 Linux 主机构建路径。
/// **注册效果与"能否在 Word 里打字"本机（Linux）无法验证**，见模块头注释。
#[cfg(target_os = "windows")]
pub mod com_server;

/// Win32 VK → 引擎键码的映射判定（纯函数，无 windows 类型 → 主机可编译可单测）。
/// `tsf.rs` 是 `#[cfg(target_os = "windows")]`：判定若留在那边，Linux 门禁就永远
/// 覆盖不到（本项目的开发/验收主机是 Linux）。
pub mod vk;

/// Windows 目标专属：候选窗通信（C3，named pipe 客户端 + TsfSink 生产实现）。
/// 与 tsf.rs 同样的双重隔离（cfg 门 + windows 依赖 target 作用域），
/// 主机（Linux）构建/测试不受影响。
#[cfg(target_os = "windows")]
pub mod candidate_io;

pub use logic::{KeyOutcome, ShiftState, TsfLogic};
