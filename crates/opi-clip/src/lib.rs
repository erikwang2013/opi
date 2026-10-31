// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! opi-clip：局域网跨端剪贴板同步内核（Phase 1，纯文本）。
//!
//! 设计约束（与各端薄壳的契约）：
//! - 只同步 UTF-8 纯文本，单条上限 256 KiB；不落盘任何剪贴板历史。
//! - 发现走 UDP 组播/广播，beacon 只含协议版本、实例 ID、公钥指纹截断、TCP 端口、
//!   能力位，**绝不包含剪贴板内容**。
//! - 配对走 Noise_XX + 6 位 SAS（双方确认后持久化对端静态公钥）；
//!   会话走 Noise_KK。密码学全部交给 `snow`，本 crate 不手写原语。
//! - 已配对实例间 TCP 长度前缀分帧（单帧 ≤64 KiB），大文本按 48 KiB 分片，
//!   逐事件 ACK/去重/过期/限速/重连。
//! - 防回环：远端来的内容不回传，且最近应用过的内容哈希在本地剪贴板回灌时被抑制。
//! - 冲突：HLC + 实例 ID + 事件 ID 的字典序全序，最后写入者胜；本地新变化在收到
//!   更旧的远端事件时不会被覆盖。
//!
//! 线程模型：`ClipNode::start` 起发现/监听/监督线程，每个 TCP 会话一线程；全部
//! 使用阻塞 socket + 短超时，`stop()` 置位后各线程在数百毫秒内退出并清空内存队列。
//! 公开 API 无 async、无回调、无泛型，ID 为定长字节，便于后续 C ABI/各端薄壳映射：
//! 一个不透明句柄 + `publish_text` / `poll_event` / `peers` / `pair` /
//! `confirm_pairing` / `stop` 即可覆盖 UI 需要。
//!
//! 未做：FFI 出口、各端 UI 接线、真机/跨目标验证、图片或文件类型。

mod config;
mod crypto;
mod discovery;
mod error;
mod frame;
mod identity;
mod net;
mod node;
mod noise;
mod pairing;
mod session;
mod state;
mod trust;
mod types;
mod util;
mod wire;

pub use config::ClipConfig;
pub use error::ClipError;
pub use node::ClipNode;
pub use types::{
    BEACON_INTERVAL, ClipEvent, DEFAULT_DISCOVERY_PORT, DISCOVERY_GROUP, EventId, FRAGMENT_BYTES,
    Fingerprint, InstanceId, MAX_FRAME_BYTES, MAX_TEXT_BYTES, PROTOCOL_VERSION, PeerInfo,
    PublishOutcome, Version,
};
