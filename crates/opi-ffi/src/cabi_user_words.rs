// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! C ABI 出口的**学习 / 用户词**一组（从 `cabi.rs` 拆出：那个文件有 500 行硬门禁，
//! 加「属性设置」的出口后越线 —— 与 `cabi/utf16.rs` 同一个拆分先例）。
//!
//! ⚠️ 拆出去**不等于脱离契约**：`tests/c_abi_contract.rs` 的门禁已改成读本模块的
//! **全部** .rs（`checker::read_cabi()`）—— 只读 cabi.rs 的那一版会让这一批**静默**
//! 不再被比对（本项目栽过：门禁只证被它覆盖到的那部分）。
//!
//! 语义与 JNI 出口（`jni.rs` / `jni_user_words.rs`）逐条一致，共享 `api::SINGLETON`。

#![allow(clippy::missing_safety_doc)]
// ⚠️ 上面那条 `allow` 与 `cabi.rs` 顶上那条是同一件事、同样的范围（按模块生效）：
// 拆出来之后**必须在本文件里再写一次**，否则本文件里那些没有裸指针入参的导出
// （如 `opi_learner_enabled`）会开始吃 clippy 的 `missing_safety_doc`。
// 替代执行点仍是 `tests/cabi_safety_doc.rs`（它按**文本**算「带指针的导出必须自带
// `# Safety` 段」，与这条 allow 的范围互补）。

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::api;
use crate::cabi::OpiString;
use crate::cabi::utf16::read_utf16;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_learner_enabled() -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.learner_enabled()).unwrap_or(false)
    }))
    .unwrap_or(false)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_learner(enabled: bool) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.set_learner(enabled))
    }));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clear_user_words() {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.clear_user_words())
    }));
}

/// remove_user_word(text: const uint16_t*, len)。长按删词的 C ABI 出口
/// （镜像 jni.rs 的 opijni_remove_user_word，两个 ABI 面语义必须一致）。
/// 词不存在 / 空串 / null → 无操作；导出 JSON 逐字节不变。
/// # Safety
/// `ptr` 必须指向至少 `len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_remove_user_word(ptr: *const u16, len: usize) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let text = unsafe { read_utf16(ptr, len) }.unwrap_or_default();
        api::with_engine(|e| e.remove_user_word(text));
    }));
}

/// import_user_words(json: const uint16_t*, len) -> i32。
/// 返回导入条数；失败返回负值（-1）——非法 JSON / 版本不符 / 词表过大 /
/// 引擎未装载 / null 入参。失败不改动既有用户词（引擎侧「全有或全无」）。
/// # Safety
/// `ptr` 必须指向至少 `len` 个有效 `u16`（或为 null，视为失败）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_import_user_words(ptr: *const u16, len: usize) -> i32 {
    catch_unwind(AssertUnwindSafe(|| -> i32 {
        // null 与空串一律失败：空串不是合法 JSON，无需特判成空词表。
        let Some(json) = (unsafe { read_utf16(ptr, len) }) else {
            return -1;
        };
        match api::with_engine(|e| e.import_user_words(json)) {
            // 条数上限 MAX_IMPORT_WORDS = 100_000，必然装得下 i32；
            // try_from 只是不给理论溢出留 unwrap panic 的口子。
            Some(Ok(n)) => i32::try_from(n).unwrap_or(-1),
            Some(Err(_)) | None => -1,
        }
    }))
    .unwrap_or(-1)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_export_user_words() -> OpiString {
    let out = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.export_user_words()).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpiString::from_utf16(&out)
}
