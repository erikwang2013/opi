// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! opi-clip 的 JNI 出口。语义与 C ABI（`cabi_clip.rs`）**同源**：两边都调
//! `OpiClip` 的方法，不各写一份实现。
//!
//! ## Kotlin 侧对应声明（`OpiEngine.kt` 已落地；两边必须逐条同名同签名）
//!
//! ```kotlin
//! external fun clipCreate(configJson: String?): Long
//! external fun clipDestroy(handle: Long)
//! external fun clipStart(handle: Long): Boolean
//! external fun clipStop(handle: Long)
//! external fun clipLocalText(handle: Long, text: String?): Int
//! external fun clipPollEvent(handle: Long, timeoutMs: Int): String?
//! external fun clipPairStart(handle: Long, peerId: String): Boolean
//! external fun clipPairConfirm(handle: Long, peerId: String, accept: Boolean): Boolean
//! external fun clipPairCancel(handle: Long, peerId: String): Boolean
//! external fun clipPeers(handle: Long): String?
//! external fun clipForgetPeer(handle: Long, peerId: String): Boolean
//! external fun clipStatus(handle: Long): String?
//! external fun clipLastError(): String
//! ```
//!
//! 注册方式与五笔/短语同一条手续：`jni.rs` 的 `JNI_OnLoad` 里
//! `methods.extend(clip_native_methods())`，**同一张 RegisterNatives 表**。
//! ⚠️ 表里错 1 条 = 整表注册失败 = `System.load` 抛 `UnsatisfiedLinkError`；
//! 名字/描述符漂移由 `tests/jni_contract.rs`（活文件集合相等）与
//! `tests/jni_clip_contract.rs`（本族 13 条钉死）兜住。
//!
//! 句柄是 `Long`（Rust 侧裸指针）；**不可跨线程**，与 C ABI 同一条契约。
//! JNI 的 `null` 入参与 C ABI 的「null 指针 = 空串」同义（见 [`read_arg`]）。

#![allow(clippy::missing_safety_doc)]

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

use jni::NativeMethod;
use jni::jni_str;
use jni::sys::{self, jboolean, jint, jlong, jstring};

use crate::cabi_clip::{OpiClip, last_error_string, settle};
use crate::jni::JEnv;
use crate::jni_util;

fn ptr(handle: jlong) -> *mut OpiClip {
    handle as usize as *mut OpiClip
}

/// JNI 的 null 按 C ABI 的「null 指针 = 空串」处理；**非法 UTF-16** 才是错误。
///
/// # Safety
/// `env` 必须是当前线程的有效 JNIEnv；`s` 必须是有效 jstring（可空）。
unsafe fn read_arg(env: JEnv, s: jstring, what: &str) -> Result<String, String> {
    if s.is_null() {
        return Ok(String::new());
    }
    unsafe { jni_util::jstring_to_rust(env, s) }
        .ok_or_else(|| format!("{what} is not valid UTF-16"))
}

/// create(configJson) -> 句柄；失败 → 0（原因读 `clipLastError()`）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_clip_create(
    env: JEnv,
    _class: sys::jclass,
    config_json: jstring,
) -> jlong {
    settle(
        catch_unwind(AssertUnwindSafe(|| -> Result<jlong, String> {
            let json = unsafe { read_arg(env, config_json, "clip config") }?;
            Ok(OpiClip::create(&json)? as usize as jlong)
        })),
        0,
    )
}

/// destroy(handle)：停止并释放；0 是空句柄，安全。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_clip_destroy(_env: JEnv, _class: sys::jclass, handle: jlong) {
    settle(
        catch_unwind(AssertUnwindSafe(|| unsafe { OpiClip::free(ptr(handle)) })),
        (),
    );
}

/// start(handle) -> Boolean：幂等。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_clip_start(
    _env: JEnv,
    _class: sys::jclass,
    handle: jlong,
) -> jboolean {
    settle(
        catch_unwind(AssertUnwindSafe(|| {
            unsafe { OpiClip::borrow_mut(ptr(handle)) }?.start()
        })),
        false,
    )
}

/// stop(handle)：幂等。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_clip_stop(_env: JEnv, _class: sys::jclass, handle: jlong) {
    settle(
        catch_unwind(AssertUnwindSafe(|| {
            unsafe { OpiClip::borrow_mut(ptr(handle)) }?.stop()
        })),
        (),
    );
}

/// localText(handle, text) -> Int：0=已发布 1=防回环抑制 -1=失败。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_clip_local_text(
    env: JEnv,
    _class: sys::jclass,
    handle: jlong,
    text: jstring,
) -> jint {
    settle(
        catch_unwind(AssertUnwindSafe(|| {
            let text = unsafe { read_arg(env, text, "clipboard text") }?;
            unsafe { OpiClip::borrow(ptr(handle)) }?.publish(&text)
        })),
        -1,
    )
}

/// pollEvent(handle, timeoutMs) -> String?：null=失败；空串=无事件。
/// timeoutMs=0 立即返回，>0 最多等 1 秒。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_clip_poll_event(
    env: JEnv,
    _class: sys::jclass,
    handle: jlong,
    timeout_ms: jint,
) -> jstring {
    let out = settle(
        catch_unwind(AssertUnwindSafe(|| {
            unsafe { OpiClip::borrow(ptr(handle)) }?
                .poll(timeout_ms.max(0) as u32)
                .map(Some)
        })),
        None,
    );
    match out {
        Some(json) => unsafe { jni_util::rust_to_jstring(env, &json) },
        None => std::ptr::null_mut(),
    }
}

/// pairStart(handle, peerId) -> Boolean。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_clip_pair_start(
    env: JEnv,
    _class: sys::jclass,
    handle: jlong,
    peer_id: jstring,
) -> jboolean {
    settle(
        catch_unwind(AssertUnwindSafe(|| {
            let peer = unsafe { read_arg(env, peer_id, "peer id") }?;
            unsafe { OpiClip::borrow(ptr(handle)) }?.pair_start(&peer)
        })),
        false,
    )
}

/// pairConfirm(handle, peerId, accept) -> Boolean。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_clip_pair_confirm(
    env: JEnv,
    _class: sys::jclass,
    handle: jlong,
    peer_id: jstring,
    accept: jboolean,
) -> jboolean {
    settle(
        catch_unwind(AssertUnwindSafe(|| {
            let peer = unsafe { read_arg(env, peer_id, "peer id") }?;
            unsafe { OpiClip::borrow(ptr(handle)) }?.pair_confirm(&peer, accept)
        })),
        false,
    )
}

/// pairCancel(handle, peerId) -> Boolean：等价于 reject。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_clip_pair_cancel(
    env: JEnv,
    _class: sys::jclass,
    handle: jlong,
    peer_id: jstring,
) -> jboolean {
    settle(
        catch_unwind(AssertUnwindSafe(|| {
            let peer = unsafe { read_arg(env, peer_id, "peer id") }?;
            unsafe { OpiClip::borrow(ptr(handle)) }?.pair_confirm(&peer, false)
        })),
        false,
    )
}

/// peers(handle) -> JSON 数组；失败 → null，未 start → `[]`。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_clip_peers(
    env: JEnv,
    _class: sys::jclass,
    handle: jlong,
) -> jstring {
    let out = settle(
        catch_unwind(AssertUnwindSafe(|| {
            unsafe { OpiClip::borrow(ptr(handle)) }?
                .peers_json()
                .map(Some)
        })),
        None,
    );
    match out {
        Some(json) => unsafe { jni_util::rust_to_jstring(env, &json) },
        None => std::ptr::null_mut(),
    }
}

/// forgetPeer(handle, peerId) -> Boolean。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_clip_forget_peer(
    env: JEnv,
    _class: sys::jclass,
    handle: jlong,
    peer_id: jstring,
) -> jboolean {
    settle(
        catch_unwind(AssertUnwindSafe(|| {
            let peer = unsafe { read_arg(env, peer_id, "peer id") }?;
            unsafe { OpiClip::borrow(ptr(handle)) }?.forget_peer(&peer)
        })),
        false,
    )
}

/// status(handle) -> JSON；失败 → null。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_clip_status(
    env: JEnv,
    _class: sys::jclass,
    handle: jlong,
) -> jstring {
    let out = settle(
        catch_unwind(AssertUnwindSafe(|| {
            unsafe { OpiClip::borrow(ptr(handle)) }?
                .status_json()
                .map(Some)
        })),
        None,
    );
    match out {
        Some(json) => unsafe { jni_util::rust_to_jstring(env, &json) },
        None => std::ptr::null_mut(),
    }
}

/// lastError() -> String：空串 = 没有错误（按线程记录）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_clip_last_error(env: JEnv, _class: sys::jclass) -> jstring {
    let message = settle(
        catch_unwind(AssertUnwindSafe(|| {
            Ok::<String, String>(last_error_string())
        })),
        String::new(),
    );
    unsafe { jni_util::rust_to_jstring(env, &message) }
}

/// 注册表（`JNI_OnLoad` 里 best-effort 单独注册，见模块头注）。
pub(crate) fn clip_native_methods() -> Vec<NativeMethod<'static>> {
    unsafe {
        vec![
            NativeMethod::from_raw_parts(
                jni_str!("clipCreate"),
                jni_str!("(Ljava/lang/String;)J"),
                opijni_clip_create as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("clipDestroy"),
                jni_str!("(J)V"),
                opijni_clip_destroy as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("clipStart"),
                jni_str!("(J)Z"),
                opijni_clip_start as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("clipStop"),
                jni_str!("(J)V"),
                opijni_clip_stop as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("clipLocalText"),
                jni_str!("(JLjava/lang/String;)I"),
                opijni_clip_local_text as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("clipPollEvent"),
                jni_str!("(JI)Ljava/lang/String;"),
                opijni_clip_poll_event as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("clipPairStart"),
                jni_str!("(JLjava/lang/String;)Z"),
                opijni_clip_pair_start as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("clipPairConfirm"),
                jni_str!("(JLjava/lang/String;Z)Z"),
                opijni_clip_pair_confirm as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("clipPairCancel"),
                jni_str!("(JLjava/lang/String;)Z"),
                opijni_clip_pair_cancel as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("clipPeers"),
                jni_str!("(J)Ljava/lang/String;"),
                opijni_clip_peers as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("clipForgetPeer"),
                jni_str!("(JLjava/lang/String;)Z"),
                opijni_clip_forget_peer as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("clipStatus"),
                jni_str!("(J)Ljava/lang/String;"),
                opijni_clip_status as *mut c_void,
            ),
            NativeMethod::from_raw_parts(
                jni_str!("clipLastError"),
                jni_str!("()Ljava/lang/String;"),
                opijni_clip_last_error as *mut c_void,
            ),
        ]
    }
}
