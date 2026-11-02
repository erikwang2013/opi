// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 分页窗口与全量排名的前缀契约（从 router_tests.rs 拆出守 500 行）。

use super::*;

/// 分页只取 `(page+1)*PAGE_SIZE`，但必须与全量列表逐条同前缀（同 `select` 的全局下标）。
#[test]
fn paged_candidates_are_prefixes_of_the_full_ranking() {
    let mut s = paging_state();
    for ch in "qwx".chars() {
        s.key_event(ch as u32, 0);
    }
    let full: Vec<String> = s
        .engine()
        .candidates(usize::MAX)
        .into_iter()
        .map(|c| c.text)
        .collect();
    assert_eq!(full.len(), 20);
    for page in 0..3 {
        let start = page * PAGE_SIZE;
        let end = (start + PAGE_SIZE).min(full.len());
        assert_eq!(s.candidates(), full[start..end]);
        if page < 2 {
            s.key_event(KEY_PAGE_DOWN, 0);
        }
    }
}
