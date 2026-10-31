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
use crate::types::{BEACON_INTERVAL, InstanceId};
use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(3);
const SUPERVISOR_TICK: Duration = Duration::from_millis(100);

pub(crate) fn discovery_loop(inner: Arc<NodeInner>, socket: UdpSocket, tcp_port: u16) {
    let mut next_beacon = Instant::now();
    let mut buffer = [0u8; 64];
    while !inner.is_stopped() {
        let now = Instant::now();
        if now >= next_beacon {
            let (instance_id, public_key, capabilities) = {
                let core = inner.lock();
                (
                    core.instance_id,
                    core.public_key.clone(),
                    core.capabilities(),
                )
            };
            let beacon = discovery::beacon_for(instance_id, &public_key, tcp_port, capabilities);
            let _ = discovery::send_beacon(&socket, inner.config.discovery_port, beacon);
            next_beacon = now + BEACON_INTERVAL;
        }
        match socket.recv_from(&mut buffer) {
            Ok((len, source)) => {
                if let Some(beacon) = Beacon::decode(&buffer[..len]) {
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
        let actions = {
            let mut core = inner.lock();
            core.prune(now);
            let own = core.instance_id;
            let mut actions = Vec::new();
            for (peer, discovered) in &core.discovered {
                // 只有实例 ID 较小的一侧主动连接，避免双向同时建链。
                if *peer <= own
                    || core.sessions.contains_key(peer)
                    || core.connecting.contains(peer)
                    || !core.trust.contains(*peer)
                {
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
            actions
        };
        for (peer, addr, public_key) in actions {
            spawn_connector(inner.clone(), peer, addr, public_key);
        }
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
