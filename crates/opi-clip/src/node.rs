// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 公开句柄与线程编排。停止语义：置位 → 清空内存队列（会话发送端同时 drop）→
//! join 主线程 → 最多等 1 秒让会话/配对线程自行退出。没有剪贴板历史可清。

use crate::ClipError;
use crate::config::ClipConfig;
use crate::discovery;
use crate::identity::Identity;
use crate::net;
use crate::pairing;
use crate::state::{Backoff, Core};
use crate::trust::TrustStore;
use crate::types::{
    ClipEvent, Fingerprint, InstanceId, Outbound, PeerInfo, PublishOutcome, SessionEntry,
    StoredEvent,
};
use std::io;
use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const MAX_ACTIVE: usize = 32;
const MAX_BACKOFF_SECS: u64 = 30;
const STABLE_SESSION: Duration = Duration::from_secs(10);

pub(crate) struct NodeInner {
    pub(crate) core: Mutex<Core>,
    pub(crate) stop: AtomicBool,
    pub(crate) identity: Identity,
    pub(crate) config: ClipConfig,
    pub(crate) generation: AtomicU64,
    active: AtomicUsize,
}

impl NodeInner {
    pub(crate) fn lock(&self) -> MutexGuard<'_, Core> {
        self.core
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn is_stopped(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    pub(crate) fn emit(&self, event: ClipEvent) {
        let mut core = self.lock();
        // 停止标志必须在锁内复查：否则 stop() 清空队列后，已通过首检的线程
        // 仍可能把事件插回队列。
        if !self.is_stopped() {
            core.push_event(event);
        }
    }

    pub(crate) fn push_error(&self, message: impl Into<String>) {
        self.emit(ClipEvent::Error {
            message: message.into(),
        });
    }

    /// 注册会话；已有同对端会话时返回 `None`（调用方直接关闭新连接）。
    pub(crate) fn register_session(
        &self,
        peer: InstanceId,
        addr: SocketAddr,
        tx: std::sync::mpsc::SyncSender<Outbound>,
    ) -> Option<(u64, Option<StoredEvent>)> {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let mut core = self.lock();
        if core.sessions.contains_key(&peer) {
            return None;
        }
        core.sessions.insert(
            peer,
            SessionEntry {
                tx,
                generation,
                addr,
            },
        );
        core.connecting.remove(&peer);
        // 保留 backoff，让短命会话在重连时继续指数退避；稳定会话结束时
        // unregister_session 会把 attempts 归零。
        Some((generation, core.pending_for_session()))
    }

    pub(crate) fn unregister_session(&self, peer: InstanceId, generation: u64, started: Instant) {
        let mut core = self.lock();
        let removed = core
            .sessions
            .get(&peer)
            .is_some_and(|session| session.generation == generation);
        if removed {
            core.sessions.remove(&peer);
        }
        core.connecting.remove(&peer);
        if removed && !self.is_stopped() {
            let attempts = if started.elapsed() >= STABLE_SESSION {
                0
            } else {
                core.backoff
                    .get(&peer)
                    .map_or(0, |b| b.attempts)
                    .saturating_add(1)
            };
            core.backoff.insert(
                peer,
                Backoff {
                    attempts,
                    next: Instant::now() + backoff_delay(attempts),
                },
            );
        }
    }

    pub(crate) fn schedule_backoff(&self, peer: InstanceId) {
        let mut core = self.lock();
        core.connecting.remove(&peer);
        if self.is_stopped() {
            return;
        }
        let attempts = core
            .backoff
            .get(&peer)
            .map_or(0, |b| b.attempts)
            .saturating_add(1);
        core.backoff.insert(
            peer,
            Backoff {
                attempts,
                next: Instant::now() + backoff_delay(attempts),
            },
        );
    }
}

fn backoff_delay(attempts: u32) -> Duration {
    Duration::from_secs((1u64 << attempts.min(5)).min(MAX_BACKOFF_SECS))
}

pub struct ClipNode {
    inner: Arc<NodeInner>,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl ClipNode {
    pub fn start(config: ClipConfig) -> Result<Self, ClipError> {
        config.validate()?;
        std::fs::create_dir_all(&config.data_dir)?;
        let identity = Identity::load_or_create(&config.data_dir)?;
        let trust_path = config.data_dir.join("trust.json");
        let trust = TrustStore::load(&trust_path)?;

        let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, config.tcp_port))?;
        listener.set_nonblocking(true)?;
        let tcp_port = listener.local_addr()?.port();
        let udp = discovery::bind(config.discovery_port)?;

        let inner = Arc::new(NodeInner {
            core: Mutex::new(Core::new(
                identity.instance_id,
                identity.public_key.clone(),
                trust,
                trust_path,
            )),
            stop: AtomicBool::new(false),
            identity,
            config,
            generation: AtomicU64::new(0),
            active: AtomicUsize::new(0),
        });

        let mut threads = Vec::new();
        let spawned = (|| -> io::Result<()> {
            let discovery_inner = inner.clone();
            threads.push(spawn_named("opi-clip-discovery", move || {
                net::discovery_loop(discovery_inner, udp, tcp_port);
            })?);
            let accept_inner = inner.clone();
            threads.push(spawn_named("opi-clip-accept", move || {
                net::accept_loop(accept_inner, listener);
            })?);
            let supervisor_inner = inner.clone();
            threads.push(spawn_named("opi-clip-supervisor", move || {
                net::supervisor_loop(supervisor_inner);
            })?);
            Ok(())
        })();
        if let Err(error) = spawned {
            inner.stop.store(true, Ordering::SeqCst);
            for handle in threads {
                let _ = handle.join();
            }
            return Err(error.into());
        }
        Ok(Self {
            inner,
            threads: Mutex::new(threads),
        })
    }

    pub fn local_instance_id(&self) -> InstanceId {
        self.inner.identity.instance_id
    }

    pub fn local_fingerprint(&self) -> Fingerprint {
        self.inner.identity.fingerprint
    }

    pub fn config(&self) -> &ClipConfig {
        &self.inner.config
    }

    /// 本地剪贴板变化。无连接时事件保留在内存 pending 中，等会话建立后补发。
    pub fn publish_text(&self, text: &str) -> Result<PublishOutcome, ClipError> {
        if self.inner.is_stopped() {
            return Err(ClipError::Stopped);
        }
        let (event, senders) = {
            let mut core = self.inner.lock();
            let Some(event) = core.prepare_local(text)? else {
                return Ok(PublishOutcome::SuppressedLoop);
            };
            let senders: Vec<_> = core.sessions.values().map(|s| s.tx.clone()).collect();
            (event, senders)
        };
        let mut peers = 0;
        for sender in senders {
            if sender.try_send(Outbound::Publish(event.clone())).is_ok() {
                peers += 1;
            }
        }
        Ok(PublishOutcome::Published {
            event_id: event.event_id,
            version: event.version,
            peers,
        })
    }

    pub fn poll_event(&self) -> Option<ClipEvent> {
        self.inner.lock().events.pop_front()
    }

    pub fn peers(&self) -> Vec<PeerInfo> {
        self.inner.lock().peer_snapshot()
    }

    /// 对发现到的未知实例发起 Noise_XX 配对；SAS 通过 `PairingPrompt` 事件返回。
    pub fn pair(&self, peer: InstanceId) -> Result<(), ClipError> {
        if self.inner.is_stopped() {
            return Err(ClipError::Stopped);
        }
        let addr = {
            let mut core = self.inner.lock();
            if core.trust.contains(peer) {
                return Err(ClipError::Busy("peer is already trusted".into()));
            }
            let Some(discovered) = core.discovered.get(&peer) else {
                return Err(ClipError::PeerNotFound);
            };
            let addr = discovered.addr;
            if !core.pairing_attempts.insert(peer) {
                return Err(ClipError::Busy("pairing already in progress".into()));
            }
            addr
        };

        let Some(guard) = ActiveGuard::enter(&self.inner) else {
            self.inner.lock().pairing_attempts.remove(&peer);
            return Err(ClipError::Busy("too many active connections".into()));
        };
        let inner = self.inner.clone();
        let spawned = spawn_named("opi-clip-pair-init", move || {
            let _guard = guard;
            pairing::run_initiator(inner, peer, addr);
        });
        if let Err(error) = spawned {
            self.inner.lock().pairing_attempts.remove(&peer);
            return Err(error.into());
        }
        Ok(())
    }

    /// 用户在 SAS 界面上确认/拒绝。必须在收到 `PairingPrompt` 后调用一次。
    pub fn confirm_pairing(&self, peer: InstanceId, accept: bool) -> Result<(), ClipError> {
        let sender = self
            .inner
            .lock()
            .pairing
            .remove(&peer)
            .ok_or(ClipError::NotPairing)?;
        sender.try_send(accept).map_err(|_| ClipError::NotPairing)
    }

    pub fn set_pairing_enabled(&self, enabled: bool) {
        self.inner.lock().pairing_enabled = enabled;
    }

    pub fn forget_peer(&self, peer: InstanceId) -> Result<(), ClipError> {
        let mut core = self.inner.lock();
        core.forget_peer(peer)?;
        core.sessions.remove(&peer);
        core.pairing.remove(&peer);
        core.connecting.remove(&peer);
        core.backoff.remove(&peer);
        core.pairing_attempts.remove(&peer);
        Ok(())
    }

    pub fn stop(&self) {
        if self.inner.stop.swap(true, Ordering::SeqCst) {
            return;
        }
        self.inner.lock().clear_queues();
        let handles = {
            let mut guard = self
                .threads
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::take(&mut *guard)
        };
        for handle in handles {
            let _ = handle.join();
        }
        let deadline = Instant::now() + Duration::from_secs(1);
        while self.inner.active.load(Ordering::SeqCst) > 0 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for ClipNode {
    fn drop(&mut self) {
        self.stop();
    }
}

pub(crate) struct ActiveGuard {
    inner: Arc<NodeInner>,
}

impl ActiveGuard {
    pub(crate) fn enter(inner: &Arc<NodeInner>) -> Option<Self> {
        if inner.active.fetch_add(1, Ordering::SeqCst) >= MAX_ACTIVE {
            inner.active.fetch_sub(1, Ordering::SeqCst);
            return None;
        }
        Some(Self {
            inner: inner.clone(),
        })
    }
}

impl Drop for ActiveGuard {
    fn drop(&mut self) {
        self.inner.active.fetch_sub(1, Ordering::SeqCst);
    }
}

pub(crate) fn spawn_named<F>(name: &str, work: F) -> io::Result<JoinHandle<()>>
where
    F: FnOnce() + Send + 'static,
{
    thread::Builder::new().name(name.to_owned()).spawn(work)
}
