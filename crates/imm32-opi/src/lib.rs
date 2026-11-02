// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! IMM32 兼容输入法（Windows `.ime`）。
//!
//! TSF 与 IMM32 是 Windows 的两条输入通路。TSF 宿主（现代应用）走 `tsf_opi`；
//! 旧式 IMM32 宿主（部分 PowerShell/Chrome/控制台路径）只认 `Ime*` 导出。本 crate
//! 复用 `tsf_opi::TsfLogic` 的键路由，只新增 IMM32 的胶水：
//! - `session.rs`：平台中立状态机（键 → 组合串/候选/提交动作），Linux 可单测；
//! - `win.rs`：HIMC/HWND、`WM_IME_*`、候选窗、`.ime` 导出（仅 Windows 编译）。

pub mod session;

// 词库共享与 Host 表是平台中立的，但生产上只被 Windows 胶水用；Linux 测试构建
// 也编进来（多 HIMC 隔离在主机上验证）。
#[cfg(any(target_os = "windows", test))]
mod dict;
#[cfg(any(target_os = "windows", test))]
mod hosts;

#[cfg(target_os = "windows")]
mod win;

#[cfg(test)]
#[path = "session_tests.rs"]
mod session_tests;
