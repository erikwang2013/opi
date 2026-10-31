// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 共享内存状态：HLC/LWW、去重、哈希抑制、分片重组、发现表、会话表、事件队列。
//! 所有可变状态由 `NodeInner.core` 的单个 Mutex 保护；任何剪贴板历史都不落盘。

use crate::ClipError;
use crate::crypto::{content_hash, content_hash_bytes, event_id};
use crate::discovery::Beacon;
use crate::trust::{TrustStore, TrustedPeer};
use crate::types::{
    CAP_PAIRING, CAP_TEXT, ClipEvent, FRAGMENT_BYTES, FRAGMENT_EXPIRY, Fingerprint,
    HASH_SUPPRESS_TTL, Hlc, InstanceId, MAX_REASSEMBLY_BYTES, MAX_REASSEMBLY_EVENTS,
    MAX_TEXT_BYTES, PEER_EXPIRY, PeerInfo, SEEN_EVENT_TTL, SessionEntry, StoredEvent, Version,
    now_ms,
};
use crate::util::{fragment_count, trim_oldest};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const MAX_EVENTS: usize = 1024;
const MAX_SEEN: usize = 1024;
const MAX_REMOTE_HASHES: usize = 64;

pub(crate) struct Core {
    pub instance_id: InstanceId,
    pub public_key: Vec<u8>,
    pub hlc: Hlc,
    pub current: Option<Version>,
    pub pending_local: Option<StoredEvent>,
    pub trust: TrustStore,
    pub trust_path: PathBuf,
    pub discovered: HashMap<InstanceId, Discovered>,
    pub sessions: HashMap<InstanceId, SessionEntry>,
    pub connecting: HashSet<InstanceId>,
    pub backoff: HashMap<InstanceId, Backoff>,
    pub pairing: HashMap<InstanceId, mpsc::SyncSender<bool>>,
    pub pairing_attempts: HashSet<InstanceId>,
    pub pairing_enabled: bool,
    pub pair_rate: HashMap<IpAddr, Instant>,
    pub events: VecDeque<ClipEvent>,
    seen_events: HashMap<crate::types::EventId, Instant>,
    remote_hashes: HashMap<[u8; 32], Instant>,
    fragments: HashMap<crate::types::EventId, Reassembly>,
}

#[derive(Clone)]
pub(crate) struct Discovered {
    pub addr: SocketAddr,
    pub fingerprint: Fingerprint,
    pub capabilities: u8,
    pub last_seen: Instant,
}

#[derive(Clone, Copy)]
pub(crate) struct Backoff {
    pub attempts: u32,
    pub next: Instant,
}

pub(crate) struct Fragment {
    pub event_id: crate::types::EventId,
    pub version: Version,
    pub total: u8,
    pub index: u8,
    pub total_len: u32,
    pub hash: [u8; 32],
    pub chunk: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Apply {
    Applied,
    Duplicate,
    Stale,
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FragmentOutcome {
    Pending,
    Done(Apply),
    Bad,
}

struct Reassembly {
    version: Version,
    total: u8,
    total_len: usize,
    hash: [u8; 32],
    chunks: Vec<Option<Vec<u8>>>,
    started: Instant,
}

impl Core {
    pub(crate) fn new(
        instance_id: InstanceId,
        public_key: Vec<u8>,
        trust: TrustStore,
        trust_path: PathBuf,
    ) -> Self {
        Self {
            instance_id,
            public_key,
            hlc: Hlc::zero(),
            current: None,
            pending_local: None,
            trust,
            trust_path,
            discovered: HashMap::new(),
            sessions: HashMap::new(),
            connecting: HashSet::new(),
            backoff: HashMap::new(),
            pairing: HashMap::new(),
            pairing_attempts: HashSet::new(),
            pairing_enabled: true,
            pair_rate: HashMap::new(),
            events: VecDeque::new(),
            seen_events: HashMap::new(),
            remote_hashes: HashMap::new(),
            fragments: HashMap::new(),
        }
    }

    pub(crate) fn push_event(&mut self, event: ClipEvent) {
        if self.events.len() >= MAX_EVENTS {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    pub(crate) fn capabilities(&self) -> u8 {
        CAP_TEXT | if self.pairing_enabled { CAP_PAIRING } else { 0 }
    }

    /// 本地剪贴板变化。返回 `None` 表示这是刚由远端写入的内容（防回环抑制）。
    pub(crate) fn prepare_local(&mut self, text: &str) -> Result<Option<StoredEvent>, ClipError> {
        if text.len() > MAX_TEXT_BYTES {
            return Err(ClipError::TooLarge(text.len()));
        }
        let now = Instant::now();
        self.remote_hashes
            .retain(|_, at| now.duration_since(*at) < HASH_SUPPRESS_TTL);
        let hash = content_hash(text);
        if self.remote_hashes.contains_key(&hash) {
            return Ok(None);
        }

        let hlc = self.hlc.tick(now_ms());
        let event_id = event_id(self.instance_id, hlc, &hash);
        let version = Version::new(hlc, self.instance_id, event_id);
        self.current = Some(version);
        let event = StoredEvent {
            event_id,
            version,
            text: text.to_owned(),
            hash,
        };
        self.pending_local = Some(event.clone());
        Ok(Some(event))
    }

    pub(crate) fn apply_event(
        &mut self,
        sender: InstanceId,
        event: &StoredEvent,
        now: Instant,
    ) -> Apply {
        if event.version.instance_id != sender
            || sender == self.instance_id
            || event.text.len() > MAX_TEXT_BYTES
        {
            return Apply::Invalid;
        }
        let hash = content_hash(&event.text);
        if hash != event.hash || event_id(sender, event.version.hlc(), &hash) != event.event_id {
            return Apply::Invalid;
        }
        self.hlc.merge(event.version.hlc());

        if self.seen_events.contains_key(&event.event_id) {
            return Apply::Duplicate;
        }
        let stale = self.current.is_some_and(|current| current >= event.version);
        self.remember_event(event.event_id, now);
        if stale {
            return Apply::Stale;
        }

        self.current = Some(event.version);
        if self
            .pending_local
            .as_ref()
            .is_some_and(|p| p.version <= event.version)
        {
            self.pending_local = None;
        }
        self.remote_hashes.insert(hash, now);
        self.trim_hashes();
        self.push_event(ClipEvent::ClipboardReceived {
            instance_id: sender,
            text: event.text.clone(),
            version: event.version,
        });
        Apply::Applied
    }

    pub(crate) fn accept_fragment(
        &mut self,
        sender: InstanceId,
        fragment: Fragment,
        now: Instant,
    ) -> FragmentOutcome {
        if fragment.version.instance_id != sender || sender == self.instance_id {
            return FragmentOutcome::Bad;
        }
        let total_len = fragment.total_len as usize;
        if total_len > MAX_TEXT_BYTES
            || event_id(sender, fragment.version.hlc(), &fragment.hash) != fragment.event_id
        {
            return FragmentOutcome::Bad;
        }
        let expected_total = fragment_count(total_len, FRAGMENT_BYTES);
        if usize::from(fragment.total) != expected_total
            || usize::from(fragment.index) >= expected_total
        {
            return FragmentOutcome::Bad;
        }
        let start = usize::from(fragment.index) * FRAGMENT_BYTES;
        let want = total_len.saturating_sub(start).min(FRAGMENT_BYTES);
        if fragment.chunk.len() != want {
            return FragmentOutcome::Bad;
        }

        // 已完成的事件（含重放）直接按去重处理，不必再占用重组内存。
        if self.seen_events.contains_key(&fragment.event_id) {
            return FragmentOutcome::Done(Apply::Duplicate);
        }

        self.prune_fragments(now);
        if !self.fragments.contains_key(&fragment.event_id) {
            if self.fragments.len() >= MAX_REASSEMBLY_EVENTS
                || self.reassembly_bytes() + total_len > MAX_REASSEMBLY_BYTES
            {
                return FragmentOutcome::Bad;
            }
            self.fragments.insert(
                fragment.event_id,
                Reassembly {
                    version: fragment.version,
                    total: fragment.total,
                    total_len,
                    hash: fragment.hash,
                    chunks: vec![None; expected_total],
                    started: now,
                },
            );
        }

        let Some(entry) = self.fragments.get(&fragment.event_id) else {
            return FragmentOutcome::Bad;
        };
        let mismatched = entry.version != fragment.version
            || entry.total != fragment.total
            || entry.total_len != total_len
            || entry.hash != fragment.hash;
        if mismatched {
            self.fragments.remove(&fragment.event_id);
            return FragmentOutcome::Bad;
        }
        let slot = usize::from(fragment.index);
        let chunk_state = match &entry.chunks[slot] {
            Some(existing) if existing == &fragment.chunk => 1,
            Some(_) => 2,
            None => 0,
        };
        match chunk_state {
            1 => return FragmentOutcome::Pending,
            2 => {
                self.fragments.remove(&fragment.event_id);
                return FragmentOutcome::Bad;
            }
            _ => {}
        }

        let Some(entry) = self.fragments.get_mut(&fragment.event_id) else {
            return FragmentOutcome::Bad;
        };
        entry.chunks[slot] = Some(fragment.chunk);
        if entry.chunks.iter().any(Option::is_none) {
            return FragmentOutcome::Pending;
        }

        let entry = self
            .fragments
            .remove(&fragment.event_id)
            .expect("checked above");
        let mut text = Vec::with_capacity(entry.total_len);
        for chunk in entry.chunks.into_iter().flatten() {
            text.extend_from_slice(&chunk);
        }
        if text.len() != entry.total_len {
            return FragmentOutcome::Bad;
        }
        let hash = content_hash_bytes(&text);
        if hash != entry.hash {
            return FragmentOutcome::Bad;
        }
        let Ok(text) = String::from_utf8(text) else {
            return FragmentOutcome::Bad;
        };
        let event = StoredEvent {
            event_id: fragment.event_id,
            version: entry.version,
            text,
            hash,
        };
        match self.apply_event(sender, &event, now) {
            Apply::Invalid => FragmentOutcome::Bad,
            other => FragmentOutcome::Done(other),
        }
    }

    pub(crate) fn observe_beacon(&mut self, source: SocketAddr, beacon: Beacon, now: Instant) {
        if beacon.instance_id == self.instance_id {
            return;
        }
        let addr = Beacon::peer_addr(source, beacon.tcp_port);
        let changed = self.discovered.get(&beacon.instance_id).is_none_or(|old| {
            old.addr != addr
                || old.fingerprint != beacon.fingerprint
                || old.capabilities != beacon.capabilities
        });
        self.discovered.insert(
            beacon.instance_id,
            Discovered {
                addr,
                fingerprint: beacon.fingerprint,
                capabilities: beacon.capabilities,
                last_seen: now,
            },
        );
        if changed {
            let info = self.peer_info_for(
                beacon.instance_id,
                addr,
                beacon.fingerprint,
                beacon.capabilities,
            );
            self.push_event(ClipEvent::PeerDiscovered(info));
        }
    }

    pub(crate) fn prune(&mut self, now: Instant) {
        self.prune_fragments(now);
        self.seen_events
            .retain(|_, at| now.duration_since(*at) < SEEN_EVENT_TTL);
        self.trim_seen();
        self.remote_hashes
            .retain(|_, at| now.duration_since(*at) < HASH_SUPPRESS_TTL);
        self.pair_rate
            .retain(|_, at| now.duration_since(*at) < Duration::from_secs(60));

        let expired: Vec<_> = self
            .discovered
            .iter()
            .filter(|(_, peer)| now.duration_since(peer.last_seen) >= PEER_EXPIRY)
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            self.discovered.remove(&id);
            self.push_event(ClipEvent::PeerOffline { instance_id: id });
        }
    }

    pub(crate) fn peer_snapshot(&self) -> Vec<PeerInfo> {
        let mut peers: Vec<PeerInfo> = self
            .discovered
            .iter()
            .map(|(id, peer)| {
                self.peer_info_for(*id, peer.addr, peer.fingerprint, peer.capabilities)
            })
            .collect();
        for (id, session) in &self.sessions {
            if !self.discovered.contains_key(id) {
                let fingerprint = self
                    .trust
                    .get(*id)
                    .map_or(Fingerprint::from_bytes([0; 8]), |peer| peer.fingerprint);
                peers.push(self.peer_info_for(*id, session.addr, fingerprint, 0));
            }
        }
        peers.sort_by_key(|peer| peer.instance_id);
        peers
    }

    fn peer_info_for(
        &self,
        id: InstanceId,
        addr: SocketAddr,
        fingerprint: Fingerprint,
        capabilities: u8,
    ) -> PeerInfo {
        let trusted = self.trust.get(id);
        PeerInfo {
            instance_id: id,
            name: trusted.map_or_else(String::new, |peer| peer.name.clone()),
            address: addr,
            fingerprint,
            capabilities,
            trusted: trusted.is_some(),
            connected: self.sessions.contains_key(&id),
        }
    }

    pub(crate) fn pending_for_session(&self) -> Option<StoredEvent> {
        self.pending_local
            .as_ref()
            .filter(|pending| self.current == Some(pending.version))
            .cloned()
    }

    pub(crate) fn trust_peer(&mut self, peer: TrustedPeer) -> Result<(), ClipError> {
        let previous = self.trust.get(peer.instance_id).cloned();
        self.trust.insert(peer.clone());
        if let Err(error) = self.trust.save(&self.trust_path) {
            match previous {
                Some(previous) => self.trust.insert(previous),
                None => {
                    self.trust.remove(peer.instance_id);
                }
            }
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn forget_peer(&mut self, id: InstanceId) -> Result<(), ClipError> {
        let Some(previous) = self.trust.get(id).cloned() else {
            return Ok(());
        };
        self.trust.remove(id);
        if let Err(error) = self.trust.save(&self.trust_path) {
            self.trust.insert(previous);
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn clear_queues(&mut self) {
        self.events.clear();
        self.fragments.clear();
        self.seen_events.clear();
        self.remote_hashes.clear();
        self.pending_local = None;
        self.current = None;
        self.sessions.clear();
        self.pairing.clear();
        self.connecting.clear();
        self.discovered.clear();
        self.backoff.clear();
        self.pairing_attempts.clear();
        self.pair_rate.clear();
    }

    fn remember_event(&mut self, id: crate::types::EventId, now: Instant) {
        self.seen_events.insert(id, now);
        self.trim_seen();
    }

    fn trim_seen(&mut self) {
        trim_oldest(&mut self.seen_events, MAX_SEEN);
    }

    fn trim_hashes(&mut self) {
        trim_oldest(&mut self.remote_hashes, MAX_REMOTE_HASHES);
    }

    fn prune_fragments(&mut self, now: Instant) {
        self.fragments
            .retain(|_, entry| now.duration_since(entry.started) < FRAGMENT_EXPIRY);
    }

    fn reassembly_bytes(&self) -> usize {
        self.fragments.values().map(|entry| entry.total_len).sum()
    }
}

#[cfg(test)]
mod tests;
