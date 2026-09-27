// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `cabi.rs` 的 UTF-16 编解码与句柄实现 —— **不是**导出面，`cabi.rs` 仍是唯一一份
//! C 导出清单（`c_abi_contract.rs` 只解析 `cabi.rs` 的文本，导出与 `#[repr(C)]`
//! 结构体**不得**挪走，否则那道门禁看不见它们）。
//!
//! 拆出来的理由只有一个：`cabi.rs` 顶到 500 行上限（门禁 `tests/line_limit.rs` ——
//! 按文件系统全量扫、上限写死 500），三个 B3/B5 出口加不进去。这里放的是**非导出**
//! 辅助（impl 与两个自由函数），拆走不改变 ABI 面。

use super::OpiString;

impl OpiString {
    /// 从 &str 分配 UTF-16 缓冲。空串返回空句柄（ptr 为 null）。
    /// 用 into_boxed_slice 使分配布局精确等于 len，free 端
    /// `Vec::from_raw_parts(ptr, len, len)` 的释放布局与之匹配，无 UB。
    pub fn from_utf16(s: &str) -> Self {
        if s.is_empty() {
            return Self::empty();
        }
        let units: Box<[u16]> = s.encode_utf16().collect::<Vec<u16>>().into_boxed_slice();
        let ptr = units.as_ptr();
        let len = units.len();
        std::mem::forget(units);
        OpiString { ptr, len }
    }

    /// 空句柄（ptr: null, len: 0）——错误/空串哨兵。
    pub fn empty() -> Self {
        OpiString {
            ptr: std::ptr::null(),
            len: 0,
        }
    }
}

/// 读取 UTF-16 输入串。`None` **只**表示「非法 UTF-16」；null/零长 → `Some("")`。
///
/// # Safety
/// `ptr` 必须指向至少 `len` 个 u16 的有效内存（或为 null）。
pub(super) unsafe fn read_utf16(ptr: *const u16, len: usize) -> Option<String> {
    if ptr.is_null() || len == 0 {
        return Some(String::new());
    }
    // Safety: 调用方保证 ptr 指向至少 len 个 u16 的有效内存
    let units = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf16(units).ok()
}

/// 共享文本数组 → JSON 字符串（OpiString）。
pub(super) fn texts_to_json(texts: Vec<String>) -> OpiString {
    OpiString::from_utf16(&crate::api::texts_json(&texts))
}
