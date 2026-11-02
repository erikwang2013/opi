// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 发现表/事件队列的边界（从 state.rs 拆出守 500 行）。

use super::*;

use std::net::SocketAddr;
use std::time::Instant;

use super::Discovered;
use crate::discovery::Beacon;
use crate::types::ClipEvent;

impl Core {
    pub(crate) fn push_event(&mut self, event: ClipEvent) {
        if self.events.len() >= MAX_EVENTS {
            // 宁可丢一条过期的发现通知，也不丢剪贴板/配对内容事件。
            if let Some(pos) = self
                .events
                .iter()
                .position(|e| matches!(e, ClipEvent::PeerDiscovered(_)))
            {
                self.events.remove(pos);
            } else {
                self.events.pop_front();
            }
        }
        self.events.push_back(event);
    }

    pub(crate) fn observe_beacon(&mut self, source: SocketAddr, beacon: Beacon, now: Instant) {
        if beacon.instance_id == self.instance_id {
            return;
        }
        if !self.discovered.contains_key(&beacon.instance_id)
            && self.discovered.len() >= MAX_DISCOVERED
        {
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
}
