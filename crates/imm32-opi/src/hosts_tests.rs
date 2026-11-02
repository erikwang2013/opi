// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `HostMap` 的主机测试：多 HIMC 隔离、生命周期清理、候选窗归属、上限淘汰。
//! 用真的 `ImmSession`（内置回退词库，毫秒级），不碰 Windows。

use tsf_opi::logic::TsfLogic;

use super::*;
use crate::session::ImmSession;

fn session() -> ImmSession {
    ImmSession::new(TsfLogic::load(None).expect("fallback dict"))
}

#[test]
fn distinct_himcs_do_not_share_composition() {
    let mut hosts: HostMap<ImmSession> = HostMap::new(8);

    let a = hosts.get_or_insert_with(0x1001, session);
    a.process('n' as u32, 0);
    a.process('i' as u32, 0);
    assert_eq!(a.logic().buffer(), "ni");

    // 另一个 HIMC 是全新会话：既看不到 A 的组合，打自己的字也不影响 A。
    let b = hosts.get_or_insert_with(0x1002, session);
    assert_eq!(b.logic().buffer(), "", "B 不得继承 A 的组合串");
    b.process('h' as u32, 0);
    assert_eq!(b.logic().buffer(), "h");

    assert_eq!(hosts.get_mut(0x1001).unwrap().logic().buffer(), "ni");
    assert_eq!(hosts.get_mut(0x1002).unwrap().logic().buffer(), "h");
}

#[test]
fn same_himc_keeps_its_state() {
    let mut hosts: HostMap<ImmSession> = HostMap::new(8);
    hosts.get_or_insert_with(7, session).process('n' as u32, 0);
    assert_eq!(hosts.get_or_insert_with(7, session).logic().buffer(), "n");
    assert_eq!(hosts.len(), 1, "同一 HIMC 只建一次");
}

#[test]
fn pending_fields_are_per_himc() {
    #[derive(Default)]
    struct Pending {
        states: u32,
        hotkey: u8,
    }

    let mut hosts: HostMap<Pending> = HostMap::new(8);
    hosts.get_or_insert_with(1, Pending::default).states = 0x8000_0000;
    hosts.get_or_insert_with(2, Pending::default).hotkey = 3;

    let one = hosts.get_mut(1).unwrap();
    assert_eq!((one.states, one.hotkey), (0x8000_0000, 0));
    let two = hosts.get_mut(2).unwrap();
    assert_eq!((two.states, two.hotkey), (0, 3));
}

#[test]
fn remove_clears_only_that_himc_and_active() {
    let mut hosts: HostMap<ImmSession> = HostMap::new(8);
    hosts.get_or_insert_with(1, session);
    hosts.get_or_insert_with(2, session);
    hosts.set_active(Some(1));

    assert!(hosts.remove(1).is_some());
    assert_eq!(hosts.active(), None, "ImeSelect(FALSE) 顺带清焦点");
    assert!(hosts.get_mut(1).is_none());
    assert!(hosts.get_mut(2).is_some(), "其他 HIMC 不受影响");
}

#[test]
fn capacity_evicts_least_recently_used_non_active() {
    let mut hosts: HostMap<u32> = HostMap::new(2);
    hosts.get_or_insert_with(1, || 10);
    hosts.get_or_insert_with(2, || 20);
    hosts.get_mut(1); // 1 变最近使用
    hosts.get_or_insert_with(3, || 30); // 淘汰 2

    assert_eq!(hosts.len(), 2);
    assert_eq!(hosts.get_mut(1).copied(), Some(10));
    assert!(hosts.get_mut(2).is_none());
    assert_eq!(hosts.get_mut(3).copied(), Some(30));
}

#[test]
fn active_host_is_never_evicted() {
    let mut hosts: HostMap<u32> = HostMap::new(2);
    hosts.get_or_insert_with(1, || 10);
    hosts.get_or_insert_with(2, || 20);
    hosts.set_active(Some(1)); // 1 是最久未用的，但正持有焦点
    hosts.get_or_insert_with(3, || 30);

    assert_eq!(hosts.get_mut(1).copied(), Some(10), "焦点里的组合不能丢");
    assert!(hosts.get_mut(2).is_none());
    assert_eq!(hosts.len(), 2);
}

#[test]
fn candidate_window_follows_active_context() {
    let mut hosts: HostMap<u32> = HostMap::new(8);
    assert!(hosts.owns_candidates(1), "没有焦点记录时退回旧行为");
    hosts.set_active(Some(1));
    assert!(hosts.owns_candidates(1));
    assert!(!hosts.owns_candidates(2), "后台 HIMC 不得抢候选窗");
    hosts.set_active(None);
    assert!(hosts.owns_candidates(2));
}
