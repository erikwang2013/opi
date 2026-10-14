// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **五笔旋钮一族**的 JNI 出口（M4 第二波，2026-10-14；与 `cabi_wubi.rs` 镜像 ——
//! 两轨族边界一致是本仓纪律）。
//!
//! ⚠️ 拆出去**不等于脱离契约**：注册表仍在 `jni.rs`（本文件每个函数都要被它 `use`），
//! `tests/jni/checker.rs` 的 `JNI_ALL` 清单已加入本文件（漏了那批出口会从门禁视野里
//! 消失，见那个常量的注）。
//!
//! 整数码与哨兵语义见 `api/wubi.rs` 的头注（写侧越界忽略 / 个数钳区间 / 未装载 →
//! false / 0 / 空串）。

#![allow(clippy::missing_safety_doc)]
// ⚠️ 同 `jni.rs` 顶上那条：按模块生效。

use std::ffi::c_void;

use jni::NativeMethod;
use jni::jni_str;
use jni::sys::{self, jboolean, jint, jstring};

use crate::api;
use crate::jni::{JEnv, guard};
use crate::jni_util;

/// loadWubi(path: String) -> boolean。空/坏路径/引擎未装载 → false
/// （五笔方案回退普通路径，与 loadStroke 同为可选增强）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_load_wubi(
    env: JEnv,
    _class: sys::jclass,
    path: jstring,
) -> jboolean {
    guard(|| {
        let path = unsafe { jni_util::jstring_to_rust(env, path) }.unwrap_or_default();
        api::with_engine(|e| e.load_wubi(&path).is_ok()).unwrap_or(false)
    })
}

/// wubiStyle() -> int（0=纯五笔 1=五笔拼音混输）。未装载 → 0 是**哨兵**
/// （同 `scheme` 一档），别读成"用户选的就是纯五笔"。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_wubi_style(_env: JEnv, _class: sys::jclass) -> jint {
    guard(|| api::with_engine(|e| api::wubi_style_to_int(e.wubi_style())).unwrap_or_default())
}

/// setWubiStyle(style: int)：越界忽略（无返回值；要结果读 `wubiStyle()`）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_set_wubi_style(_env: JEnv, _class: sys::jclass, style: jint) {
    guard(|| {
        api::with_engine(|e| {
            if let Some(s) = api::wubi_style_from_int(style) {
                e.set_wubi_style(s);
            }
        })
    });
}

/// wubiMixedEntry() -> int（0=直接字母 1=z键）。未装载 → 0（哨兵）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_wubi_mixed_entry(_env: JEnv, _class: sys::jclass) -> jint {
    guard(|| {
        api::with_engine(|e| api::wubi_mixed_entry_to_int(e.wubi_mixed_entry())).unwrap_or_default()
    })
}

/// setWubiMixedEntry(entry: int)：越界忽略。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_set_wubi_mixed_entry(
    _env: JEnv,
    _class: sys::jclass,
    entry: jint,
) {
    guard(|| {
        api::with_engine(|e| {
            if let Some(x) = api::wubi_mixed_entry_from_int(entry) {
                e.set_wubi_mixed_entry(x);
            }
        })
    });
}

/// wubiCandidateLimit() -> int（1..=9）。未装载 → 0 是**哨兵**（真缺省是 5）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_wubi_candidate_limit(_env: JEnv, _class: sys::jclass) -> jint {
    guard(|| api::with_engine(|e| e.wubi_candidate_limit() as jint).unwrap_or_default())
}

/// setWubiCandidateLimit(n: int)：**负数/0 归 1、过大由引擎钳到 9**（坏值钳到确定值）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_set_wubi_candidate_limit(
    _env: JEnv,
    _class: sys::jclass,
    n: jint,
) {
    guard(|| {
        api::with_engine(|e| e.set_wubi_candidate_limit(n.max(1) as usize));
    });
}

/// wubiReturnStyle() -> int（0=清空 1=上屏原文）。未装载 → 0（哨兵）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_wubi_return_style(_env: JEnv, _class: sys::jclass) -> jint {
    guard(|| {
        api::with_engine(|e| api::wubi_return_to_int(e.wubi_return_style())).unwrap_or_default()
    })
}

/// setWubiReturnStyle(style: int)：越界忽略。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_set_wubi_return_style(
    _env: JEnv,
    _class: sys::jclass,
    style: jint,
) {
    guard(|| {
        api::with_engine(|e| {
            if let Some(s) = api::wubi_return_from_int(style) {
                e.set_wubi_return_style(s);
            }
        })
    });
}

/// wubiAutoCommit() -> boolean（四码唯一自动上屏开关；缺省 true）。未装载 → false（哨兵）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_wubi_auto_commit(_env: JEnv, _class: sys::jclass) -> jboolean {
    guard(|| api::with_engine(|e| e.wubi_auto_commit()).unwrap_or_default())
}

/// setWubiAutoCommit(on: boolean)：无返回值（设置项）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_set_wubi_auto_commit(
    _env: JEnv,
    _class: sys::jclass,
    on: jboolean,
) {
    guard(|| {
        api::with_engine(|e| e.set_wubi_auto_commit(on));
    });
}

/// wubiNextKeys() -> String：编码提示（当前缓冲可接续的键，拼成串）。
/// 非五笔态/无提示 → 空串。⚠️ 引擎侧每次调用全量重算，**别在每帧渲染里调**。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_wubi_next_keys(env: JEnv, _class: sys::jclass) -> jstring {
    guard(|| {
        let s = api::with_engine(|e| e.wubi_next_keys()).unwrap_or_default();
        unsafe { jni_util::rust_to_jstring(env, &s) }
    })
}

/// 本族 12 条注册项（`jni.rs` 的 `JNI_OnLoad` 调它并进注册表 —— 整块搬来是 500 行门禁：
/// `jni.rs` 加完这批是 528 行）。**条目一字未改**，搬的只是位置。
pub(crate) fn wubi_native_methods() -> Vec<NativeMethod<'static>> {
    // from_raw_parts 是 unsafe（与 `jni.rs` 注册表同一处 unsafe 上下文）。
    unsafe {
        vec![
            NativeMethod::from_raw_parts(
                jni_str!("loadWubi"),
                jni_str!("(Ljava/lang/String;)Z"),
                opijni_load_wubi as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("wubiStyle"),
                jni_str!("()I"),
                opijni_wubi_style as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("setWubiStyle"),
                jni_str!("(I)V"),
                opijni_set_wubi_style as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("wubiMixedEntry"),
                jni_str!("()I"),
                opijni_wubi_mixed_entry as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("setWubiMixedEntry"),
                jni_str!("(I)V"),
                opijni_set_wubi_mixed_entry as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("wubiCandidateLimit"),
                jni_str!("()I"),
                opijni_wubi_candidate_limit as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("setWubiCandidateLimit"),
                jni_str!("(I)V"),
                opijni_set_wubi_candidate_limit as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("wubiReturnStyle"),
                jni_str!("()I"),
                opijni_wubi_return_style as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("setWubiReturnStyle"),
                jni_str!("(I)V"),
                opijni_set_wubi_return_style as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("wubiAutoCommit"),
                jni_str!("()Z"),
                opijni_wubi_auto_commit as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("setWubiAutoCommit"),
                jni_str!("(Z)V"),
                opijni_set_wubi_auto_commit as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("wubiNextKeys"),
                jni_str!("()Ljava/lang/String;"),
                opijni_wubi_next_keys as *mut c_void,
            ),
        ]
    }
}
