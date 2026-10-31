// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 非密码学原语的哈希组合：内容哈希、事件 ID、指纹、SAS。
//! DH/AEAD/握手全部由 `snow` 完成；这里只做确定性的 SHA-256 派生。

use crate::types::{EventId, Fingerprint, Hlc, InstanceId};
use sha2::{Digest, Sha256};

pub(crate) fn content_hash(text: &str) -> [u8; 32] {
    content_hash_bytes(text.as_bytes())
}

pub(crate) fn content_hash_bytes(bytes: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

pub(crate) fn fingerprint(public_key: &[u8]) -> Fingerprint {
    let digest = Sha256::digest(public_key);
    let mut out = [0u8; 8];
    out.copy_from_slice(&digest[..8]);
    Fingerprint::from_bytes(out)
}

/// 事件 ID 由 (实例, HLC, 内容哈希) 确定性派生：同一事件重放时 ID 不变，便于去重。
pub(crate) fn event_id(instance: InstanceId, hlc: Hlc, hash: &[u8; 32]) -> EventId {
    let mut h = Sha256::new();
    h.update(b"opi-clip/event/v1");
    h.update(instance.as_bytes());
    h.update(hlc.wall_ms.to_be_bytes());
    h.update(hlc.counter.to_be_bytes());
    h.update(hash);
    let digest = h.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    EventId::from_bytes(out)
}

/// 双方在 Noise 握手完成后得到同一 handshake hash，据此显示 6 位 SAS。
/// 20 bit 只用于人工比对，真正的身份绑定由 Noise 静态公钥完成。
pub(crate) fn sas(prologue: &[u8], handshake_hash: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b"opi-clip/sas/v1");
    h.update((prologue.len() as u64).to_be_bytes());
    h.update(prologue);
    h.update(handshake_hash);
    let digest = h.finalize();
    let mut head = [0u8; 8];
    head.copy_from_slice(&digest[..8]);
    format!("{:06}", u64::from_be_bytes(head) % 1_000_000)
}

/// 握手 prologue 绑定协议、模式与双方实例 ID，任何一头被改都会导致握手失败。
pub(crate) fn prologue(mode: u8, initiator: InstanceId, responder: InstanceId) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + 3 + 32);
    out.extend_from_slice(b"opi-clip");
    out.push(1);
    out.push(mode);
    out.extend_from_slice(initiator.as_bytes());
    out.extend_from_slice(responder.as_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u8) -> InstanceId {
        InstanceId::from_bytes([n; 16])
    }

    #[test]
    fn sas_is_six_digits_and_symmetric() {
        let p = prologue(2, id(1), id(2));
        let a = sas(&p, b"handshake hash");
        let b = sas(&p, b"handshake hash");
        assert_eq!(a, b);
        assert_eq!(a.len(), 6);
        assert!(a.bytes().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn sas_changes_with_transcript() {
        let p = prologue(2, id(1), id(2));
        assert_ne!(sas(&p, b"one"), sas(&p, b"two"));
        assert_ne!(sas(&p, b"x"), sas(&prologue(2, id(2), id(1)), b"x"));
    }

    #[test]
    fn event_id_is_stable() {
        let h = content_hash("hello");
        let hlc = Hlc {
            wall_ms: 10,
            counter: 2,
        };
        assert_eq!(event_id(id(3), hlc, &h), event_id(id(3), hlc, &h));
        assert_ne!(event_id(id(3), hlc, &h), event_id(id(4), hlc, &h));
        assert_ne!(
            event_id(id(3), hlc, &h),
            event_id(id(3), hlc, &content_hash("other"))
        );
    }

    #[test]
    fn fingerprint_is_eight_bytes() {
        assert_eq!(fingerprint(&[7u8; 32]).as_bytes().len(), 8);
        assert_ne!(fingerprint(&[7u8; 32]), fingerprint(&[8u8; 32]));
    }
}
