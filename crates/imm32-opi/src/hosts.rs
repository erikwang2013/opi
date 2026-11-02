// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 按 HIMC 键控的 Host 表（平台中立，Linux 可单测）。
//!
//! 旧实现是 `Mutex<Option<Host>>`：进程里所有输入上下文共用一个会话，切窗口会
//! 串组合。这里每个 HIMC 一个槽位；候选窗只有一个，归属由 [`HostMap::active`]
//! 决定（见 `owns_candidates`），所以窗口仍只跟随当前焦点。
//!
//! 上限防的是「每个 HIMC 都重装导入词库/码表」这类按槽位计的成本（主词库已由
//! `dict.rs` 进程级共享）；正常路径靠 `ImeSelect(FALSE)` 移除，不会走到淘汰。

use std::collections::HashMap;

struct Slot<H> {
    host: H,
    used: u64,
}

pub(crate) struct HostMap<H> {
    hosts: HashMap<isize, Slot<H>>,
    active: Option<isize>,
    cap: usize,
    tick: u64,
}

impl<H> HostMap<H> {
    pub(crate) fn new(cap: usize) -> Self {
        Self {
            hosts: HashMap::new(),
            active: None,
            cap: cap.max(1),
            tick: 0,
        }
    }

    fn next_tick(&mut self) -> u64 {
        self.tick += 1;
        self.tick
    }

    /// 取一个槽位并刷新 LRU 时间戳；不创建。
    pub(crate) fn get_mut(&mut self, himc: isize) -> Option<&mut H> {
        let tick = self.next_tick();
        let slot = self.hosts.get_mut(&himc)?;
        slot.used = tick;
        Some(&mut slot.host)
    }

    /// 取或建。`build` 只在缺槽位时调用一次。
    pub(crate) fn get_or_insert_with(&mut self, himc: isize, build: impl FnOnce() -> H) -> &mut H {
        let tick = self.next_tick();
        if !self.hosts.contains_key(&himc) {
            self.evict_for(himc);
            self.hosts.insert(
                himc,
                Slot {
                    host: build(),
                    used: tick,
                },
            );
        }
        let slot = self.hosts.get_mut(&himc).expect("slot just inserted");
        slot.used = tick;
        &mut slot.host
    }

    /// 淘汰最久未用、且**不是**当前焦点/本次键所属的槽位。当前焦点的 Host
    /// 永不淘汰：那个槽位上的组合串正显示在候选窗里。全都不可淘汰时宁可超限
    /// 一个（同时只能有一个 active，故最多 +1），也不丢焦点中的状态。
    fn evict_for(&mut self, keep: isize) {
        while self.hosts.len() >= self.cap {
            let victim = self
                .hosts
                .iter()
                .filter(|(k, _)| **k != keep && Some(**k) != self.active)
                .min_by_key(|(_, s)| s.used)
                .map(|(k, _)| *k);
            let Some(victim) = victim else { break };
            self.hosts.remove(&victim);
        }
    }

    pub(crate) fn remove(&mut self, himc: isize) -> Option<H> {
        if self.active == Some(himc) {
            self.active = None;
        }
        self.hosts.remove(&himc).map(|s| s.host)
    }

    pub(crate) fn set_active(&mut self, himc: Option<isize>) {
        self.active = himc;
    }

    pub(crate) fn active(&self) -> Option<isize> {
        self.active
    }

    /// 候选窗归属：有焦点记录时只归当前焦点；没有记录（宿主不发
    /// `ImeSetActiveContext`）时退回旧行为 —— 有动作就显示。
    pub(crate) fn owns_candidates(&self, himc: isize) -> bool {
        self.active.map_or(true, |a| a == himc)
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.hosts.len()
    }
}

#[cfg(test)]
#[path = "hosts_tests.rs"]
mod tests;
