// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `state.rs` 的小工具；拆出来只为守住 500 行源码门禁。

use std::collections::HashMap;
use std::hash::Hash;

pub(crate) fn fragment_count(total_len: usize, fragment_bytes: usize) -> usize {
    if total_len == 0 {
        1
    } else {
        total_len.div_ceil(fragment_bytes)
    }
}

pub(crate) fn trim_oldest<K: Hash + Eq + Copy, V: Ord>(map: &mut HashMap<K, V>, max: usize) {
    while map.len() > max {
        let Some(oldest) = map.iter().min_by_key(|(_, v)| *v).map(|(k, _)| *k) else {
            break;
        };
        map.remove(&oldest);
    }
}
