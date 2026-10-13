// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「属性设置」**旋钮族**的 JNI 出口（2026-10-13 从 `jni.rs` 拆出：加拼音纠错一批后
//! 那个文件 509 行撞门禁；与 `cabi_switches.rs` 同一次拆分、同一主题）。
//!
//! ⚠️ 拆出去**不等于脱离契约**：注册表仍在 `jni.rs`（本文件每个函数都要被它 `use`），
//! `tests/jni/checker.rs` 的 `JNI_ALL` 清单已加入本文件（漏了那批出口会从门禁视野里
//! 消失，见那个常量的注）。

#![allow(clippy::missing_safety_doc)]
// ⚠️ 同 `jni.rs` 顶上那条：按模块生效。

use std::panic::{AssertUnwindSafe, catch_unwind};

use jni::sys::{self, jboolean, jint};

use crate::api;
use crate::jni::{JEnv, guard};

/// chinesePunct() -> boolean：中文标点表读侧。未装载 → false。
/// 关掉本档 ≠ 半角：表未命中的字符仍由全角那一档管。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_chinese_punct(_env: JEnv, _class: sys::jclass) -> jboolean {
    guard(|| api::with_engine(|e| e.chinese_punct()).unwrap_or_default())
}

/// setChinesePunct(on: boolean)：设置中文标点表是否生效。**无返回值**（设置项）；
/// 要拿结果读 `chinesePunct()`。**写入口有两个**：本出口是设置项，触发键走
/// `toggleChinesePunct`（那个有返回值）。未装载时是空操作。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_set_chinese_punct(
    _env: JEnv,
    _class: sys::jclass,
    on: jboolean,
) {
    guard(|| api::with_engine(|e| e.set_chinese_punct(on)));
}

/// fuzzyEnabled() -> boolean：「属性设置」的读侧之一（写侧见 `setFuzzy`）。
/// 未装载 → false（哨兵语义见模块头）。**别当成"开关的默认值"** ——
/// 默认值的真源是 `engine-core` 的 `RankOptions::default()`（两个都开）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_fuzzy_enabled(_env: JEnv, _class: sys::jclass) -> jboolean {
    guard(|| api::with_engine(|e| e.fuzzy_enabled()).unwrap_or_default())
}

/// jianpinEnabled() -> boolean：同上。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_jianpin_enabled(_env: JEnv, _class: sys::jclass) -> jboolean {
    guard(|| api::with_engine(|e| e.jianpin_enabled()).unwrap_or_default())
}

/// setFuzzy(on: boolean)：模糊音开关（「属性设置」四个旋钮之一）。**无返回值**（设置项）；
/// 要拿结果读 `fuzzyEnabled()`。未装载时是空操作。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_set_fuzzy(_env: JEnv, _class: sys::jclass, on: jboolean) {
    guard(|| api::with_engine(|e| e.set_fuzzy(on)));
}

/// setJianpin(on: boolean)：简拼开关（同上）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_set_jianpin(_env: JEnv, _class: sys::jclass, on: jboolean) {
    guard(|| api::with_engine(|e| e.set_jianpin(on)));
}

/// correctionEnabled() -> boolean：拼音纠错读侧（2026-10-13；写侧见 `setCorrection`）。
/// 未装载 → false（哨兵语义同族；真缺省是 true —— 对齐搜狗，见引擎侧 Default）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_correction_enabled(_env: JEnv, _class: sys::jclass) -> jboolean {
    guard(|| api::with_engine(|e| e.correction_enabled()).unwrap_or_default())
}

/// setCorrection(on: boolean)：拼音纠错开关（「属性设置」旋钮之一）。无返回值。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_set_correction(_env: JEnv, _class: sys::jclass, on: jboolean) {
    guard(|| api::with_engine(|e| e.set_correction(on)));
}

/// setFuzzyPairs(mask: int)：模糊音**逐对**设置（2026-10-13 用户裁决「可多选类似 z=zh」）。
/// 位序 = `engine_core::fuzzy::FUZZY_PAIRS` 的数组序（对外契约，四端同此）；
/// `0xFF` = 全开（= 旧 `setFuzzy(true)`）；越界位钳掉（只保留低 8 位）。
/// **无返回值**（设置项）；要拿结果读 `fuzzyPairs()`。未装载时是空操作。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_set_fuzzy_pairs(
    _env: JEnv,
    _class: sys::jclass,
    mask: jint,
) {
    let m = (mask as u32 & 0xFF) as u8;
    guard(|| api::with_engine(|e| e.set_fuzzy_pairs(m)));
}

/// fuzzyPairs() -> int：模糊音启用集的读侧（写侧见 `setFuzzyPairs`；旧布尔读侧
/// `fuzzyEnabled` 保留 = 「任一组开着」）。未装载 → 0（哨兵语义同族）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_fuzzy_pairs(_env: JEnv, _class: sys::jclass) -> jint {
    guard(|| api::with_engine(|e| i32::from(e.fuzzy_pairs())).unwrap_or_default())
}

/// toggleChinesePunct() -> boolean：中文标点表开关的**触发键入口**，返回**切换后的
/// 新状态**。⚠️ 与 `toggleFullwidth` 的重读规则不通用（本档 `switchMode` 不重置）。
/// 完整契约在 `cabi.rs` 的 `opi_toggle_chinese_punct` 上。未装载 → false。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_toggle_chinese_punct(
    _env: JEnv,
    _class: sys::jclass,
) -> jboolean {
    guard(|| api::with_engine(|e| e.toggle_chinese_punct()).unwrap_or_default())
}

