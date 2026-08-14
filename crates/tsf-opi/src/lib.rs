//! tsf-opi：Windows TSF（Text Services Framework）输入法插件（C1 逻辑层 + C2 TSF 胶水）。
//!
//! 结构镜像 Linux 轨（crates/fcitx5-opi）：纯 Rust 逻辑层 + 平台胶水分离。
//! 逻辑层（`logic`）纯 Rust、无 windows/COM 类型，全平台可编译可单测；
//! 键码约定：可打印字符 = Unicode 码点（与 fcitx5 轨一致），特殊键 = Windows VK 码；
//! TSF 胶水（`tsf`，仅 Windows 目标编译）把 TSF 键事件映射为
//! `logic::TsfLogic::input_key` 的参数，并按 `KeyOutcome` 驱动
//! composition/候选窗/文档插入（骨架，见 `tsf` 模块头注释）。

pub mod logic;

/// Windows 目标专属：TSF COM 胶水（ITfTextInputProcessor / ITfKeyEventSink）。
/// Linux/其他主机不编译本模块（`windows` crate 依赖不进入主机构建路径），
/// 保证 `cargo test --workspace` 在 Linux 上全绿。
#[cfg(target_os = "windows")]
pub mod tsf;

pub use logic::{KeyOutcome, ShiftState, TsfLogic};
