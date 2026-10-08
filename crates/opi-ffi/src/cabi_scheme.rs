// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! C ABI 出口的**笔画词典 / 输入方案**一组（从 `cabi.rs` 拆出：那个文件有 500 行硬
//! 门禁，加「输入方案」的出口后越线 —— 与 `cabi_user_words.rs` 同一个拆分先例）。
//!
//! ⚠️ 拆出去**不等于脱离契约**：`tests/c_abi_contract.rs` 的门禁读本模块**全部** .rs
//! （`checker::read_cabi()`，本次已把本文件加进那份合成文本）—— 漏加就是这一批
//! **静默**不再被比对。
//!
//! 语义与 JNI 侧（`jni_scheme.rs`）逐条一致，共享 `api::SINGLETON` 与
//! `api::scheme_from_int` / `scheme_to_int`（整数表的唯一定义处）。

#![allow(clippy::missing_safety_doc)]
// ⚠️ 同 `cabi.rs` 顶上那条：按模块生效，拆出来要在本文件里再写一次（本文件里
// `opi_set_scheme` / `opi_scheme` 没有裸指针入参，没有这条就会吃 missing_safety_doc）。

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::api;
use crate::cabi::utf16::read_utf16;

/// 笔画词典（stroke.opid，2026-10-09）。空/坏路径/引擎未装载 → false（笔画方案
/// 回退普通路径，与繁体那条「回退」姿态相同）。
/// # Safety
/// `ptr` 必须指向至少 `len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_load_stroke(path: *const u16, len: usize) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        let path = unsafe { read_utf16(path, len) }.unwrap_or_default();
        api::install_stroke(&path).is_ok()
    }))
    .unwrap_or(false)
}

/// setScheme(scheme: int32)：输入方案（2026-10-09，移动端「输入模式」的引擎侧）。
/// 取值：0=全键盘 1=拼音9键 2=拼音14键 3=笔画；**越界忽略**（与 `opi_switch_mode`
/// 同一条边界约定）。⚠️ **换方案清缓冲** —— 调用后须按 `opi_switch_mode` 那一档
/// 重读 buffer / 候选 / 页码。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_scheme(scheme: i32) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| {
            if let Some(s) = api::scheme_from_int(scheme) {
                e.set_scheme(s);
            }
        })
    }));
}

/// scheme() -> int32：输入方案的读侧（同 [`opi_set_scheme`] 的整数表）。
/// 哨兵与 `opi_fuzzy_enabled` 同档：**引擎未装载 → 0（全键盘）** —— 那是对
/// 「没引擎时什么方案都谈不上」的表达，别把它读成"用户选的就是全键盘"。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_scheme() -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| api::scheme_to_int(e.scheme())).unwrap_or(0)
    }))
    .unwrap_or(0)
}
