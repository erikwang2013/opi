// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Noise_XX 配对 + 6 位 SAS 人工确认。双方都确认后才写 trust store。
//! SAS 由握手 transcript 派生；真实身份绑定由 Noise 静态公钥完成。

use crate::ClipError;
use crate::crypto;
use crate::frame::{LinkHeader, LinkMode, read_frame_blocking, write_frame};
use crate::node::NodeInner;
use crate::noise::{self, PairHandshake};
use crate::trust::TrustedPeer;
use crate::types::{ClipEvent, InstanceId, PAIRING_TIMEOUT, PROTOCOL_VERSION, unix_secs};
use crate::wire::{self, Wire};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const IO_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_NAME_BYTES: usize = 128;

struct PairOutcome {
    trusted: bool,
    reason: Option<String>,
}

pub(crate) fn run_initiator(inner: Arc<NodeInner>, peer: InstanceId, addr: SocketAddr) {
    let outcome = match initiate(&inner, peer, addr) {
        Ok(outcome) => outcome,
        Err(error) => PairOutcome {
            trusted: false,
            reason: Some(error.to_string()),
        },
    };
    finish(&inner, peer, outcome);
}

pub(crate) fn run_responder(inner: Arc<NodeInner>, mut stream: TcpStream, header: LinkHeader) {
    let peer = header.initiator;
    let outcome = match respond(&inner, &mut stream, peer) {
        Ok(outcome) => outcome,
        Err(error) => PairOutcome {
            trusted: false,
            reason: Some(error.to_string()),
        },
    };
    finish(&inner, peer, outcome);
}

fn finish(inner: &Arc<NodeInner>, peer: InstanceId, outcome: PairOutcome) {
    inner.lock().pairing_attempts.remove(&peer);
    inner.emit(ClipEvent::PairingFinished {
        instance_id: peer,
        trusted: outcome.trusted,
        reason: outcome.reason,
    });
}

fn initiate(
    inner: &Arc<NodeInner>,
    peer: InstanceId,
    addr: SocketAddr,
) -> Result<PairOutcome, ClipError> {
    let expected = inner
        .lock()
        .discovered
        .get(&peer)
        .map(|discovered| discovered.fingerprint)
        .ok_or(ClipError::PeerNotFound)?;
    let own = inner.identity.instance_id;
    let mut stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)?;
    configure(&stream)?;
    write_frame(
        &mut stream,
        &LinkHeader {
            mode: LinkMode::Pairing,
            initiator: own,
            responder: peer,
        }
        .encode(),
    )?;

    let prologue = crypto::prologue(2, own, peer);
    let handshake = noise::xx_initiator(&mut stream, &inner.identity, &prologue)?;
    if crypto::fingerprint(&handshake.remote_static) != expected {
        return Err(ClipError::invalid(
            "peer static key does not match discovery",
        ));
    }
    let PairHandshake {
        transport,
        remote_static,
        handshake_hash,
    } = handshake;
    let mut transport = transport;
    let peer_name = exchange_info(
        &mut stream,
        &mut transport,
        own,
        &inner.config.device_name,
        peer,
    )?;

    let code = crypto::sas(&prologue, &handshake_hash);
    let local_accept = prompt(inner, peer, &code, false)?;
    stream.set_read_timeout(Some(PAIRING_TIMEOUT + Duration::from_secs(5)))?;
    send_wire(
        &mut stream,
        &mut transport,
        &Wire::PairDecision {
            accept: local_accept,
        },
    )?;
    let peer_accept = read_decision(&mut stream, &mut transport)?;
    complete(
        inner,
        peer,
        peer_name,
        remote_static,
        local_accept,
        peer_accept,
    )
}

fn respond(
    inner: &Arc<NodeInner>,
    stream: &mut TcpStream,
    peer: InstanceId,
) -> Result<PairOutcome, ClipError> {
    configure(stream)?;
    let own = inner.identity.instance_id;
    let prologue = crypto::prologue(2, peer, own);
    let handshake = noise::xx_responder(stream, &inner.identity, &prologue)?;
    let PairHandshake {
        transport,
        remote_static,
        handshake_hash,
    } = handshake;
    let mut transport = transport;
    let peer_name = exchange_info(stream, &mut transport, own, &inner.config.device_name, peer)?;

    let code = crypto::sas(&prologue, &handshake_hash);
    let local_accept = prompt(inner, peer, &code, true)?;
    stream.set_read_timeout(Some(PAIRING_TIMEOUT + Duration::from_secs(5)))?;
    send_wire(
        stream,
        &mut transport,
        &Wire::PairDecision {
            accept: local_accept,
        },
    )?;
    let peer_accept = read_decision(stream, &mut transport)?;
    complete(
        inner,
        peer,
        peer_name,
        remote_static,
        local_accept,
        peer_accept,
    )
}

fn complete(
    inner: &Arc<NodeInner>,
    peer: InstanceId,
    peer_name: String,
    remote_static: Vec<u8>,
    local_accept: bool,
    peer_accept: bool,
) -> Result<PairOutcome, ClipError> {
    if !local_accept {
        return Ok(PairOutcome {
            trusted: false,
            reason: Some("declined locally".into()),
        });
    }
    if !peer_accept {
        return Ok(PairOutcome {
            trusted: false,
            reason: Some("declined by peer".into()),
        });
    }
    let trusted = TrustedPeer {
        instance_id: peer,
        name: peer_name,
        fingerprint: crypto::fingerprint(&remote_static),
        public_key: remote_static,
        paired_at: unix_secs(),
    };
    inner.lock().trust_peer(trusted)?;
    Ok(PairOutcome {
        trusted: true,
        reason: None,
    })
}

fn configure(stream: &TcpStream) -> Result<(), ClipError> {
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    Ok(())
}

fn exchange_info(
    stream: &mut TcpStream,
    transport: &mut snow::TransportState,
    own: InstanceId,
    device_name: &str,
    expected: InstanceId,
) -> Result<String, ClipError> {
    send_wire(
        stream,
        transport,
        &Wire::PairInfo {
            protocol: PROTOCOL_VERSION,
            instance_id: own,
            name: device_name.to_owned(),
        },
    )?;
    match read_wire(stream, transport)? {
        Wire::PairInfo {
            protocol,
            instance_id,
            name,
        } => {
            if protocol != PROTOCOL_VERSION || instance_id != expected {
                return Err(ClipError::invalid("pairing identity mismatch"));
            }
            if name.len() > MAX_NAME_BYTES {
                return Err(ClipError::invalid("peer name too long"));
            }
            Ok(name)
        }
        _ => Err(ClipError::invalid("unexpected pairing message")),
    }
}

fn read_decision(
    stream: &mut TcpStream,
    transport: &mut snow::TransportState,
) -> Result<bool, ClipError> {
    match read_wire(stream, transport)? {
        Wire::PairDecision { accept } => Ok(accept),
        _ => Err(ClipError::invalid("missing pairing decision")),
    }
}

fn send_wire(
    stream: &mut TcpStream,
    transport: &mut snow::TransportState,
    message: &Wire,
) -> Result<(), ClipError> {
    let bytes = wire::encode(message)?;
    noise::send_encrypted(stream, transport, &bytes)
}

fn read_wire(
    stream: &mut TcpStream,
    transport: &mut snow::TransportState,
) -> Result<Wire, ClipError> {
    let frame = read_frame_blocking(stream).map_err(ClipError::from)?;
    let plaintext = noise::decrypt(transport, &frame)?;
    wire::decode(&plaintext)
}

/// 等待用户确认。`stop()`/超时/通道关闭都按拒绝处理，绝不默认接受。
fn prompt(
    inner: &Arc<NodeInner>,
    peer: InstanceId,
    code: &str,
    incoming: bool,
) -> Result<bool, ClipError> {
    let (sender, receiver) = mpsc::sync_channel(1);
    {
        let mut core = inner.lock();
        if core.pairing.contains_key(&peer) {
            return Err(ClipError::Busy("pairing prompt already active".into()));
        }
        core.pairing.insert(peer, sender);
        core.push_event(ClipEvent::PairingPrompt {
            instance_id: peer,
            sas: code.to_owned(),
            incoming,
        });
    }
    let deadline = Instant::now() + PAIRING_TIMEOUT;
    let accepted = loop {
        if inner.is_stopped() || Instant::now() >= deadline {
            break false;
        }
        match receiver.recv_timeout(Duration::from_millis(200)) {
            Ok(accept) => break accept,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break false,
        }
    };
    inner.lock().pairing.remove(&peer);
    Ok(accepted)
}
