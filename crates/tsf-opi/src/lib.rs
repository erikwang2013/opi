//! tsf-opi：Windows TSF（Text Services Framework）输入法插件的逻辑层（C1）。
//!
//! 结构镜像 Linux 轨（crates/fcitx5-opi）：纯 Rust 逻辑层 + 平台胶水分离。
//! 本 crate 为纯逻辑（无 windows/COM 类型），键码约定：
//! 可打印字符 = Unicode 码点（与 fcitx5 轨一致），特殊键 = Windows VK 码；
//! C2 的 TSF 胶水负责把 TSF 键事件映射为 `logic::TsfLogic::input_key` 的参数，
//! 并按 `KeyOutcome` 驱动 composition/候选窗/文档插入。

pub mod logic;

pub use logic::{KeyOutcome, ShiftState, TsfLogic};
