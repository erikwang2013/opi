// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 前缀区间的二分下界（2026-10-22 从 `loader.rs` 拆出 —— 那边 500 行门禁，
//! 加 `visit_prefix` 覆写需要空位；与 `candidates_tests.rs` 同一套拆分先例）。
//!
//! 只读表头给出的三段布局，不物化条目：`entry_pinyin` 返回拼音区段里的字节切片。

use crate::format::ENTRY_LEN;

/// 第 i 条记录的 pinyin 字节。
pub(super) fn entry_pinyin(
    data: &[u8],
    table_start: usize,
    pinyin_start: usize,
    i: usize,
) -> &[u8] {
    let off = table_start + i * ENTRY_LEN;
    let po = u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as usize;
    let pl = data[off + 4] as usize;
    &data[pinyin_start + po..pinyin_start + po + pl]
}

/// 前缀区间下界：第一个 pinyin >= needle 的下标。
pub(super) fn lower_bound(
    data: &[u8],
    table_start: usize,
    pinyin_start: usize,
    count: usize,
    needle: &[u8],
) -> usize {
    let mut lo = 0usize;
    let mut hi = count;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if entry_pinyin(data, table_start, pinyin_start, mid) < needle {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}
