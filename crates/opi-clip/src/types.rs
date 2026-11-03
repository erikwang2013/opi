// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 公开值类型、协议常量与内部事件表示。

use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const PROTOCOL_VERSION: u8 = 1;
pub const MAX_TEXT_BYTES: usize = 256 * 1024;
/// 外层 TCP 帧长度上限（长度前缀不计）。
pub const MAX_FRAME_BYTES: usize = 64 * 1024;
/// 剪贴板事件分片大小，留出 postcard 头与 Noise tag 的余量。
pub const FRAGMENT_BYTES: usize = 48 * 1024;
pub const DEFAULT_DISCOVERY_PORT: u16 = 45888;
/// 链路本地组播（224.0.0.0/24）。路由器与 AP 必须转发这一段的组播；
/// 2026-11-03 起改用 LocalSend 已在现场验证可达的 224.0.0.167。
pub const DISCOVERY_GROUP: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 167);
/// 旧组播组：只为兼容未升级的客户端，新客户端同时加入并发送。
pub const DISCOVERY_GROUP_LEGACY: Ipv4Addr = Ipv4Addr::new(239, 255, 42, 99);
pub const BEACON_INTERVAL: Duration = Duration::from_secs(2);
pub(crate) const PEER_EXPIRY: Duration = Duration::from_secs(15);
pub(crate) const FRAGMENT_EXPIRY: Duration = Duration::from_secs(30);
pub(crate) const HASH_SUPPRESS_TTL: Duration = Duration::from_secs(60);
pub(crate) const SEEN_EVENT_TTL: Duration = Duration::from_secs(600);
pub(crate) const ACK_TIMEOUT: Duration = Duration::from_secs(3);
pub(crate) const MAX_ACK_ATTEMPTS: u8 = 3;
pub(crate) const PAIRING_TIMEOUT: Duration = Duration::from_secs(60);
pub(crate) const MAX_REASSEMBLY_EVENTS: usize = 8;
pub(crate) const MAX_REASSEMBLY_BYTES: usize = 4 * MAX_TEXT_BYTES;
pub(crate) const CAP_TEXT: u8 = 1 << 0;
pub(crate) const CAP_PAIRING: u8 = 1 << 1;
/// 支持文件/文件夹传输（`TransferOffer` 一族 Wire 消息）。
pub(crate) const CAP_TRANSFER: u8 = 1 << 2;
/// 对方愿意在发现后自动配对（TOFU）；不置位的老客户端仍走手动 SAS。
pub(crate) const CAP_AUTO_PAIR: u8 = 1 << 3;
/// 探测报文：请求收到者立刻单播回一个普通 beacon。只在广播/组播被丢时用。
pub(crate) const CAP_PROBE: u8 = 1 << 7;

// ---------- 文件传输上限（对外公开，宿主据此禁用超限选择） ----------
/// 单次传输的条目上限（文件 + 目录项）。offer 必须能放进一帧 64 KiB。
pub const MAX_TRANSFER_FILES: usize = 128;
/// 相对路径的 UTF-8 字节上限。
pub const MAX_TRANSFER_PATH_BYTES: usize = 256;
/// 单文件字节上限。
pub const MAX_TRANSFER_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// 单次传输总字节上限。
pub const MAX_TRANSFER_TOTAL_BYTES: u64 = 8 * 1024 * 1024 * 1024;
/// 全局同时进行的传输数（收 + 发）。
pub const MAX_CONCURRENT_TRANSFERS: usize = 4;
/// 文件块大小，留出 postcard 头与 Noise tag 的余量。
pub const TRANSFER_CHUNK_BYTES: usize = 48 * 1024;
/// 等待对方接受 / 等待最终 Done 确认的空闲上限。
pub(crate) const TRANSFER_TIMEOUT: Duration = Duration::from_secs(60);

/// 设备实例 ID：随机 16 字节，首次启动生成并随身份文件持久化。
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstanceId([u8; 16]);

impl InstanceId {
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(text: &str) -> Result<Self, crate::ClipError> {
        let bytes = hex::decode(text).map_err(|e| crate::ClipError::Invalid(e.to_string()))?;
        let bytes: [u8; 16] = bytes
            .try_into()
            .map_err(|_| crate::ClipError::Invalid("instance id must be 16 bytes".into()))?;
        Ok(Self(bytes))
    }
}

impl fmt::Debug for InstanceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Display for InstanceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// 剪贴板事件 ID：由实例 ID、HLC、内容哈希确定性派生。
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventId([u8; 16]);

impl EventId {
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Debug for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// 静态公钥 SHA-256 的前 8 字节，仅用于发现阶段的弱标识，不作信任依据。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Fingerprint([u8; 8]);

impl Fingerprint {
    pub fn from_bytes(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 8] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// 一次文件传输的随机 16 字节 ID（收发双方各自的路由键）。
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TransferId([u8; 16]);

impl TransferId {
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(text: &str) -> Result<Self, crate::ClipError> {
        let bytes = hex::decode(text).map_err(|e| crate::ClipError::Invalid(e.to_string()))?;
        let bytes: [u8; 16] = bytes
            .try_into()
            .map_err(|_| crate::ClipError::Invalid("transfer id must be 16 bytes".into()))?;
        Ok(Self(bytes))
    }
}

impl fmt::Debug for TransferId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Display for TransferId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// offer / 事件里的文件条目（不含宿主本地路径）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferFileInfo {
    /// 相对路径，`/` 分隔；不含 `..`、绝对路径、空组件。
    pub relative_path: String,
    pub size: u64,
    pub mime: String,
    pub is_dir: bool,
}

/// 待发送条目：本地源路径 + 已校验的元数据。由 [`crate::transfer`] 的构造器生成。
#[derive(Clone, Debug)]
pub struct TransferFile {
    pub(crate) source: PathBuf,
    pub(crate) info: TransferFileInfo,
}

/// Hybrid Logical Clock 的物理/逻辑分量。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub(crate) struct Hlc {
    pub wall_ms: u64,
    pub counter: u32,
}

impl Hlc {
    pub(crate) fn zero() -> Self {
        Self {
            wall_ms: 0,
            counter: 0,
        }
    }

    pub(crate) fn tick(&mut self, now_ms: u64) -> Self {
        if now_ms > self.wall_ms {
            self.wall_ms = now_ms;
            self.counter = 0;
        } else {
            self.counter = self.counter.saturating_add(1);
        }
        *self
    }

    pub(crate) fn merge(&mut self, remote: Self) {
        let wall_ms = self.wall_ms.max(remote.wall_ms);
        let counter = match (self.wall_ms == wall_ms, remote.wall_ms == wall_ms) {
            (true, true) => self.counter.max(remote.counter).saturating_add(1),
            (true, false) => self.counter.saturating_add(1),
            (false, true) => remote.counter.saturating_add(1),
            (false, false) => 0,
        };
        self.wall_ms = wall_ms;
        self.counter = counter;
    }
}

/// 剪贴板状态的全序版本：HLC → 实例 ID → 事件 ID。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct Version {
    pub wall_ms: u64,
    pub counter: u32,
    pub instance_id: InstanceId,
    pub event_id: EventId,
}

impl Version {
    pub(crate) fn new(hlc: Hlc, instance_id: InstanceId, event_id: EventId) -> Self {
        Self {
            wall_ms: hlc.wall_ms,
            counter: hlc.counter,
            instance_id,
            event_id,
        }
    }

    pub(crate) fn hlc(self) -> Hlc {
        Hlc {
            wall_ms: self.wall_ms,
            counter: self.counter,
        }
    }
}

/// 发现到的对端快照（地址/信任/连接状态随时可查）。
#[derive(Clone, Debug)]
pub struct PeerInfo {
    pub instance_id: InstanceId,
    pub name: String,
    pub address: SocketAddr,
    pub fingerprint: Fingerprint,
    pub capabilities: u8,
    pub trusted: bool,
    pub connected: bool,
}

/// 内核向上层投递的通知；上层通过 [`crate::ClipNode::poll_event`] 拉取。
#[derive(Clone, Debug)]
pub enum ClipEvent {
    PeerDiscovered(PeerInfo),
    PeerOffline {
        instance_id: InstanceId,
    },
    /// 双方算出同一 6 位 SAS；`incoming` 表示这次配对由对端发起。
    PairingPrompt {
        instance_id: InstanceId,
        sas: String,
        incoming: bool,
    },
    PairingFinished {
        instance_id: InstanceId,
        trusted: bool,
        reason: Option<String>,
    },
    /// 收到并接受了一条更新的远端剪贴板；上层应写入系统剪贴板。
    ClipboardReceived {
        instance_id: InstanceId,
        text: String,
        version: Version,
    },
    SendFailed {
        instance_id: InstanceId,
        event_id: EventId,
    },
    /// 收到对方的文件清单，等待宿主调用 [`crate::ClipNode::respond_transfer`]。
    /// `outgoing=true` 表示本端发起、正在等对方确认。
    TransferOffer {
        instance_id: InstanceId,
        transfer_id: TransferId,
        files: Vec<TransferFileInfo>,
        total_bytes: u64,
        outgoing: bool,
    },
    /// 已传字节进度（发送/接收两侧都会发，按 1 MiB 或 500 ms 节流）。
    TransferProgress {
        instance_id: InstanceId,
        transfer_id: TransferId,
        transferred_bytes: u64,
        total_bytes: u64,
        file_index: u16,
        file_count: u16,
        outgoing: bool,
    },
    /// 完成；接收侧的 `saved_dir` 是宿主给出的落盘目录。
    TransferCompleted {
        instance_id: InstanceId,
        transfer_id: TransferId,
        files: Vec<TransferFileInfo>,
        total_bytes: u64,
        outgoing: bool,
        saved_dir: Option<PathBuf>,
    },
    TransferFailed {
        instance_id: InstanceId,
        transfer_id: TransferId,
        reason: String,
        outgoing: bool,
    },
    TransferCanceled {
        instance_id: InstanceId,
        transfer_id: TransferId,
        outgoing: bool,
    },
    Error {
        message: String,
    },
}

/// 本地发布结果。`SuppressedLoop` 表示内容刚由远端写入，避免回传。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublishOutcome {
    Published {
        event_id: EventId,
        version: Version,
        peers: usize,
    },
    SuppressedLoop,
}

#[derive(Clone)]
pub(crate) struct StoredEvent {
    pub event_id: EventId,
    pub version: Version,
    pub text: String,
    pub hash: [u8; 32],
}

pub(crate) enum Outbound {
    Publish(StoredEvent),
    SendTransfer {
        id: TransferId,
        files: Vec<TransferFile>,
    },
    RespondTransfer {
        id: TransferId,
        accept: bool,
        save_dir: Option<PathBuf>,
    },
    CancelTransfer {
        id: TransferId,
    },
}

pub(crate) struct SessionEntry {
    pub tx: mpsc::SyncSender<Outbound>,
    pub generation: u64,
    pub addr: SocketAddr,
}

/// 内核里的传输路由：把 transfer id 映射回对端会话，同时用于并发上限。
pub(crate) struct TransferRoute {
    pub peer: InstanceId,
    pub outgoing: bool,
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

pub(crate) fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
