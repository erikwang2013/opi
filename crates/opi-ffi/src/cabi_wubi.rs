// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **五笔旋钮一族**的 C 出口（M4 第二波，2026-10-14；与 `jni_wubi.rs` 镜像 ——
//! 两轨族边界一致是本仓纪律）。
//!
//! ⚠️ 拆出去**不等于脱离契约**：`tests/c_abi/checker.rs` 的 `read_cabi()` 必须把本文件
//! 一起读（否则头文件比对会报一堆假「多出」）；`cabi.rs` 顶部 `pub use` 保持旧路径
//! （`opi_ffi::cabi::opi_load_wubi` 等调用方与测试一行不改）。
//!
//! 整数码与哨兵语义见 `api/wubi.rs` 的头注（写侧越界忽略 / 个数钳区间 / 未装载 →
//! false / 0 / 空句柄）。

#![allow(clippy::missing_safety_doc)]
// ⚠️ 同 `cabi.rs` 顶上那条：按模块生效。

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::api;
use crate::cabi::OpiString;
use crate::cabi::utf16::read_utf16;

/// loadWubi(path) -> bool：装载五笔码表（`(ptr, len)` = UTF-16 路径串）。
/// 空/坏路径/引擎未装载 → false（五笔方案回退普通路径，与 `opi_load_stroke` 同姿态）。
/// # Safety
/// `ptr` 必须指向至少 `len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_load_wubi(ptr: *const u16, len: usize) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        let path = unsafe { read_utf16(ptr, len) }.unwrap_or_default();
        api::with_engine(|e| e.load_wubi(&path).is_ok()).unwrap_or(false)
    }))
    .unwrap_or(false)
}

/// enterAction() -> int32：**回车（缓冲非空时）该做什么** —— 0=选首候选、1=清空缓冲、
/// 2=上屏原文（宿主随后读 `opi_buffer()` 取原文）。判据在引擎（`Engine::enter_action`；
/// 五笔读「回车行为」设置位，非五笔 / 空缓冲恒 0）。未装载 → 0（= 各端既有行为，安全默认）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_enter_action() -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.enter_action()).unwrap_or(0)
    }))
    .unwrap_or(0)
}

/// wubiStyle() -> int32（0=纯五笔 1=五笔拼音混输）。哨兵同 `opi_scheme`：未装载 → 0。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_wubi_style() -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| api::wubi_style_to_int(e.wubi_style())).unwrap_or(0)
    }))
    .unwrap_or(0)
}

/// setWubiStyle(style)：越界忽略（写侧）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_wubi_style(style: i32) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| {
            if let Some(s) = api::wubi_style_from_int(style) {
                e.set_wubi_style(s);
            }
        })
    }));
}

/// wubiMixedEntry() -> int32（0=直接字母 1=z键）。未装载 → 0（哨兵）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_wubi_mixed_entry() -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| api::wubi_mixed_entry_to_int(e.wubi_mixed_entry())).unwrap_or(0)
    }))
    .unwrap_or(0)
}

/// setWubiMixedEntry(entry)：越界忽略。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_wubi_mixed_entry(entry: i32) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| {
            if let Some(x) = api::wubi_mixed_entry_from_int(entry) {
                e.set_wubi_mixed_entry(x);
            }
        })
    }));
}

/// wubiCandidateLimit() -> int32（1..=9）。未装载 → 0 是**哨兵**（真缺省是 5）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_wubi_candidate_limit() -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.wubi_candidate_limit() as i32).unwrap_or(0)
    }))
    .unwrap_or(0)
}

/// setWubiCandidateLimit(n)：**负数/0 归 1、过大由引擎钳到 9**（坏值钳到确定值 ——
/// 与 `opi_set_shuangpin_layout` 同一条纪律）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_wubi_candidate_limit(n: i32) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.set_wubi_candidate_limit(n.max(1) as usize));
    }));
}

/// wubiReturnStyle() -> int32（0=清空 1=上屏原文）。未装载 → 0（哨兵）。
/// ⚠️ 本档是**设置位**：消费在四端路由（见 `WubiReturnStyle` 头注），接线前无行为效果。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_wubi_return_style() -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| api::wubi_return_to_int(e.wubi_return_style())).unwrap_or(0)
    }))
    .unwrap_or(0)
}

/// setWubiReturnStyle(style)：越界忽略（写侧）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_wubi_return_style(style: i32) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| {
            if let Some(s) = api::wubi_return_from_int(style) {
                e.set_wubi_return_style(s);
            }
        })
    }));
}

/// wubiAutoCommit() -> bool（四码唯一自动上屏开关；缺省 true）。未装载 → false（哨兵）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_wubi_auto_commit() -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.wubi_auto_commit()).unwrap_or(false)
    }))
    .unwrap_or(false)
}

/// setWubiAutoCommit(on)：无返回值（设置项）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_wubi_auto_commit(on: bool) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.set_wubi_auto_commit(on));
    }));
}

/// wubiNextKeys() -> OpiString：编码提示（当前缓冲可接续的键，UTF-16）。
/// 非五笔态/无提示 → 空句柄。⚠️ 引擎侧每次调用全量重算，宿主**别在每帧渲染里调**。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_wubi_next_keys() -> OpiString {
    catch_unwind(AssertUnwindSafe(|| {
        let s = api::with_engine(|e| e.wubi_next_keys()).unwrap_or_default();
        crate::cabi::OpiString::from_utf16(&s)
    }))
    .unwrap_or_else(|_| OpiString::empty())
}
