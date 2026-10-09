// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! JNI 出口的**候选 / 缓冲读侧**一族（从 `jni.rs` 拆出：那个文件加
//! `candidateReadings` 后越 500 行门禁；与 `jni_scheme.rs` / `jni_user_words.rs`
//! 同一个拆分先例）。
//!
//! ⚠️ 拆出去**不等于脱离契约**：`tests/jni_contract.rs` 读的是本模块**全部** .rs 的
//! 合成文本（注册表仍在 `jni.rs`，函数体在这里 —— 两侧都在文本里，门禁才看得全）。
//! `guard` / `JEnv` 仍是 `jni.rs` 的（pub(crate)），语义与 C ABI 同源。

#![allow(clippy::missing_safety_doc)]
// ⚠️ 同 `jni.rs` 顶上那条：按模块生效，拆出来要在本文件里再写一次。

use jni::sys::{self, jint, jstring};

use crate::api;
use crate::jni::{JEnv, guard};
use crate::jni_util;

/// candidates(limit: Int) -> String[]。仅文本数组（kind/score UI 不用）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_candidates(
    env: JEnv,
    _class: sys::jclass,
    limit: jint,
) -> sys::jobjectArray {
    let texts = guard(|| {
        api::with_engine(|e| api::candidate_texts(e, limit.max(0) as usize)).unwrap_or_default()
    });
    unsafe { jni_util::string_array(env, texts) }
}

/// candidateReadings() -> String[]。**逐条**候选拼音读数（已分词），与最近一次
/// `candidates(limit)` 同序同源 —— 9 键左列按候选逐条显示用（2026-10-09 用户裁决
/// 「一列多个、可滑动查看」）。非 9/14 键方案、或缓冲已变（提交/退格）⇒ 空数组。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_candidate_readings(
    env: JEnv,
    _class: sys::jclass,
) -> sys::jobjectArray {
    let readings = guard(|| api::with_engine(|e| e.candidate_readings()).unwrap_or_default());
    unsafe { jni_util::string_array(env, readings) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_buffer(env: JEnv, _class: sys::jclass) -> jstring {
    let out = guard(|| api::with_engine(|e| e.buffer()).unwrap_or_default());
    unsafe { jni_util::rust_to_jstring(env, &out) }
}

/// displayBuffer() -> String。显示用拼音读数（与 `buffer()` **两个出口**，见 `api::display_buffer`）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_display_buffer(env: JEnv, _class: sys::jclass) -> jstring {
    let out = guard(|| api::with_engine(|e| e.display_buffer()).unwrap_or_default());
    unsafe { jni_util::rust_to_jstring(env, &out) }
}
