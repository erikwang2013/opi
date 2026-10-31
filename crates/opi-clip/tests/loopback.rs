// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 同机双实例端到端：发现、SAS 配对、传输（含 256 KiB 上限）、防回环、
//! LWW 冲突、拒绝配对、停止清空。每个测试使用独立发现端口，可并行运行。

use opi_clip::{
    ClipConfig, ClipError, ClipEvent, ClipNode, MAX_TEXT_BYTES, PublishOutcome, Version,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU16, AtomicU32, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(10);

static NEXT_PORT: AtomicU16 = AtomicU16::new(46810);
static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

fn unique_port() -> u16 {
    NEXT_PORT.fetch_add(1, Ordering::Relaxed)
}

fn temp_dir(tag: &str) -> PathBuf {
    let n = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("opi-clip-it-{tag}-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn start(tag: &str, discovery_port: u16) -> (ClipNode, PathBuf) {
    let dir = temp_dir(tag);
    let mut config = ClipConfig::new(&dir);
    config.discovery_port = discovery_port;
    config.device_name = tag.to_owned();
    let node = ClipNode::start(config).unwrap();
    (node, dir)
}

struct Log(Vec<ClipEvent>);

impl Log {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn wait(
        &mut self,
        node: &ClipNode,
        timeout: Duration,
        predicate: impl Fn(&ClipEvent) -> bool,
    ) -> ClipEvent {
        let deadline = Instant::now() + timeout;
        loop {
            while let Some(event) = node.poll_event() {
                let hit = predicate(&event);
                self.0.push(event.clone());
                if hit {
                    return event;
                }
            }
            assert!(
                Instant::now() < deadline,
                "timed out; saw {} events",
                self.0.len()
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn quiet(
        &mut self,
        node: &ClipNode,
        duration: Duration,
        predicate: impl Fn(&ClipEvent) -> bool,
    ) -> CowLog {
        let deadline = Instant::now() + duration;
        let mut unexpected = 0;
        loop {
            while let Some(event) = node.poll_event() {
                if predicate(&event) {
                    unexpected += 1;
                }
                self.0.push(event);
            }
            if Instant::now() >= deadline {
                return CowLog(unexpected);
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

struct CowLog(usize);

impl CowLog {
    fn none(self) {
        assert_eq!(self.0, 0, "unexpected matching event");
    }
}

fn wait_until(timeout: Duration, mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + timeout;
    while !check() {
        assert!(Instant::now() < deadline, "condition timed out");
        thread::sleep(Duration::from_millis(20));
    }
}

fn discover(a: &ClipNode, b: &ClipNode, log_a: &mut Log, log_b: &mut Log) {
    let a_id = a.local_instance_id();
    let b_id = b.local_instance_id();
    log_a.wait(
        a,
        WAIT,
        |event| matches!(event, ClipEvent::PeerDiscovered(peer) if peer.instance_id == b_id),
    );
    log_b.wait(
        b,
        WAIT,
        |event| matches!(event, ClipEvent::PeerDiscovered(peer) if peer.instance_id == a_id),
    );
}

fn pair_nodes(a: &ClipNode, b: &ClipNode, log_a: &mut Log, log_b: &mut Log) {
    let a_id = a.local_instance_id();
    let b_id = b.local_instance_id();
    a.pair(b_id).unwrap();

    let prompt_a = log_a.wait(a, WAIT, |event| {
        matches!(event, ClipEvent::PairingPrompt { instance_id, .. } if *instance_id == b_id)
    });
    let prompt_b = log_b.wait(b, WAIT, |event| {
        matches!(event, ClipEvent::PairingPrompt { instance_id, .. } if *instance_id == a_id)
    });
    let sas = |event: ClipEvent| match event {
        ClipEvent::PairingPrompt { sas, .. } => sas,
        other => panic!("expected pairing prompt, got {other:?}"),
    };
    assert_eq!(
        sas(prompt_a),
        sas(prompt_b),
        "both sides must show the same SAS"
    );

    a.confirm_pairing(b_id, true).unwrap();
    b.confirm_pairing(a_id, true).unwrap();
    log_a.wait(a, WAIT, |event| {
        matches!(event, ClipEvent::PairingFinished { instance_id, trusted: true, .. }
            if *instance_id == b_id)
    });
    log_b.wait(b, WAIT, |event| {
        matches!(event, ClipEvent::PairingFinished { instance_id, trusted: true, .. }
            if *instance_id == a_id)
    });
    wait_until(WAIT, || {
        a.peers()
            .iter()
            .any(|peer| peer.instance_id == b_id && peer.connected)
            && b.peers()
                .iter()
                .any(|peer| peer.instance_id == a_id && peer.connected)
    });
}

fn published_version(outcome: PublishOutcome) -> Version {
    match outcome {
        PublishOutcome::Published { version, .. } => version,
        PublishOutcome::SuppressedLoop => panic!("unexpected loop suppression"),
    }
}

#[test]
fn pair_transfer_max_text_loop_suppression_and_stop() {
    let port = unique_port();
    let (a, dir_a) = start("a", port);
    let (b, dir_b) = start("b", port);
    let a_id = a.local_instance_id();
    let mut log_a = Log::new();
    let mut log_b = Log::new();

    discover(&a, &b, &mut log_a, &mut log_b);
    pair_nodes(&a, &b, &mut log_a, &mut log_b);

    let outcome = a.publish_text("hello from a").unwrap();
    let PublishOutcome::Published { peers, version, .. } = outcome else {
        panic!("local text was suppressed");
    };
    assert_eq!(peers, 1);
    log_b.wait(&b, WAIT, |event| {
        matches!(event, ClipEvent::ClipboardReceived { instance_id, text, version: received }
            if *instance_id == a_id && text == "hello from a" && *received == version)
    });

    // 远端写入后系统剪贴板的回灌不能再发给对端。
    assert_eq!(
        b.publish_text("hello from a").unwrap(),
        PublishOutcome::SuppressedLoop
    );
    log_a
        .quiet(&a, Duration::from_millis(500), |event| {
            matches!(event, ClipEvent::ClipboardReceived { .. })
        })
        .none();

    let big = "x".repeat(MAX_TEXT_BYTES);
    assert!(matches!(
        a.publish_text(&big),
        Ok(PublishOutcome::Published { .. })
    ));
    let event = log_b.wait(&b, WAIT, |event| {
        matches!(event, ClipEvent::ClipboardReceived { text, .. } if text.len() == MAX_TEXT_BYTES)
    });
    if let ClipEvent::ClipboardReceived { text, .. } = event {
        assert_eq!(text, big);
    }

    assert!(dir_a.join("trust.json").exists());
    assert!(dir_b.join("trust.json").exists());
    a.stop();
    assert!(a.poll_event().is_none(), "stop must clear pending events");
    assert!(matches!(
        a.publish_text("after stop"),
        Err(ClipError::Stopped)
    ));
    assert!(a.peers().is_empty());
    b.stop();
    let _ = fs::remove_dir_all(dir_a);
    let _ = fs::remove_dir_all(dir_b);
}

#[test]
fn last_writer_wins_and_stale_is_not_echoed() {
    let port = unique_port();
    let (a, dir_a) = start("a", port);
    let (b, dir_b) = start("b", port);
    let mut log_a = Log::new();
    let mut log_b = Log::new();
    discover(&a, &b, &mut log_a, &mut log_b);
    pair_nodes(&a, &b, &mut log_a, &mut log_b);

    let va = published_version(a.publish_text("alpha").unwrap());
    let vb = published_version(b.publish_text("beta").unwrap());
    assert_ne!(va, vb);

    let (winner, winner_text, winner_version, loser, winner_log, loser_log) = if va > vb {
        (&a, "alpha", va, &b, &mut log_a, &mut log_b)
    } else {
        (&b, "beta", vb, &a, &mut log_b, &mut log_a)
    };
    loser_log.wait(loser, WAIT, |event| {
        matches!(event, ClipEvent::ClipboardReceived { text, version, .. }
            if text == winner_text && *version == winner_version)
    });
    // 输家已应用赢家内容：再次从本地剪贴板回灌同一内容应被抑制。
    assert_eq!(
        loser.publish_text(winner_text).unwrap(),
        PublishOutcome::SuppressedLoop
    );
    winner_log
        .quiet(winner, Duration::from_millis(400), |event| {
            matches!(event, ClipEvent::ClipboardReceived { .. })
        })
        .none();

    a.stop();
    b.stop();
    let _ = fs::remove_dir_all(dir_a);
    let _ = fs::remove_dir_all(dir_b);
}

#[test]
fn rejected_sas_leaves_both_sides_untrusted() {
    let port = unique_port();
    let (a, dir_a) = start("a", port);
    let (b, dir_b) = start("b", port);
    let a_id = a.local_instance_id();
    let b_id = b.local_instance_id();
    let mut log_a = Log::new();
    let mut log_b = Log::new();
    discover(&a, &b, &mut log_a, &mut log_b);

    a.pair(b_id).unwrap();
    let prompt_a = log_a.wait(&a, WAIT, |event| {
        matches!(event, ClipEvent::PairingPrompt { instance_id, .. } if *instance_id == b_id)
    });
    let prompt_b = log_b.wait(&b, WAIT, |event| {
        matches!(event, ClipEvent::PairingPrompt { instance_id, .. } if *instance_id == a_id)
    });
    if let (ClipEvent::PairingPrompt { sas: sa, .. }, ClipEvent::PairingPrompt { sas: sb, .. }) =
        (&prompt_a, &prompt_b)
    {
        assert_eq!(sa, sb);
    }

    // 模拟用户看到 SAS 后拒绝（错误 SAS 的处置路径）。
    b.confirm_pairing(a_id, false).unwrap();
    a.confirm_pairing(b_id, true).unwrap();
    log_a.wait(&a, WAIT, |event| {
        matches!(event, ClipEvent::PairingFinished { instance_id, trusted: false, .. }
            if *instance_id == b_id)
    });
    log_b.wait(&b, WAIT, |event| {
        matches!(event, ClipEvent::PairingFinished { instance_id, trusted: false, .. }
            if *instance_id == a_id)
    });

    assert!(!dir_a.join("trust.json").exists());
    assert!(!dir_b.join("trust.json").exists());
    assert!(
        a.peers()
            .iter()
            .all(|peer| !(peer.instance_id == b_id && peer.trusted))
    );
    assert!(matches!(
        a.publish_text("not paired").unwrap(),
        PublishOutcome::Published { peers: 0, .. }
    ));
    log_b
        .quiet(&b, Duration::from_millis(500), |event| {
            matches!(event, ClipEvent::ClipboardReceived { .. })
        })
        .none();

    a.stop();
    b.stop();
    let _ = fs::remove_dir_all(dir_a);
    let _ = fs::remove_dir_all(dir_b);
}

#[test]
fn oversize_text_is_rejected_before_sending() {
    let port = unique_port();
    let (node, dir) = start("solo", port);
    let too_big = "z".repeat(MAX_TEXT_BYTES + 1);
    assert!(matches!(
        node.publish_text(&too_big),
        Err(ClipError::TooLarge(size)) if size == MAX_TEXT_BYTES + 1
    ));
    let max = "z".repeat(MAX_TEXT_BYTES);
    assert!(matches!(
        node.publish_text(&max).unwrap(),
        PublishOutcome::Published { peers: 0, .. }
    ));
    node.stop();
    let _ = fs::remove_dir_all(dir);
}
