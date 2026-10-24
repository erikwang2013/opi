// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! C ABI 出口的**自定义短语**一组（2026-10-23，从 `cabi.rs` 拆出的同套先例：
//! 那个文件 411 行，但导出面按族分文件是既有结构，别把这块塞回去）。
//!
//! 语义与 JNI 出口（`jni_phrases.rs`）逐条一致，共享 `api::SINGLETON`；JSON 格式、
//! 校验与匹配全在 `engine_core::custom_phrases`。
//!
//! ⚠️ 拆出去**不等于脱离契约**：`tests/c_abi_contract.rs` 读本模块（`checker::read_cabi()`），
//! `tests/cabi_safety_doc.rs` 的替代执行点也扫全部 cabi 文件。

#![allow(clippy::missing_safety_doc)]
// 同上：按模块生效，拆出来必须再写一次（替代执行点 = cabi_safety_doc.rs）。

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::api;
use crate::cabi::OpiString;
use crate::cabi::utf16::read_utf16;

/// setCustomPhrases(json: const uint16_t*, len) -> int32_t。返回导入条数；
/// 失败返回 -1（非法 JSON / 版本不符 / 校验不过 / 引擎未装载 / null 入参），
/// 且**不改动**既有短语表（引擎侧全有或全无）。
/// # Safety
/// `ptr` 必须指向至少 `len` 个有效 `u16`（或为 null，视为失败）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_custom_phrases(ptr: *const u16, len: usize) -> i32 {
    catch_unwind(AssertUnwindSafe(|| -> i32 {
        let Some(json) = (unsafe { read_utf16(ptr, len) }) else {
            return -1;
        };
        match api::with_engine(|e| e.set_custom_phrases(json)) {
            // 条数上限 2000，必然装得下 i32；try_from 只是不给理论溢出留 panic 口子。
            Some(Ok(n)) => i32::try_from(n).unwrap_or(-1),
            Some(Err(_)) | None => -1,
        }
    }))
    .unwrap_or(-1)
}

/// exportCustomPhrases() -> OpiString。当前短语表 JSON（schema 见模块头）。
/// 未装载 → 空句柄（调用方可无条件释放）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_export_custom_phrases() -> OpiString {
    let out = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.export_custom_phrases()).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpiString::from_utf16(&out)
}

/// setCustomPhrasesEnabled(on: bool)。缺省为关（2026-10-25 用户裁决）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_custom_phrases_enabled(on: bool) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.set_custom_phrases_enabled(on))
    }));
}

/// customPhrasesEnabled() -> bool。未装载 → false（哨兵，同其余开关读侧）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_custom_phrases_enabled() -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.custom_phrases_enabled()).unwrap_or(false)
    }))
    .unwrap_or(false)
}

// ---- 英文套（2026-10-24）：与上四条逐条同义，操作的是独立的英文表/英文开关。 ----

/// setEnglishCustomPhrases(json: const uint16_t*, len) -> int32_t。英文套导入，
/// 返回值/失败语义与 [`opi_set_custom_phrases`] 完全一致。
/// # Safety
/// `ptr` 必须指向至少 `len` 个有效 `u16`（或为 null，视为失败）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_english_custom_phrases(ptr: *const u16, len: usize) -> i32 {
    catch_unwind(AssertUnwindSafe(|| -> i32 {
        let Some(json) = (unsafe { read_utf16(ptr, len) }) else {
            return -1;
        };
        match api::with_engine(|e| e.set_english_custom_phrases(json)) {
            Some(Ok(n)) => i32::try_from(n).unwrap_or(-1),
            Some(Err(_)) | None => -1,
        }
    }))
    .unwrap_or(-1)
}

/// exportEnglishCustomPhrases() -> OpiString。英文套 JSON；未装载 → 空句柄。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_export_english_custom_phrases() -> OpiString {
    let out = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.export_english_custom_phrases()).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpiString::from_utf16(&out)
}

/// setEnglishCustomPhrasesEnabled(on: bool)。英文套开关；缺省为关。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_english_custom_phrases_enabled(on: bool) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.set_english_custom_phrases_enabled(on))
    }));
}

/// englishCustomPhrasesEnabled() -> bool。未装载 → false（哨兵）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_english_custom_phrases_enabled() -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.english_custom_phrases_enabled()).unwrap_or(false)
    }))
    .unwrap_or(false)
}
