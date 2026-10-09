// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! JNI 出口的**词典装载（luna / trad / stroke）+ 输入方案**一组（从 `jni.rs` 拆出：
//! 那个文件加「输入方案」后越 500 行门禁；与 `jni_user_words.rs` 同一个拆分先例）。
//!
//! ⚠️ 拆出去**不等于脱离契约**：`tests/jni_contract.rs` 读的是本模块**全部** .rs 的
//! 合成文本（注册表仍在 `jni.rs`，函数体在这里 —— 两侧都在文本里，门禁才看得全）。
//! `guard` / `JEnv` 仍是 `jni.rs` 的（pub(crate)），语义与 C ABI 的
//! `cabi_scheme.rs` 同源。

#![allow(clippy::missing_safety_doc)]
// ⚠️ 同 `jni.rs` 顶上那条：按模块生效，拆出来要在本文件里再写一次。

use jni::sys::{self, jboolean, jint, jstring};

use crate::api;
use crate::jni::{JEnv, guard};
use crate::jni_util;

/// load(path: String?) -> bool。null/空串 → 内置回退词库；坏路径 → false。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_load(
    env: JEnv,
    _class: sys::jclass,
    path: jstring,
) -> jboolean {
    guard(|| {
        let path = unsafe { jni_util::jstring_to_rust(env, path) };
        api::install(path.as_deref()).is_ok()
    })
}

/// loadTrad(path: String) -> bool。空/坏路径/引擎未加载 → false（繁体模式回退简体库）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_load_trad(
    env: JEnv,
    _class: sys::jclass,
    path: jstring,
) -> jboolean {
    guard(|| {
        let path = unsafe { jni_util::jstring_to_rust(env, path) }.unwrap_or_default();
        api::install_trad(&path).is_ok()
    })
}

/// loadStroke(path: String) -> bool。笔画词典（stroke.opid，2026-10-09）。空/坏
/// 路径/引擎未加载 → false（笔画方案回退普通路径，与 loadTrad 同为可选增强）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_load_stroke(
    env: JEnv,
    _class: sys::jclass,
    path: jstring,
) -> jboolean {
    guard(|| {
        let path = unsafe { jni_util::jstring_to_rust(env, path) }.unwrap_or_default();
        api::install_stroke(&path).is_ok()
    })
}

/// setScheme(scheme: int)：输入方案（移动端「输入模式」，2026-10-09）。**无返回值**
/// （设置项）；要拿结果读 `scheme()`。取值 0=全键盘 1=拼音9键 2=拼音14键 3=笔画，
/// 越界忽略。⚠️ **换方案清缓冲** —— 与 `switchMode` 同一档，调用方重读
/// buffer/候选/页码。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_set_scheme(_env: JEnv, _class: sys::jclass, scheme: jint) {
    guard(|| {
        api::with_engine(|e| {
            if let Some(s) = api::scheme_from_int(scheme) {
                e.set_scheme(s);
            }
        })
    });
}

/// switchLanguage(mode: int, scheme: int)：**语言切换**（中↔英/中↔繁）—— 换模式与
/// 方案但**保留缓冲**（2026-10-09 用户裁决「中文输入的时候切换英文，拼音保留可继续
/// 使用」）。与 `switchMode`/`setScheme` 的区别只有不清缓冲这一条，判据与成对切换的
/// 理由见 `engine_core::Engine::switch_language`。取值同 `switchMode`/`setScheme`
/// 两张整数表；任一越界则整体忽略（宁可不动，也不做半程切换）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_switch_language(
    _env: JEnv,
    _class: sys::jclass,
    mode: jint,
    scheme: jint,
) {
    guard(|| {
        api::with_engine(|e| {
            if let (Some(m), Some(s)) = (api::mode_from_int(mode), api::scheme_from_int(scheme)) {
                e.switch_language(m.into(), s);
            }
        })
    });
}

/// scheme() -> int：输入方案读侧（同 `setScheme` 的整数表）。未装载 → 0 是
/// **哨兵**（同 `fuzzyEnabled` 一档），别读成"用户选的就是全键盘"。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_scheme(_env: JEnv, _class: sys::jclass) -> jint {
    guard(|| api::with_engine(|e| api::scheme_to_int(e.scheme())).unwrap_or_default())
}
