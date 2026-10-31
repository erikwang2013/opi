// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 公开值类型、协议常量与内部事件表示。

use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const PROTOCOL_VERSION: u8 = 1;
pub const MAX_TEXT_BYTES: usize = 256 * 1024;
/// 外层 TCP 帧长度上限（长度前缀不计）。
pub const MAX_FRAME_BYTES: usize = 64 * 1024;
/// 剪贴板事件分片大小，留出 postcard 头与 Noise tag 的余量。
pub const FRAGMENT_BYTES: usize = 48 * 1024;
pub const DEFAULT_DISCOVERY_PORT: u16 = 45888;
pub const DISCOVERY_GROUP: Ipv4Addr = Ipv4Addr::new(239, 255, 42, 99);
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
}

pub(crate) struct SessionEntry {
    pub tx: mpsc::SyncSender<Outbound>,
    pub generation: u64,
    pub addr: SocketAddr,
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
