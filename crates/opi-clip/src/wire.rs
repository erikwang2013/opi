// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Noise 传输层内的应用消息。postcard 编码，外层再由 Noise AEAD 保护。

use crate::ClipError;
use crate::types::{EventId, InstanceId, TransferFileInfo, TransferId, Version};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Wire {
    /// 配对握手后的身份声明（加密），必须与 preamble/发现记录一致。
    PairInfo {
        protocol: u8,
        instance_id: InstanceId,
        name: String,
    },
    PairDecision {
        accept: bool,
    },
    /// 剪贴板事件分片；`chunk` 最大 FRAGMENT_BYTES。
    EventFragment {
        event_id: EventId,
        version: Version,
        total: u8,
        index: u8,
        total_len: u32,
        hash: [u8; 32],
        chunk: Vec<u8>,
    },
    Ack {
        event_id: EventId,
    },
    /// 文件清单；接收端用 `TransferDecision` 回答。
    TransferOffer {
        transfer_id: TransferId,
        files: Vec<TransferFileInfo>,
    },
    TransferDecision {
        transfer_id: TransferId,
        accept: bool,
        reason: Option<String>,
    },
    /// 一块文件内容；`offset` 是该文件内的字节偏移。
    TransferChunk {
        transfer_id: TransferId,
        file_index: u16,
        offset: u64,
        bytes: Vec<u8>,
    },
    /// 发送端宣告结束并给出整次传输的 SHA-256；接收端校验后原样回一条作为确认。
    TransferDone {
        transfer_id: TransferId,
        sha256: Option<[u8; 32]>,
    },
    TransferCancel {
        transfer_id: TransferId,
    },
    TransferError {
        transfer_id: TransferId,
        message: String,
    },
    Ping,
    Pong,
}

pub(crate) fn encode(message: &Wire) -> Result<Vec<u8>, ClipError> {
    postcard::to_allocvec(message).map_err(|e| ClipError::Wire(e.to_string()))
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Wire, ClipError> {
    postcard::from_bytes(bytes).map_err(|e| ClipError::Wire(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{EventId, InstanceId, Version};

    fn id(n: u8) -> InstanceId {
        InstanceId::from_bytes([n; 16])
    }

    fn tid(n: u8) -> TransferId {
        TransferId::from_bytes([n; 16])
    }

    fn info(path: &str, size: u64) -> TransferFileInfo {
        TransferFileInfo {
            relative_path: path.into(),
            size,
            mime: "application/octet-stream".into(),
            is_dir: false,
        }
    }

    #[test]
    fn all_messages_roundtrip() {
        let version = Version {
            wall_ms: 7,
            counter: 3,
            instance_id: id(1),
            event_id: EventId::from_bytes([2; 16]),
        };
        let messages = vec![
            Wire::PairInfo {
                protocol: 1,
                instance_id: id(1),
                name: "iPad".into(),
            },
            Wire::PairDecision { accept: true },
            Wire::EventFragment {
                event_id: EventId::from_bytes([2; 16]),
                version,
                total: 1,
                index: 0,
                total_len: 5,
                hash: [3; 32],
                chunk: b"hello".to_vec(),
            },
            Wire::Ack {
                event_id: EventId::from_bytes([4; 16]),
            },
            Wire::TransferOffer {
                transfer_id: tid(5),
                files: vec![info("dir/a.txt", 5), info("dir", 0)],
            },
            Wire::TransferDecision {
                transfer_id: tid(5),
                accept: true,
                reason: None,
            },
            Wire::TransferChunk {
                transfer_id: tid(5),
                file_index: 0,
                offset: 48 * 1024,
                bytes: vec![7u8; 1024],
            },
            Wire::TransferDone {
                transfer_id: tid(5),
                sha256: Some([8u8; 32]),
            },
            Wire::TransferCancel {
                transfer_id: tid(5),
            },
            Wire::TransferError {
                transfer_id: tid(5),
                message: "disk full".into(),
            },
            Wire::Ping,
            Wire::Pong,
        ];
        for message in messages {
            let bytes = encode(&message).unwrap();
            assert_eq!(decode(&bytes).unwrap(), message);
        }
    }

    #[test]
    fn decode_rejects_garbage() {
        assert!(decode(&[0xFF; 3]).is_err());
    }
}
