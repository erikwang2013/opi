// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 同机双实例文件传输：配对 → offer → 接受 → 分块 → 完整性校验 → 落盘，
//! 以及取消、信任门槛、路径穿越拒绝。发现显式指定本机网卡，避免 docker/VPN
//! 多网卡时组播出口漂移（这是本机环境里既有 loopback 测试偶发超时的原因）。

use opi_clip::{
    ClipConfig, ClipError, ClipEvent, ClipNode, InstanceId, TransferFile, TransferFileInfo,
    collect_files,
};
use std::fs;
use std::net::{Ipv4Addr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU16, AtomicU32, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(10);

static NEXT_PORT: AtomicU16 = AtomicU16::new(47510);
static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

fn unique_port() -> u16 {
    NEXT_PORT.fetch_add(1, Ordering::Relaxed)
}

fn temp_dir(tag: &str) -> PathBuf {
    let n = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("opi-clip-xfer-{tag}-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 默认路由对应的本机 IPv4：组播/广播都显式走这块网卡。
fn local_ipv4() -> Option<Ipv4Addr> {
    let probe = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    probe.connect((Ipv4Addr::new(8, 8, 8, 8), 80)).ok()?;
    match probe.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(ip) => Some(ip),
        std::net::IpAddr::V6(_) => None,
    }
}

fn start(tag: &str, discovery_port: u16) -> (ClipNode, PathBuf) {
    let dir = temp_dir(tag);
    let mut config = ClipConfig::new(&dir);
    config.discovery_port = discovery_port;
    // 设备名只允许中文/ASCII 字母/数字（ClipConfig::validate）。
    let mut name: String = tag.chars().filter(|c| c.is_ascii_alphanumeric()).take(20).collect();
    if name.is_empty() { name = "test".into(); }
    config.device_name = name;
    // 手动配对用例保持原有的 SAS 流程；自动连接由单独用例覆盖。
    config.auto_connect = false;
    config.advertise_ipv4 = local_ipv4();
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
                "timed out; saw {} events: {:?}",
                self.0.len(),
                self.0
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn has(&self, predicate: impl Fn(&ClipEvent) -> bool) -> bool {
        self.0.iter().any(predicate)
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
    assert_eq!(sas(prompt_a), sas(prompt_b));
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

fn write(pattern: u8, len: usize) -> Vec<u8> {
    (0..len).map(|i| pattern.wrapping_add(i as u8)).collect()
}

#[test]
fn folder_transfer_progress_and_cancel() {
    let port = unique_port();
    let (a, dir_a) = start("a", port);
    let (b, dir_b) = start("b", port);
    let a_id = a.local_instance_id();
    let b_id = b.local_instance_id();
    let source = temp_dir("source");
    let save = temp_dir("save");
    let mut log_a = Log::new();
    let mut log_b = Log::new();

    discover(&a, &b, &mut log_a, &mut log_b);
    pair_nodes(&a, &b, &mut log_a, &mut log_b);

    let folder = source.join("相册");
    fs::create_dir_all(folder.join("sub")).unwrap();
    let text = write(1, 100 * 1024);
    let image = write(2, 2 * 1024 * 1024);
    fs::write(folder.join("a.txt"), &text).unwrap();
    fs::write(folder.join("sub/p.png"), &image).unwrap();
    let files = collect_files(&[folder]).unwrap();
    let expected_paths: Vec<_> = files.iter().map(TransferFile::relative_path).collect();
    assert_eq!(
        expected_paths,
        vec!["相册", "相册/a.txt", "相册/sub", "相册/sub/p.png"]
    );

    let transfer = a.send_files(b_id, files).unwrap();
    let offer = log_b.wait(&b, WAIT, |event| {
        matches!(event, ClipEvent::TransferOffer { transfer_id, outgoing: false, .. }
            if *transfer_id == transfer)
    });
    let ClipEvent::TransferOffer {
        files: offered,
        total_bytes,
        instance_id,
        ..
    } = &offer
    else {
        unreachable!()
    };
    assert_eq!(*instance_id, a_id);
    assert_eq!(*total_bytes, (text.len() + image.len()) as u64);
    assert!(
        offered
            .iter()
            .any(|file: &TransferFileInfo| file.mime == "image/png")
    );

    b.respond_transfer(transfer, true, Some(save.clone()))
        .unwrap();
    let completed = log_a.wait(&a, WAIT, |event| {
        matches!(event, ClipEvent::TransferCompleted { transfer_id, outgoing: true, .. }
            if *transfer_id == transfer)
    });
    assert!(matches!(
        completed,
        ClipEvent::TransferCompleted { ref files, .. } if files.len() == 4
    ));
    log_b.wait(&b, WAIT, |event| {
        matches!(event, ClipEvent::TransferCompleted { transfer_id, outgoing: false, saved_dir: Some(dir), .. }
            if *transfer_id == transfer && dir == &save)
    });
    assert!(
        log_a.has(|event| matches!(event, ClipEvent::TransferProgress {
            transfer_id, outgoing: true, transferred_bytes, .. } if *transfer_id == transfer && *transferred_bytes > 0)),
        "sender must report progress before completion"
    );

    let saved_image = save.join("相册/sub/p.png");
    assert_eq!(fs::read(save.join("相册/a.txt")).unwrap(), text);
    assert_eq!(fs::read(&saved_image).unwrap(), image);
    assert!(!saved_image.with_extension("png.part").exists());
    assert!(!save.join("相册/a.txt.part").exists());

    // 取消：>64 块（6 MiB）保证取消命令在发送端完成前被处理。
    let big_path = source.join("big.bin");
    let big = write(3, 6 * 1024 * 1024);
    fs::write(&big_path, &big).unwrap();
    let big_file = TransferFile::from_path(&big_path).unwrap();
    let transfer = a.send_files(b_id, vec![big_file]).unwrap();
    log_b.wait(&b, WAIT, |event| {
        matches!(event, ClipEvent::TransferOffer { transfer_id, outgoing: false, .. }
            if *transfer_id == transfer)
    });
    b.respond_transfer(transfer, true, Some(save.join("cancel")))
        .unwrap();
    a.cancel_transfer(transfer).unwrap();
    log_a.wait(&a, WAIT, |event| {
        matches!(event, ClipEvent::TransferCanceled { transfer_id, outgoing: true, .. }
            if *transfer_id == transfer)
    });
    log_b.wait(&b, WAIT, |event| {
        matches!(event, ClipEvent::TransferCanceled { transfer_id, outgoing: false, .. }
            if *transfer_id == transfer)
    });

    a.stop();
    b.stop();
    for dir in [dir_a, dir_b, source, save] {
        let _ = fs::remove_dir_all(dir);
    }
}

#[test]
fn send_files_requires_trusted_peer() {
    let (node, dir) = start("solo", unique_port());
    let source = temp_dir("solo-source");
    let path = source.join("a.txt");
    fs::write(&path, b"secret").unwrap();
    let file = TransferFile::from_path(&path).unwrap();
    let stranger = InstanceId::from_bytes([9; 16]);
    assert!(matches!(
        node.send_files(stranger, vec![file]),
        Err(ClipError::NotTrusted)
    ));
    assert!(matches!(
        node.respond_transfer(
            opi_clip::TransferId::from_bytes([1; 16]),
            true,
            Some(source.clone())
        ),
        Err(ClipError::TransferNotFound)
    ));
    node.stop();
    let _ = fs::remove_dir_all(dir);
    let _ = fs::remove_dir_all(source);
}

#[test]
fn transfer_paths_reject_traversal_and_absolute() {
    let dir = temp_dir("paths");
    let path = dir.join("a.txt");
    fs::write(&path, b"x").unwrap();
    for bad in ["../escape", "/etc/passwd", "a/../../b", "", "a\\b"] {
        assert!(
            TransferFile::from_path_named(&path, bad, None).is_err(),
            "{bad:?} must be rejected"
        );
    }
    assert!(TransferFile::from_path_named(&path, "ok/name.txt", None).is_ok());

    // collect_files 只输出字面相对路径；这里验证驱动器不把绝对路径写进 offer。
    let files = collect_files(&[path]).unwrap();
    assert_eq!(files[0].relative_path(), "a.txt");
    assert!(Path::new(files[0].relative_path()).is_relative());
    let _ = fs::remove_dir_all(dir);
}
