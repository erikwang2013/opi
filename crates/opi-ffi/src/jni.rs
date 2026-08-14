//! JNI 出口：`JNI_OnLoad` + `RegisterNatives` 注册（不用 Java_ 命名导出，防签名脆断）。
//! 宿主类：`io/opi/input/jni/OpiEngine`。每个函数 `catch_unwind` 包裹，
//! panic / 错误返回哨兵（boolean false、int 0、String/数组 null）。
//! 语义与 C ABI（cabi.rs）完全一致，共享 api::SINGLETON 与内部实现。
//!
//! # Safety（本模块所有 `opijni_*` 与 `JNI_OnLoad` 的统一契约）
//! `env` 必须为当前线程有效且非空的 JNIEnv（JVM 调用约定保证）；jstring 参数须为
//! 有效本地引用（可 null）。每个函数内部均以 catch_unwind 包裹，panic 不跨 FFI 边界。
#![allow(clippy::missing_safety_doc)]

use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};

use jni::jni_str;
use jni::sys::{self, jboolean, jint, jshort, jstring};
use jni::{JavaVM, NativeMethod, ScopeToken};

use crate::api;
use crate::jni_util;

type JEnv = *mut sys::JNIEnv;

// ---------- 19 个 native 方法 ----------

/// load(path: String?) -> bool。null/空串 → 内置回退词库；坏路径 → false。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_load(env: JEnv, _class: sys::jclass, path: jstring) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        let path = unsafe { jni_util::jstring_to_rust(env, path) };
        api::install(path.as_deref()).is_ok()
    }))
    .unwrap_or(false)
}

/// loadTrad(path: String) -> bool。空/坏路径/引擎未加载 → false（繁体模式回退简体库）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_load_trad(env: JEnv, _class: sys::jclass, path: jstring) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        let path = unsafe { jni_util::jstring_to_rust(env, path) }.unwrap_or_default();
        api::install_trad(&path).is_ok()
    }))
    .unwrap_or(false)
}

/// inputKey(ch: String) -> String。永不 panic。单字符外（空/多字符/非 ASCII）返回空串。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_input_key(env: JEnv, _class: sys::jclass, key: jstring) -> jstring {
    let out = catch_unwind(AssertUnwindSafe(|| {
        let ch = unsafe { jni_util::jstring_to_rust(env, key) }.unwrap_or_default();
        api::with_engine(|e| e.input_key(ch)).unwrap_or_default()
    }))
    .unwrap_or_default();
    unsafe { jni_util::rust_to_jstring(env, &out) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_backspace(_env: JEnv, _class: sys::jclass) {
    let _ = catch_unwind(AssertUnwindSafe(|| api::with_engine(|e| e.backspace())));
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_clear(_env: JEnv, _class: sys::jclass) {
    let _ = catch_unwind(AssertUnwindSafe(|| api::with_engine(|e| e.clear())));
}

/// select(index: Int) -> String。越界返回空串（旧语义）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_select(env: JEnv, _class: sys::jclass, index: jint) -> jstring {
    let out = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.select(index.max(0) as usize)).unwrap_or_default()
    }))
    .unwrap_or_default();
    unsafe { jni_util::rust_to_jstring(env, &out) }
}

/// switchMode(mode: Int)。0=Pinyin 1=English 2=Number 3=Symbol，越界忽略。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_switch_mode(_env: JEnv, _class: sys::jclass, mode: jint) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(m) = api::mode_from_int(mode) {
            api::with_engine(|e| e.switch_mode(m.into()));
        }
    }));
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_set_shift(_env: JEnv, _class: sys::jclass, on: jboolean) {
    let _ = catch_unwind(AssertUnwindSafe(|| api::with_engine(|e| e.set_shift(on))));
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_input_space(env: JEnv, _class: sys::jclass) -> jstring {
    let out = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.input_space()).unwrap_or_default()
    }))
    .unwrap_or_default();
    unsafe { jni_util::rust_to_jstring(env, &out) }
}

/// candidates(limit: Int) -> String[]。仅文本数组（kind/score UI 不用）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_candidates(env: JEnv, _class: sys::jclass, limit: jint) -> sys::jobjectArray {
    let texts = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| api::candidate_texts(e, limit.max(0) as usize)).unwrap_or_default()
    }))
    .unwrap_or_default();
    unsafe { jni_util::string_array(env, texts) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_buffer(env: JEnv, _class: sys::jclass) -> jstring {
    let out = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.buffer()).unwrap_or_default()
    }))
    .unwrap_or_default();
    unsafe { jni_util::rust_to_jstring(env, &out) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_mode(_env: JEnv, _class: sys::jclass) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| api::mode_to_int(e.mode().into())).unwrap_or(0)
    }))
    .unwrap_or(0)
}

/// searchSymbols(keyword: String) -> String[]。仅文本数组。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_search_symbols(env: JEnv, _class: sys::jclass, keyword: jstring) -> sys::jobjectArray {
    let texts = catch_unwind(AssertUnwindSafe(|| {
        let kw = unsafe { jni_util::jstring_to_rust(env, keyword) }.unwrap_or_default();
        api::with_engine(|e| api::search_symbol_texts(e, &kw)).unwrap_or_default()
    }))
    .unwrap_or_default();
    unsafe { jni_util::string_array(env, texts) }
}

/// symbolBlocks() -> String。JSON：`[{id,start,end,name,common}]`。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_symbol_blocks(env: JEnv, _class: sys::jclass) -> jstring {
    let json = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| api::symbol_blocks_json(e)).unwrap_or_default()
    }))
    .unwrap_or_default();
    unsafe { jni_util::rust_to_jstring(env, &json) }
}

/// symbolsInBlock(id: Short) -> String[]。仅文本数组。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_symbols_in_block(env: JEnv, _class: sys::jclass, id: jshort) -> sys::jobjectArray {
    let texts = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| api::symbol_texts(e, id.max(0) as u16)).unwrap_or_default()
    }))
    .unwrap_or_default();
    unsafe { jni_util::string_array(env, texts) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_learner_enabled(_env: JEnv, _class: sys::jclass) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| api::with_engine(|e| e.learner_enabled()).unwrap_or(false))).unwrap_or(false)
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_set_learner(_env: JEnv, _class: sys::jclass, enabled: jboolean) {
    let _ = catch_unwind(AssertUnwindSafe(|| api::with_engine(|e| e.set_learner(enabled))));
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_clear_user_words(_env: JEnv, _class: sys::jclass) {
    let _ = catch_unwind(AssertUnwindSafe(|| api::with_engine(|e| e.clear_user_words())));
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_export_user_words(env: JEnv, _class: sys::jclass) -> jstring {
    let out = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.export_user_words()).unwrap_or_default()
    }))
    .unwrap_or_default();
    unsafe { jni_util::rust_to_jstring(env, &out) }
}

// ---------- JNI_OnLoad ----------

#[unsafe(no_mangle)]
pub unsafe extern "system" fn JNI_OnLoad(vm: *mut sys::JavaVM, _reserved: *mut c_void) -> jint {
    let result = catch_unwind(AssertUnwindSafe(|| -> Result<jint, String> {
        // JNI_OnLoad 必然运行在已 attach 的 JVM 线程上（System.loadLibrary 的调用线程），
        // get_env_attachment 即 GetEnv，不会触发 attach。
        let mut scope = ScopeToken::default();
        let mut guard = unsafe { JavaVM::from_raw(vm).get_env_attachment(&mut scope).map_err(|e| format!("get_env_attachment 失败: {e}"))? };
        let env = guard.borrow_env_mut();
        let class = env
            .find_class(jni_str!("io/opi/input/jni/OpiEngine"))
            .map_err(|e| format!("find_class 失败: {e}"))?;
        let methods = unsafe {
            [
                NativeMethod::from_raw_parts(jni_str!("load"), jni_str!("(Ljava/lang/String;)Z"), opijni_load as *mut c_void),
                NativeMethod::from_raw_parts(jni_str!("loadTrad"), jni_str!("(Ljava/lang/String;)Z"), opijni_load_trad as *mut c_void),
                NativeMethod::from_raw_parts(jni_str!("inputKey"), jni_str!("(Ljava/lang/String;)Ljava/lang/String;"), opijni_input_key as *mut c_void),
                NativeMethod::from_raw_parts(jni_str!("backspace"), jni_str!("()V"), opijni_backspace as *mut c_void),
                NativeMethod::from_raw_parts(jni_str!("clear"), jni_str!("()V"), opijni_clear as *mut c_void),
                NativeMethod::from_raw_parts(jni_str!("select"), jni_str!("(I)Ljava/lang/String;"), opijni_select as *mut c_void),
                NativeMethod::from_raw_parts(jni_str!("switchMode"), jni_str!("(I)V"), opijni_switch_mode as *mut c_void),
                NativeMethod::from_raw_parts(jni_str!("setShift"), jni_str!("(Z)V"), opijni_set_shift as *mut c_void),
                NativeMethod::from_raw_parts(jni_str!("inputSpace"), jni_str!("()Ljava/lang/String;"), opijni_input_space as *mut c_void),
                NativeMethod::from_raw_parts(jni_str!("candidates"), jni_str!("(I)[Ljava/lang/String;"), opijni_candidates as *mut c_void),
                NativeMethod::from_raw_parts(jni_str!("buffer"), jni_str!("()Ljava/lang/String;"), opijni_buffer as *mut c_void),
                NativeMethod::from_raw_parts(jni_str!("mode"), jni_str!("()I"), opijni_mode as *mut c_void),
                NativeMethod::from_raw_parts(jni_str!("searchSymbols"), jni_str!("(Ljava/lang/String;)[Ljava/lang/String;"), opijni_search_symbols as *mut c_void),
                NativeMethod::from_raw_parts(jni_str!("symbolBlocks"), jni_str!("()Ljava/lang/String;"), opijni_symbol_blocks as *mut c_void),
                NativeMethod::from_raw_parts(jni_str!("symbolsInBlock"), jni_str!("(S)[Ljava/lang/String;"), opijni_symbols_in_block as *mut c_void),
                NativeMethod::from_raw_parts(jni_str!("learnerEnabled"), jni_str!("()Z"), opijni_learner_enabled as *mut c_void),
                NativeMethod::from_raw_parts(jni_str!("setLearner"), jni_str!("(Z)V"), opijni_set_learner as *mut c_void),
                NativeMethod::from_raw_parts(jni_str!("clearUserWords"), jni_str!("()V"), opijni_clear_user_words as *mut c_void),
                NativeMethod::from_raw_parts(jni_str!("exportUserWords"), jni_str!("()Ljava/lang/String;"), opijni_export_user_words as *mut c_void),
            ]
        };
        unsafe {
            env.register_native_methods(class, &methods)
                .map_err(|e| format!("register_natives 失败: {e}"))?;
        }
        Ok(sys::JNI_VERSION_1_6)
    }));
    match result {
        Ok(Ok(v)) => v,
        _ => 0, // 注册失败：System.load 将抛出 UnsatisfiedLinkError
    }
}

