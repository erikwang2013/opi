// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 网络线程与连接处理：发现、accept、监督重连、会话/配对握手分派。
//! 会话线程由 `session` 模块在握手完成后接管。

use crate::ClipError;
use crate::crypto;
use crate::discovery::{self, Beacon};
use crate::frame::{LinkHeader, LinkMode, read_frame_blocking, write_frame};
use crate::node::{ActiveGuard, NodeInner, spawn_named};
use crate::noise;
use crate::pairing;
use crate::session;
use crate::types::{BEACON_INTERVAL, CAP_AUTO_PAIR, InstanceId};
use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(3);
const SUPERVISOR_TICK: Duration = Duration::from_millis(100);
/// 没有发现任何设备时，每隔这么久对 /24 逐机发一次探测；一旦发现就停。
/// /24 探测每次 254 个 sendto，在 ARP 未解析的网段上要 ~3 秒；2 秒一轮会让
/// 发现线程几乎一直阻塞在探测里、读不到组播。8 秒与注释里的兜底周期一致。
const PROBE_INTERVAL: Duration = Duration::from_secs(8);
const PROBE_FIRST_DELAY: Duration = Duration::ZERO;

/// 当前 beacon 的能力位：配置打开自动连接时额外通告 CAP_AUTO_PAIR。
fn make_beacon(inner: &NodeInner, tcp_port: u16) -> Beacon {
    let core = inner.lock();
    let mut capabilities = core.capabilities();
    if inner.config.auto_connect {
        capabilities |= CAP_AUTO_PAIR;
    }
    discovery::beacon_for(core.instance_id, &core.public_key, tcp_port, capabilities)
}

pub(crate) fn discovery_loop(inner: Arc<NodeInner>, socket: UdpSocket, tcp_port: u16) {
    let mut next_beacon = Instant::now();
    let mut next_probe = Instant::now() + PROBE_FIRST_DELAY;
    let mut buffer = [0u8; 64];
    while !inner.is_stopped() {
        let now = Instant::now();
        let beacon = if now >= next_beacon || now >= next_probe {
            Some(make_beacon(&inner, tcp_port))
        } else {
            None
        };
        if now >= next_beacon {
            if let Some(beacon) = beacon {
                let _ = discovery::send_beacon(
                    &socket,
                    inner.config.discovery_port,
                    beacon,
                    inner.config.advertise_ipv4,
                    inner.config.advertise_prefix,
                );
                inner
                    .beacons_sent
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            next_beacon = now + BEACON_INTERVAL;
        }
        if now >= next_probe {
            let no_peers = inner.lock().discovered.is_empty();
            if no_peers && let Some(beacon) = beacon {
                let _ = discovery::send_probes(
                    &socket,
                    inner.config.discovery_port,
                    beacon,
                    inner.config.advertise_ipv4,
                    inner.config.advertise_prefix,
                );
                inner
                    .beacons_sent
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            next_probe = now + PROBE_INTERVAL;
        }
        // 手动连接：设置页输入的 IP 立即发一个探测，不等 8 秒周期。
        let requested: Vec<std::net::Ipv4Addr> = {
            let mut q = inner
                .probes
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::take(&mut *q)
        };
        if !requested.is_empty() {
            let probe = make_beacon(&inner, tcp_port);
            for ip in requested {
                let _ = discovery::send_probe_to(&socket, inner.config.discovery_port, probe, ip);
                inner
                    .beacons_sent
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }
        match socket.recv_from(&mut buffer) {
            Ok((len, source)) => {
                if let Some(mut beacon) = Beacon::decode(&buffer[..len]) {
                    inner
                        .beacons_received
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    // /24 探测的报文：立刻单播回一个普通 beacon，让对方也能发现我们。
                    if beacon.capabilities & crate::types::CAP_PROBE != 0 {
                        beacon.capabilities &= !crate::types::CAP_PROBE;
                        let reply = make_beacon(&inner, tcp_port);
                        let _ = socket.send_to(&reply.encode(), source);
                    }
                    inner.lock().observe_beacon(source, beacon, Instant::now());
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(error) => {
                inner.push_error(format!("discovery receive: {error}"));
                break;
            }
        }
    }
}

pub(crate) fn accept_loop(inner: Arc<NodeInner>, listener: TcpListener) {
    while !inner.is_stopped() {
        match listener.accept() {
            Ok((stream, source)) => {
                let Some(guard) = ActiveGuard::enter(&inner) else {
                    drop(stream);
                    continue;
                };
                let connection_inner = inner.clone();
                let spawned = spawn_named("opi-clip-conn", move || {
                    let _guard = guard;
                    handle_connection(connection_inner, stream, source);
                });
                if spawned.is_err() {
                    // 线程创建失败时闭包（含 guard）已 drop，不会泄漏计数。
                    continue;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => {
                inner.push_error(format!("tcp accept: {error}"));
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

fn handle_connection(inner: Arc<NodeInner>, mut stream: TcpStream, source: SocketAddr) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT));
    let _ = stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT));

    let header = match read_frame_blocking(&mut stream) {
        Ok(frame) => match LinkHeader::decode(&frame) {
            Ok(header) => header,
            Err(_) => return,
        },
        Err(_) => return,
    };
    if header.responder != inner.identity.instance_id
        || header.initiator == inner.identity.instance_id
    {
        return;
    }

    match header.mode {
        LinkMode::Pairing => handle_pairing_connection(&inner, stream, header, source),
        LinkMode::Session => handle_session_connection(&inner, stream, header, source),
    }
}

fn handle_pairing_connection(
    inner: &Arc<NodeInner>,
    stream: TcpStream,
    header: LinkHeader,
    source: SocketAddr,
) {
    let now = Instant::now();
    {
        let mut core = inner.lock();
        if !core.pairing_enabled
            || core.trust.contains(header.initiator)
            || core
                .pair_rate
                .get(&source.ip())
                .is_some_and(|last| now.duration_since(*last) < Duration::from_secs(3))
            || !core.pairing_attempts.insert(header.initiator)
        {
            return;
        }
        core.pair_rate.insert(source.ip(), now);
    }
    pairing::run_responder(inner.clone(), stream, header);
}

fn handle_session_connection(
    inner: &Arc<NodeInner>,
    mut stream: TcpStream,
    header: LinkHeader,
    source: SocketAddr,
) {
    let Some(trusted) = inner.lock().trust.get(header.initiator).cloned() else {
        return;
    };
    let prologue = crypto::prologue(1, header.initiator, inner.identity.instance_id);
    let Ok(handshake) =
        noise::kk_responder(&mut stream, &inner.identity, &trusted.public_key, &prologue)
    else {
        return;
    };
    if handshake.remote_static != trusted.public_key {
        return;
    }
    session::start(inner.clone(), stream, header.initiator, handshake, source);
}

pub(crate) fn supervisor_loop(inner: Arc<NodeInner>) {
    while !inner.is_stopped() {
        thread::sleep(SUPERVISOR_TICK);
        if inner.is_stopped() {
            break;
        }
        let now = Instant::now();
        let (actions, pair_actions) = {
            let mut core = inner.lock();
            core.prune(now);
            let own = core.instance_id;
            let mut actions = Vec::new();
            let mut pair_actions = Vec::new();
            for (peer, discovered) in &core.discovered {
                // 只有实例 ID 较大的一侧主动连接/配对，避免双向同时建链。
                if *peer <= own {
                    continue;
                }
                if !core.trust.contains(*peer) {
                    // 双方都开启自动连接才 TOFU；老客户端没有 CAP_AUTO_PAIR，保持手动 SAS。
                    if inner.config.auto_connect
                        && core.pairing_enabled
                        && discovered.capabilities & CAP_AUTO_PAIR != 0
                        && !core.pairing_attempts.contains(peer)
                        && !core.pairing.contains_key(peer)
                        && core
                            .backoff
                            .get(peer)
                            .is_none_or(|backoff| now >= backoff.next)
                    {
                        pair_actions.push((*peer, discovered.addr));
                    }
                    continue;
                }
                if core.sessions.contains_key(peer) || core.connecting.contains(peer) {
                    continue;
                }
                if core
                    .backoff
                    .get(peer)
                    .is_some_and(|backoff| now < backoff.next)
                {
                    continue;
                }
                if let Some(trusted) = core.trust.get(*peer) {
                    actions.push((*peer, discovered.addr, trusted.public_key.clone()));
                }
            }
            for (peer, _, _) in &actions {
                core.connecting.insert(*peer);
            }
            for (peer, _) in &pair_actions {
                core.pairing_attempts.insert(*peer);
            }
            (actions, pair_actions)
        };
        for (peer, addr, public_key) in actions {
            spawn_connector(inner.clone(), peer, addr, public_key);
        }
        for (peer, addr) in pair_actions {
            spawn_auto_pair(inner.clone(), peer, addr);
        }
    }
}

/// 自动配对线程。`pairing_attempts` 已由 supervisor 预占，失败时释放并退避。
fn spawn_auto_pair(inner: Arc<NodeInner>, peer: InstanceId, addr: SocketAddr) {
    let Some(guard) = ActiveGuard::enter(&inner) else {
        inner.lock().pairing_attempts.remove(&peer);
        inner.schedule_backoff(peer);
        return;
    };
    let connection_inner = inner.clone();
    let spawned = spawn_named("opi-clip-auto-pair", move || {
        let _guard = guard;
        pairing::run_initiator(connection_inner, peer, addr);
    });
    if spawned.is_err() {
        inner.lock().pairing_attempts.remove(&peer);
        inner.schedule_backoff(peer);
    }
}

fn spawn_connector(inner: Arc<NodeInner>, peer: InstanceId, addr: SocketAddr, public_key: Vec<u8>) {
    let Some(guard) = ActiveGuard::enter(&inner) else {
        inner.schedule_backoff(peer);
        return;
    };
    let connection_inner = inner.clone();
    let spawned = spawn_named("opi-clip-session", move || {
        let _guard = guard;
        connect_session(connection_inner, peer, addr, public_key);
    });
    if spawned.is_err() {
        inner.schedule_backoff(peer);
    }
}

fn connect_session(inner: Arc<NodeInner>, peer: InstanceId, addr: SocketAddr, public_key: Vec<u8>) {
    let result = (|| -> Result<(), ClipError> {
        let mut stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)?;
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT))?;
        stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT))?;
        let header = LinkHeader {
            mode: LinkMode::Session,
            initiator: inner.identity.instance_id,
            responder: peer,
        };
        write_frame(&mut stream, &header.encode())?;
        let prologue = crypto::prologue(1, inner.identity.instance_id, peer);
        let handshake = noise::kk_initiator(&mut stream, &inner.identity, &public_key, &prologue)?;
        if handshake.remote_static != public_key {
            return Err(ClipError::invalid("peer static key mismatch"));
        }
        session::start(inner.clone(), stream, peer, handshake, addr);
        Ok(())
    })();
    if result.is_err() {
        inner.schedule_backoff(peer);
    }
}
