// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `Core` 的确定性单元测试：LWW/回环抑制/重放/分片边界。

use super::*;
use crate::config::temp_dir;
use crate::crypto::event_id;
use crate::types::{Hlc, Version, now_ms};
use std::path::PathBuf;
use std::time::Instant;

fn sender(n: u8) -> InstanceId {
    InstanceId::from_bytes([n; 16])
}

fn core() -> (Core, PathBuf) {
    let dir = temp_dir("state");
    let path = dir.join("trust.json");
    let core = Core::new(sender(1), vec![1u8; 32], TrustStore::default(), path);
    (core, dir)
}

fn event(from: InstanceId, wall_ms: u64, counter: u32, text: &str) -> StoredEvent {
    let hash = content_hash(text);
    let hlc = Hlc { wall_ms, counter };
    let id = event_id(from, hlc, &hash);
    StoredEvent {
        event_id: id,
        version: Version::new(hlc, from, id),
        text: text.to_owned(),
        hash,
    }
}

fn fragments(from: InstanceId, wall_ms: u64, text: &str) -> Vec<Fragment> {
    let hash = content_hash(text);
    let hlc = Hlc {
        wall_ms,
        counter: 0,
    };
    let id = event_id(from, hlc, &hash);
    let version = Version::new(hlc, from, id);
    let bytes = text.as_bytes();
    let total = bytes.len().div_ceil(FRAGMENT_BYTES).max(1);
    (0..total)
        .map(|index| {
            let start = index * FRAGMENT_BYTES;
            let end = (start + FRAGMENT_BYTES).min(bytes.len());
            Fragment {
                event_id: id,
                version,
                total: total as u8,
                index: index as u8,
                total_len: bytes.len() as u32,
                hash,
                chunk: bytes[start..end].to_vec(),
            }
        })
        .collect()
}

#[test]
fn lww_and_pending_local() {
    let (mut core, dir) = core();
    let local = core.prepare_local("local").unwrap().unwrap();
    let older = event(
        sender(2),
        local.version.wall_ms.saturating_sub(10),
        0,
        "older",
    );
    assert_eq!(
        core.apply_event(sender(2), &older, Instant::now()),
        Apply::Stale
    );
    assert!(
        core.pending_local.is_some(),
        "newer local change must stay pending"
    );

    let newer = event(sender(2), local.version.wall_ms + 10, 0, "newer");
    assert_eq!(
        core.apply_event(sender(2), &newer, Instant::now()),
        Apply::Applied
    );
    assert!(core.pending_local.is_none());
    assert_eq!(core.current, Some(newer.version));

    let spoofed = event(sender(3), newer.version.wall_ms + 10, 0, "spoof");
    assert_eq!(
        core.apply_event(sender(2), &spoofed, Instant::now()),
        Apply::Invalid
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn replay_is_deduplicated() {
    let (mut core, dir) = core();
    let remote = event(sender(2), now_ms() + 5, 0, "hello");
    assert_eq!(
        core.apply_event(sender(2), &remote, Instant::now()),
        Apply::Applied
    );
    assert_eq!(
        core.apply_event(sender(2), &remote, Instant::now()),
        Apply::Duplicate
    );
    assert_eq!(
        core.events
            .iter()
            .filter(|e| matches!(e, ClipEvent::ClipboardReceived { .. }))
            .count(),
        1
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn applied_remote_content_is_suppressed_on_reentry() {
    let (mut core, dir) = core();
    let remote = event(sender(2), now_ms() + 5, 0, "remote text");
    assert_eq!(
        core.apply_event(sender(2), &remote, Instant::now()),
        Apply::Applied
    );
    assert!(core.prepare_local("remote text").unwrap().is_none());
    assert!(core.prepare_local("typed text").unwrap().is_some());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn oversize_local_text_is_rejected() {
    let (mut core, dir) = core();
    let text = "x".repeat(MAX_TEXT_BYTES + 1);
    assert!(matches!(
        core.prepare_local(&text),
        Err(ClipError::TooLarge(_))
    ));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn fragments_reassemble_out_of_order_and_replay_as_duplicate() {
    let (mut core, dir) = core();
    let text = "x".repeat(FRAGMENT_BYTES * 2 + 100);
    let wall = now_ms() + 5;
    let mut parts = fragments(sender(2), wall, &text);
    assert_eq!(parts.len(), 3);
    let last = parts.pop().unwrap();
    let first = parts.remove(0);
    assert_eq!(
        core.accept_fragment(sender(2), last, Instant::now()),
        FragmentOutcome::Pending
    );
    assert_eq!(
        core.accept_fragment(sender(2), first, Instant::now()),
        FragmentOutcome::Pending
    );
    let middle = parts.remove(0);
    assert_eq!(
        core.accept_fragment(sender(2), middle, Instant::now()),
        FragmentOutcome::Done(Apply::Applied)
    );
    let replay = fragments(sender(2), wall, &text);
    assert_eq!(
        core.accept_fragment(
            sender(2),
            replay.into_iter().next().unwrap(),
            Instant::now()
        ),
        FragmentOutcome::Done(Apply::Duplicate)
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn conflicting_chunk_discards_reassembly() {
    let (mut core, dir) = core();
    let text = "y".repeat(FRAGMENT_BYTES + 10);
    let wall = now_ms() + 5;
    let mut parts = fragments(sender(2), wall, &text);
    let first = parts.remove(0);
    let mut conflict = Fragment {
        event_id: first.event_id,
        version: first.version,
        total: first.total,
        index: first.index,
        total_len: first.total_len,
        hash: first.hash,
        chunk: first.chunk.clone(),
    };
    conflict.chunk[0] ^= 0xFF;
    assert_eq!(
        core.accept_fragment(sender(2), first, Instant::now()),
        FragmentOutcome::Pending
    );
    assert_eq!(
        core.accept_fragment(sender(2), conflict, Instant::now()),
        FragmentOutcome::Bad
    );
    assert_eq!(core.fragments.len(), 0, "bad reassembly is discarded");

    let mut retry = fragments(sender(2), wall, &text);
    assert_eq!(
        core.accept_fragment(sender(2), retry.remove(0), Instant::now()),
        FragmentOutcome::Pending
    );
    assert_eq!(
        core.accept_fragment(sender(2), retry.remove(0), Instant::now()),
        FragmentOutcome::Done(Apply::Applied)
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn fragment_limits_and_corrupt_hash_are_rejected() {
    let (mut core, dir) = core();
    let mut oversized = fragments(sender(2), now_ms() + 5, "small").remove(0);
    oversized.total_len = (MAX_TEXT_BYTES + 1) as u32;
    assert_eq!(
        core.accept_fragment(sender(2), oversized, Instant::now()),
        FragmentOutcome::Bad
    );

    let hlc = Hlc {
        wall_ms: now_ms() + 6,
        counter: 0,
    };
    let hash = [9u8; 32];
    let id = event_id(sender(2), hlc, &hash);
    let corrupt = Fragment {
        event_id: id,
        version: Version::new(hlc, sender(2), id),
        total: 1,
        index: 0,
        total_len: 4,
        hash,
        chunk: b"nope".to_vec(),
    };
    assert_eq!(
        core.accept_fragment(sender(2), corrupt, Instant::now()),
        FragmentOutcome::Bad
    );

    let text = "z".repeat(FRAGMENT_BYTES + 1);
    for index in 0..=MAX_REASSEMBLY_EVENTS {
        // 只送第一片，事件不会完成，才能占住一个未完成的重组槽位。
        let part = fragments(sender(2), now_ms() + 100 + index as u64, &text).remove(0);
        let outcome = core.accept_fragment(sender(2), part, Instant::now());
        if index < MAX_REASSEMBLY_EVENTS {
            assert_eq!(outcome, FragmentOutcome::Pending);
        } else {
            assert_eq!(outcome, FragmentOutcome::Bad, "reassembly count cap");
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}
