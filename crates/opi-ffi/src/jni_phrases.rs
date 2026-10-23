// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! JNI 出口的**自定义短语**一组（2026-10-23）。与 C ABI 面（`cabi_phrases.rs`）语义
//! 逐条一致，共享 `api::SINGLETON`；JSON 格式/校验/匹配在 `engine_core::custom_phrases`。
//!
//! ⚠️ 注册表在 `jni.rs` 的 `JNI_OnLoad`，本文件导出 `custom_phrase_native_methods()`
//! 返回这四条（先例：`jni_wubi.rs` 的 `wubi_native_methods()`）—— 数组没有 extend，
//! 那边先转 Vec 再并。**少一条声明 = 整张注册表失败**（宿主面见
//! `android/.../jni/OpiEngine.kt` 与 `android/jni_smoke/Main.java`）。

#![allow(clippy::missing_safety_doc)]
// 同 `jni.rs` 顶上那条：按模块生效，拆出来要再写一次。

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

use jni::NativeMethod;
use jni::jni_str;
use jni::sys::{self, jboolean, jint, jstring};

use crate::api;
use crate::jni::{JEnv, guard};
use crate::jni_util;

/// setCustomPhrases(json: String) -> Int。返回导入条数；
/// 负数表示失败（非法 JSON / 版本不符 / 校验不过 / null）且不改动既有短语表。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_set_custom_phrases(
    env: JEnv,
    _class: sys::jclass,
    json: jstring,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| -> jint {
        let Some(json) = (unsafe { jni_util::jstring_to_rust(env, json) }) else {
            return -1;
        };
        match api::with_engine(|e| e.set_custom_phrases(json)) {
            Some(Ok(n)) => i32::try_from(n).unwrap_or(-1),
            Some(Err(_)) | None => -1,
        }
    }))
    .unwrap_or(-1)
}

/// exportCustomPhrases() -> String。当前短语表 JSON；未装载 → 空串。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_export_custom_phrases(
    env: JEnv,
    _class: sys::jclass,
) -> jstring {
    let out = guard(|| api::with_engine(|e| e.export_custom_phrases()).unwrap_or_default());
    unsafe { jni_util::rust_to_jstring(env, &out) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_set_custom_phrases_enabled(
    _env: JEnv,
    _class: sys::jclass,
    on: jboolean,
) {
    guard(|| api::with_engine(|e| e.set_custom_phrases_enabled(on)));
}

/// customPhrasesEnabled() -> Boolean。未装载 → false。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_custom_phrases_enabled(
    _env: JEnv,
    _class: sys::jclass,
) -> jboolean {
    guard(|| api::with_engine(|e| e.custom_phrases_enabled()).unwrap_or_default())
}

/// 注册表条目（`jni.rs` 用 `methods.extend(...)` 并入）。名字/描述符必须与
/// `OpiEngine.kt` 的 `external fun` 逐字一致 —— 契约门禁：`tests/jni_contract.rs`。
pub(crate) fn custom_phrase_native_methods() -> Vec<NativeMethod<'static>> {
    unsafe {
        vec![
            NativeMethod::from_raw_parts(
                jni_str!("setCustomPhrases"),
                jni_str!("(Ljava/lang/String;)I"),
                opijni_set_custom_phrases as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("exportCustomPhrases"),
                jni_str!("()Ljava/lang/String;"),
                opijni_export_custom_phrases as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("setCustomPhrasesEnabled"),
                jni_str!("(Z)V"),
                opijni_set_custom_phrases_enabled as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("customPhrasesEnabled"),
                jni_str!("()Z"),
                opijni_custom_phrases_enabled as *mut c_void,
            ),
        ]
    }
}
