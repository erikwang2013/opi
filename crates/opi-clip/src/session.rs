// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Noise_KK 会话：分片发送、逐事件 ACK/重试、入站重组、令牌桶限速、
//! 心跳与短读超时下的增量分帧。一个会话一线程，退出时注销并进入重连退避。

use crate::ClipError;
use crate::frame::FrameReader;
use crate::node::NodeInner;
use crate::noise::{self, PairHandshake};
use crate::state::{Apply, Fragment, FragmentOutcome};
use crate::types::{
    ACK_TIMEOUT, ClipEvent, EventId, FRAGMENT_BYTES, InstanceId, MAX_ACK_ATTEMPTS, MAX_FRAME_BYTES,
    Outbound, StoredEvent,
};
use crate::wire::{self, Wire};
use snow::TransportState;
use std::collections::HashMap;
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

const READ_TIMEOUT: Duration = Duration::from_millis(150);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const PING_INTERVAL: Duration = Duration::from_secs(10);
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_FRAMES_PER_PASS: usize = 64;
const MAX_PENDING_ACKS: usize = 8;
const OUTBOUND_QUEUE: usize = 8;
/// 入站限速：持续 2 MiB/s，突发 4 MiB；每帧另计 256 字节协议开销。
const RATE_BYTES_PER_SEC: f64 = 2.0 * 1024.0 * 1024.0;
const RATE_BURST: f64 = 4.0 * 1024.0 * 1024.0;
const FRAME_OVERHEAD: usize = 256;

struct PendingAck {
    event: StoredEvent,
    attempts: u8,
    sent_at: Instant,
}

struct RateLimit {
    tokens: f64,
    last: Instant,
}

impl RateLimit {
    fn new() -> Self {
        Self {
            tokens: RATE_BURST,
            last: Instant::now(),
        }
    }

    fn allow(&mut self, bytes: usize) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last).as_secs_f64();
        self.tokens = (self.tokens + elapsed * RATE_BYTES_PER_SEC).min(RATE_BURST);
        self.last = now;
        let cost = bytes as f64;
        if self.tokens >= cost {
            self.tokens -= cost;
            true
        } else {
            false
        }
    }
}

pub(crate) fn start(
    inner: Arc<NodeInner>,
    stream: TcpStream,
    peer: InstanceId,
    handshake: PairHandshake,
    addr: SocketAddr,
) {
    let (tx, rx) = std::sync::mpsc::sync_channel(OUTBOUND_QUEUE);
    let Some((generation, initial)) = inner.register_session(peer, addr, tx) else {
        // 已有同对端会话；直接关闭新连接。
        return;
    };
    let started = Instant::now();
    let PairHandshake { transport, .. } = handshake;
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));

    let mut session = Session {
        inner: inner.clone(),
        peer,
        generation,
        stream,
        transport,
        reader: FrameReader::new(),
        pending: HashMap::new(),
        rate: RateLimit::new(),
        last_received: Instant::now(),
        next_ping: Instant::now() + PING_INTERVAL,
    };
    if let Some(event) = initial {
        let _ = session.queue_event(event);
    }
    session.run(&rx);
    inner.unregister_session(peer, generation, started);
}

struct Session {
    inner: Arc<NodeInner>,
    peer: InstanceId,
    generation: u64,
    stream: TcpStream,
    transport: TransportState,
    reader: FrameReader,
    pending: HashMap<EventId, PendingAck>,
    rate: RateLimit,
    last_received: Instant,
    next_ping: Instant,
}

impl Session {
    fn run(&mut self, receiver: &Receiver<Outbound>) {
        loop {
            if self.inner.is_stopped() {
                break;
            }
            // forget_peer/停止会移除会话表项；旧线程必须退出，不能再收发数据。
            let active = self
                .inner
                .lock()
                .sessions
                .get(&self.peer)
                .is_some_and(|entry| entry.generation == self.generation);
            if !active {
                break;
            }
            if self.service_acks().is_err() || self.drain_outbound(receiver).is_err() {
                break;
            }
            if self.read_available().is_err() {
                break;
            }
            let now = Instant::now();
            if now.duration_since(self.last_received) >= IDLE_TIMEOUT {
                break;
            }
            if now >= self.next_ping {
                if self.send(&Wire::Ping).is_err() {
                    break;
                }
                self.next_ping = now + PING_INTERVAL;
            }
        }
        let _ = self.stream.shutdown(Shutdown::Both);
    }

    fn drain_outbound(&mut self, receiver: &Receiver<Outbound>) -> Result<(), ClipError> {
        loop {
            match receiver.try_recv() {
                Ok(Outbound::Publish(event)) => {
                    let id = event.event_id;
                    if let Err(error) = self.queue_event(event) {
                        self.inner.emit(ClipEvent::SendFailed {
                            instance_id: self.peer,
                            event_id: id,
                        });
                        return Err(error);
                    }
                }
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
    }

    fn read_available(&mut self) -> Result<(), ClipError> {
        for _ in 0..MAX_FRAMES_PER_PASS {
            let Some(frame) = self.reader.next(&mut self.stream)? else {
                return Ok(());
            };
            if !self.rate.allow(frame.len() + FRAME_OVERHEAD) {
                self.inner
                    .push_error(format!("rate limit exceeded by {}", self.peer));
                return Err(ClipError::invalid("peer rate limit"));
            }
            let plaintext = noise::decrypt(&mut self.transport, &frame)?;
            let message = wire::decode(&plaintext)?;
            self.handle(message)?;
        }
        Ok(())
    }

    fn handle(&mut self, message: Wire) -> Result<(), ClipError> {
        match message {
            Wire::EventFragment {
                event_id,
                version,
                total,
                index,
                total_len,
                hash,
                chunk,
            } => {
                let fragment = Fragment {
                    event_id,
                    version,
                    total,
                    index,
                    total_len,
                    hash,
                    chunk,
                };
                let outcome =
                    self.inner
                        .lock()
                        .accept_fragment(self.peer, fragment, Instant::now());
                match outcome {
                    FragmentOutcome::Done(Apply::Applied | Apply::Duplicate | Apply::Stale) => {
                        self.send(&Wire::Ack { event_id })?
                    }
                    FragmentOutcome::Pending
                    | FragmentOutcome::Done(Apply::Invalid)
                    | FragmentOutcome::Bad => {}
                }
            }
            Wire::Ack { event_id } => {
                self.pending.remove(&event_id);
            }
            Wire::Ping => self.send(&Wire::Pong)?,
            Wire::Pong => {}
            Wire::PairInfo { .. } | Wire::PairDecision { .. } => {
                return Err(ClipError::invalid("unexpected session message"));
            }
        }
        self.last_received = Instant::now();
        Ok(())
    }

    fn service_acks(&mut self) -> Result<(), ClipError> {
        let now = Instant::now();
        let expired: Vec<EventId> = self
            .pending
            .iter()
            .filter(|(_, pending)| now.duration_since(pending.sent_at) >= ACK_TIMEOUT)
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            let Some(mut pending) = self.pending.remove(&id) else {
                continue;
            };
            if pending.attempts >= MAX_ACK_ATTEMPTS {
                self.inner.emit(ClipEvent::SendFailed {
                    instance_id: self.peer,
                    event_id: id,
                });
                continue;
            }
            if let Err(error) = self.send_event(&pending.event) {
                self.inner.emit(ClipEvent::SendFailed {
                    instance_id: self.peer,
                    event_id: id,
                });
                return Err(error);
            }
            pending.attempts += 1;
            pending.sent_at = Instant::now();
            self.pending.insert(id, pending);
        }
        Ok(())
    }

    fn queue_event(&mut self, event: StoredEvent) -> Result<(), ClipError> {
        if self.pending.contains_key(&event.event_id) {
            return Ok(());
        }
        if self.pending.len() >= MAX_PENDING_ACKS {
            // 只保留最近的事件；LWW 下被挤掉的旧内容会被更新的事件覆盖。
            if let Some(oldest) = self
                .pending
                .iter()
                .min_by_key(|(_, pending)| pending.sent_at)
                .map(|(id, _)| *id)
            {
                self.pending.remove(&oldest);
            }
        }
        self.send_event(&event)?;
        self.pending.insert(
            event.event_id,
            PendingAck {
                event,
                attempts: 1,
                sent_at: Instant::now(),
            },
        );
        Ok(())
    }

    fn send_event(&mut self, event: &StoredEvent) -> Result<(), ClipError> {
        let bytes = event.text.as_bytes();
        let total = bytes.len().div_ceil(FRAGMENT_BYTES).max(1);
        for index in 0..total {
            let start = index * FRAGMENT_BYTES;
            let end = (start + FRAGMENT_BYTES).min(bytes.len());
            let message = Wire::EventFragment {
                event_id: event.event_id,
                version: event.version,
                total: total as u8,
                index: index as u8,
                total_len: bytes.len() as u32,
                hash: event.hash,
                chunk: bytes[start..end].to_vec(),
            };
            self.send(&message)?;
        }
        Ok(())
    }

    fn send(&mut self, message: &Wire) -> Result<(), ClipError> {
        let bytes = wire::encode(message)?;
        if bytes.len() > MAX_FRAME_BYTES {
            return Err(ClipError::invalid("session frame too large"));
        }
        noise::send_encrypted(&mut self.stream, &mut self.transport, &bytes)
    }
}
