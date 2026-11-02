// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 事件/对端 → JSON（`cabi_clip.rs` 的**非导出**辅助，拆出只为 500 行门禁）。
//! 真源是 `opi_clip::ClipEvent`；`type` 是判别键，字段名与 Kotlin/Swift 侧约定一致。

use opi_clip::{ClipEvent, PeerInfo, TransferFileInfo, Version};
use serde_json::{Value, json};

/// 单条事件 JSON 的输出上限。
const MAX_EVENT_JSON_BYTES: usize = 1024 * 1024;

pub(super) fn to_json(value: &Value) -> Result<String, String> {
    serde_json::to_string(value).map_err(|e| format!("JSON encode: {e}"))
}

pub(super) fn peer_json(peer: &PeerInfo) -> Value {
    json!({
        "instance_id": peer.instance_id.to_hex(),
        "name": peer.name,
        "address": peer.address.to_string(),
        "fingerprint": peer.fingerprint.to_hex(),
        "capabilities": peer.capabilities,
        "trusted": peer.trusted,
        "connected": peer.connected,
    })
}

fn version_json(version: Version) -> Value {
    json!({
        "wall_ms": version.wall_ms,
        "counter": version.counter,
        "instance_id": version.instance_id.to_hex(),
        "event_id": version.event_id.to_hex(),
    })
}

fn files_json(files: &[TransferFileInfo]) -> Value {
    Value::Array(
        files
            .iter()
            .map(|file| {
                json!({
                    "relative_path": file.relative_path,
                    "size": file.size,
                    "mime": file.mime,
                    "is_dir": file.is_dir,
                })
            })
            .collect(),
    )
}

/// 事件 → JSON；文本由 serde_json 转义。超过 1 MiB 直接报错（事件由调用方丢弃）。
pub(super) fn encode_event(event: ClipEvent) -> Result<String, String> {
    let value = match event {
        ClipEvent::PeerDiscovered(peer) => {
            json!({"type": "peer_discovered", "peer": peer_json(&peer)})
        }
        ClipEvent::PeerOffline { instance_id } => {
            json!({"type": "peer_offline", "instance_id": instance_id.to_hex()})
        }
        ClipEvent::PairingPrompt {
            instance_id,
            sas,
            incoming,
        } => json!({
            "type": "pairing_prompt",
            "instance_id": instance_id.to_hex(),
            "sas": sas,
            "incoming": incoming,
        }),
        ClipEvent::PairingFinished {
            instance_id,
            trusted,
            reason,
        } => json!({
            "type": "pairing_finished",
            "instance_id": instance_id.to_hex(),
            "trusted": trusted,
            "reason": reason,
        }),
        ClipEvent::ClipboardReceived {
            instance_id,
            text,
            version,
        } => json!({
            "type": "clipboard_received",
            "instance_id": instance_id.to_hex(),
            "text": text,
            "version": version_json(version),
        }),
        ClipEvent::SendFailed {
            instance_id,
            event_id,
        } => json!({
            "type": "send_failed",
            "instance_id": instance_id.to_hex(),
            "event_id": event_id.to_hex(),
        }),
        ClipEvent::TransferOffer {
            instance_id,
            transfer_id,
            files,
            total_bytes,
            outgoing,
        } => json!({
            "type": "transfer_offer",
            "instance_id": instance_id.to_hex(),
            "transfer_id": transfer_id.to_hex(),
            "files": files_json(&files),
            "total_bytes": total_bytes,
            "outgoing": outgoing,
        }),
        ClipEvent::TransferProgress {
            instance_id,
            transfer_id,
            transferred_bytes,
            total_bytes,
            file_index,
            file_count,
            outgoing,
        } => json!({
            "type": "transfer_progress",
            "instance_id": instance_id.to_hex(),
            "transfer_id": transfer_id.to_hex(),
            "transferred_bytes": transferred_bytes,
            "total_bytes": total_bytes,
            "file_index": file_index,
            "file_count": file_count,
            "outgoing": outgoing,
        }),
        ClipEvent::TransferCompleted {
            instance_id,
            transfer_id,
            files,
            total_bytes,
            outgoing,
            saved_dir,
        } => json!({
            "type": "transfer_completed",
            "instance_id": instance_id.to_hex(),
            "transfer_id": transfer_id.to_hex(),
            "files": files_json(&files),
            "total_bytes": total_bytes,
            "outgoing": outgoing,
            "saved_dir": saved_dir.map(|dir| dir.to_string_lossy().into_owned()),
        }),
        ClipEvent::TransferFailed {
            instance_id,
            transfer_id,
            reason,
            outgoing,
        } => json!({
            "type": "transfer_failed",
            "instance_id": instance_id.to_hex(),
            "transfer_id": transfer_id.to_hex(),
            "reason": reason,
            "outgoing": outgoing,
        }),
        ClipEvent::TransferCanceled {
            instance_id,
            transfer_id,
            outgoing,
        } => json!({
            "type": "transfer_canceled",
            "instance_id": instance_id.to_hex(),
            "transfer_id": transfer_id.to_hex(),
            "outgoing": outgoing,
        }),
        ClipEvent::Error { message } => json!({"type": "error", "message": message}),
    };
    let text = to_json(&value)?;
    if text.len() > MAX_EVENT_JSON_BYTES {
        return Err(format!(
            "clip event JSON is {} bytes; limit is {MAX_EVENT_JSON_BYTES} (event dropped)",
            text.len()
        ));
    }
    Ok(text)
}
