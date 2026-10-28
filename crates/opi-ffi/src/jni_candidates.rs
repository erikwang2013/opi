// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! JNI 出口的**候选 / 分页 / 缓冲**一族（从 `jni.rs` 拆出：那个文件加
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

/// pageSize() -> Int：每页候选数（`engine_core::router::PAGE_SIZE` 的 JNI 载体，
/// 与 C 面 `opi_page_size()` 同源）。**不依赖引擎装载**（常量，未装载也有值）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_page_size(_env: JEnv, _class: sys::jclass) -> jint {
    guard(|| engine_core::router::PAGE_SIZE as jint)
}

/// candidateReadings() -> String[]。候选拼音读数（已分词），**按读音去重保序**、
/// 与最近一次 `candidates(limit)` 同源同一次排名 —— 9 键左列用（2026-10-09 用户
/// 裁决「一列多个、可滑动查看」；2026-10-12 用户裁决「左列去重」——此前逐候选
/// 出一行，真库 `426` 实测 8 行里 5 行重复）。非 9/14 键方案、或缓冲已变
/// （提交/退格）⇒ 空数组。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_candidate_readings(
    env: JEnv,
    _class: sys::jclass,
) -> sys::jobjectArray {
    let readings = guard(|| api::with_engine(|e| e.candidate_readings()).unwrap_or_default());
    unsafe { jni_util::string_array(env, readings) }
}

/// pinReading(reading: String)：**钉读音**（空串/null = 解除）—— 9 键左列点一行拼音后
/// **不确认**，候选列表收窄到该读音一族（2026-10-09 用户裁决「选择后所有候选词也跟着
/// 拼音变化显示出来，不要直接确认」）。缓冲一变自动作废（引擎侧按缓冲配对校验）。
/// 调用方重读 candidates/读数（refresh）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_pin_reading(env: JEnv, _class: sys::jclass, reading: jstring) {
    guard(|| {
        let r = unsafe { jni_util::jstring_to_rust(env, reading) }.unwrap_or_default();
        api::with_engine(|e| e.set_pinned_reading(if r.is_empty() { None } else { Some(r) }));
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_buffer(env: JEnv, _class: sys::jclass) -> jstring {
    let out = guard(|| api::with_engine(|e| e.buffer()).unwrap_or_default());
    unsafe { jni_util::rust_to_jstring(env, &out) }
}

/// bufferIsCarried() -> Boolean。缓冲是否由**别的模式**带过来（`switchLanguage`
/// 保留缓冲时打标；清空/提交/回到来处即清）。未装载 → false。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_buffer_is_carried(
    _env: JEnv,
    _class: sys::jclass,
) -> sys::jboolean {
    guard(|| api::with_engine(|e| e.buffer_is_carried()).unwrap_or_default())
}

/// displayBuffer() -> String。显示用拼音读数（与 `buffer()` **两个出口**，见 `api::display_buffer`）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_display_buffer(env: JEnv, _class: sys::jclass) -> jstring {
    let out = guard(|| api::with_engine(|e| e.display_buffer()).unwrap_or_default());
    unsafe { jni_util::rust_to_jstring(env, &out) }
}

/// 已确认的逐字片段（2026-10-17 逐字确认）：显示串里已含它（display_buffer 前置），
/// 安卓另需**长度**来修正 9 键「·」待编标记的覆盖率换算。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_pending_text(env: JEnv, _class: sys::jclass) -> jstring {
    let out = guard(|| api::with_engine(|e| e.pending_text()).unwrap_or_default());
    unsafe { jni_util::rust_to_jstring(env, &out) }
}

#[cfg(test)]
mod tests {
    use super::opijni_page_size;

    /// 门禁（2026-10-28）：两个 ABI 的页大小都必须是 `router::PAGE_SIZE` 本身，
    /// 不是各自手抄的 8。JNI 不依赖引擎装载（裸指针即可调）；C 面由 ios 同批落地。
    #[test]
    fn page_size_exports_match_router_constant() {
        let jni = unsafe { opijni_page_size(std::ptr::null_mut(), std::ptr::null_mut()) };
        assert_eq!(jni as usize, engine_core::router::PAGE_SIZE);
        let c = unsafe { crate::cabi::opi_page_size() };
        assert_eq!(c as usize, engine_core::router::PAGE_SIZE);
    }
}
