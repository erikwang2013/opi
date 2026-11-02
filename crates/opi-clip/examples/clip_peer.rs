// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 双机发现/配对/传输的**真实网络**冒烟工具（测试用，不进产品）。
//!
//! 两个进程（最好在不同 netns/容器里）各起一个 `ClipNode`，对端地址由探测包
//! 直接指定，不依赖组播。用法：
//!   clip_peer a <dir> <port> <own-ip> <prefix> <peer-ip> [text]
//!   clip_peer b <dir> <port> <own-ip> <prefix> <peer-ip> [text]
//! 双方自动确认 SAS；a 在连接后发布文本，b 收到后打印 PASS。

use opi_clip::{ClipConfig, ClipEvent, ClipNode, PublishOutcome};
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 7 {
        eprintln!("usage: clip_peer <a|b> <dir> <port> <own-ip> <prefix> <peer-ip> [text]");
        std::process::exit(2);
    }
    let role = args[1].clone();
    let dir = PathBuf::from(&args[2]);
    let port: u16 = args[3].parse().expect("port");
    let own: Ipv4Addr = args[4].parse().expect("own ip");
    let prefix: u8 = args[5].parse().expect("prefix");
    let peer_ip: Ipv4Addr = args[6].parse().expect("peer ip");
    let text = args
        .get(7)
        .cloned()
        .unwrap_or_else(|| format!("hello-from-{role}"));

    let mut config = ClipConfig::new(&dir);
    config.discovery_port = port;
    config.device_name = format!("peer-{role}");
    config.advertise_ipv4 = Some(own);
    config.advertise_prefix = Some(prefix);
    let node = ClipNode::start(config).expect("start clip node");
    println!("READY {role} {}", node.local_instance_id());

    let deadline = Instant::now() + Duration::from_secs(40);
    let mut paired = false;
    let mut published = false;
    let mut received = false;
    let mut last_probe = Instant::now() - Duration::from_secs(10);

    while Instant::now() < deadline && !(received && role == "b") {
        // 0.0.0.0 = 不手动探测，只验证组播/广播自动发现。
        if !peer_ip.is_unspecified() && last_probe.elapsed() >= Duration::from_secs(1) {
            let _ = node.probe(peer_ip);
            last_probe = Instant::now();
        }
        while let Some(event) = node.poll_event() {
            match event {
                ClipEvent::PeerDiscovered(info) => {
                    if role == "a" && !paired {
                        node.pair(info.instance_id).expect("pair");
                        paired = true;
                    }
                }
                ClipEvent::PairingPrompt { instance_id, .. } => {
                    node.confirm_pairing(instance_id, true).expect("confirm");
                }
                ClipEvent::PairingFinished { trusted: true, .. } => {
                    paired = true;
                }
                ClipEvent::ClipboardReceived { text: got, .. } => {
                    if role == "b" {
                        println!("PASS received {} bytes", got.len());
                        received = true;
                    }
                }
                ClipEvent::Error { message } => eprintln!("error: {message}"),
                _ => {}
            }
        }
        if role == "a" && paired && !published && !node.peers().is_empty() {
            match node.publish_text(&text) {
                Ok(PublishOutcome::Published { peers, .. }) if peers > 0 => {
                    println!("PASS published to {peers} peer(s)");
                    published = true;
                }
                Ok(other) => eprintln!("publish not sent: {other:?}"),
                Err(e) => eprintln!("publish failed: {e}"),
            }
        }
        if published && role == "a" && last_probe.elapsed() < Duration::from_secs(1) {
            // 给对端留出接收/回执时间再退出。
            thread::sleep(Duration::from_millis(500));
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    if role == "b" && !received {
        eprintln!("FAIL no clipboard event");
        std::process::exit(1);
    }
    node.stop();
    if role == "a" {
        // a 可能先于 b 打印，退出码只表示「发出去了」；b 的 PASS 才是端到端。
        println!("DONE {role}");
    }
}
