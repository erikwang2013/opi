// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! JNI 出口的**学习 / 用户词**一组（从 `jni.rs` 拆出：那个文件 499 行，加「属性设置」
//! 的两个开关就会越 500 行门禁；与 C ABI 侧的 `cabi_user_words.rs` 同一次拆分、同一主题）。
//!
//! ⚠️ 拆出去**不等于脱离契约**：`tests/jni_contract.rs` 已改成读本模块**全部** .rs
//! （注册表在 `jni.rs`，函数体在这里 —— 两侧都要在文本里，门禁才看得全）。
//! `guard` / `JEnv` 仍是 `jni.rs` 的（改为 pub(crate)），语义与 C ABI 同源。

#![allow(clippy::missing_safety_doc)]
// ⚠️ 同 `jni.rs` 顶上那条：按模块生效，拆出来要在本文件里再写一次。

use std::panic::{AssertUnwindSafe, catch_unwind};

use jni::sys::{self, jboolean, jint, jstring};

use crate::api;
use crate::jni::{JEnv, guard};
use crate::jni_util;

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_learner_enabled(_env: JEnv, _class: sys::jclass) -> jboolean {
    guard(|| api::with_engine(|e| e.learner_enabled()).unwrap_or_default())
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_set_learner(
    _env: JEnv,
    _class: sys::jclass,
    enabled: jboolean,
) {
    guard(|| api::with_engine(|e| e.set_learner(enabled)));
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_clear_user_words(_env: JEnv, _class: sys::jclass) {
    guard(|| api::with_engine(|e| e.clear_user_words()));
}

/// removeUserWord(text: String)。长按删词（B4）。词不存在 / null / 空串 → 无操作。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_remove_user_word(
    env: JEnv,
    _class: sys::jclass,
    text: jstring,
) {
    guard(|| {
        let text = unsafe { jni_util::jstring_to_rust(env, text) }.unwrap_or_default();
        api::with_engine(|e| e.remove_user_word(text));
    });
}

/// importUserWords(json: String) -> Int。返回导入条数；
/// 负数表示失败（非法 JSON / 版本不符 / 引擎未装载 / null）且不改动既有用户词。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_import_user_words(
    env: JEnv,
    _class: sys::jclass,
    json: jstring,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| -> jint {
        let Some(json) = (unsafe { jni_util::jstring_to_rust(env, json) }) else {
            return -1;
        };
        match api::with_engine(|e| e.import_user_words(json)) {
            Some(Ok(n)) => i32::try_from(n).unwrap_or(-1),
            Some(Err(_)) | None => -1,
        }
    }))
    .unwrap_or(-1)
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_export_user_words(env: JEnv, _class: sys::jclass) -> jstring {
    let out = guard(|| api::with_engine(|e| e.export_user_words()).unwrap_or_default());
    unsafe { jni_util::rust_to_jstring(env, &out) }
}
