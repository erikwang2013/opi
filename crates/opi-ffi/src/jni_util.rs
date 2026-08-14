//! JNI UTF-16 字符串转换（红线：禁止 GetStringUTFChars / NewStringUTF——
//! modified UTF-8 会把 emoji 编码成 CESU-8，被 Rust 的 UTF-8 校验拒绝）。
//!
//! jni 0.22 移除了 `get_string_chars`，且 `new_string` 只收 `AsRef<str>`、
//! 内部走 modified UTF-8（NewStringUTF）。因此此处直连 JNI 原生接口
//! （`JNINativeInterface_`）调用 `GetStringChars` / `NewString`，全程 UTF-16。
//! 纯转换逻辑拆出 `utf16_units_to_string` / `string_to_utf16_units` 供无 JVM 单测。

use jni::sys::{self, jchar, jsize, jstring};

/// UTF-16 单元 → Rust String。非法 UTF-16（代理对拆半）返回 None。
pub fn utf16_units_to_string(units: &[u16]) -> Option<String> {
    String::from_utf16(units).ok()
}

/// Rust str → UTF-16 单元（NewString 的输入形式）。
pub fn string_to_utf16_units(s: &str) -> Vec<jchar> {
    s.encode_utf16().collect()
}

/// 从 Java String（UTF-16 jchar 序列）读取为 Rust String。
///
/// # Safety
///
/// `env` 必须是当前线程有效且非空的 JNIEnv；`s` 必须是有效 jstring（可空，空返回 None）。
pub unsafe fn jstring_to_rust(env: *mut sys::JNIEnv, s: jstring) -> Option<String> {
    if env.is_null() || s.is_null() {
        return None;
    }
    // jni-sys 的 JNIEnv 即接口指针（*const JNINativeInterface_），两层解引用得接口；
    // JNINativeInterface_ 是 union，函数指针在 versioned sub-struct（v1_1/v1_2/...）中，
    // 本模块所需函数全部属于 JNI 1.1（jni_added 默认），统一走 v1_1。
    let iface = unsafe { &**env };
    let len = unsafe { (iface.v1_1.GetStringLength)(env, s) } as usize;
    if len == 0 {
        return Some(String::new());
    }
    let mut is_copy = false;
    let chars = unsafe { (iface.v1_1.GetStringChars)(env, s, &mut is_copy) };
    if chars.is_null() {
        return None;
    }
    // GetStringChars 契约：chars 指向 len 个有效 jchar
    let units = unsafe { std::slice::from_raw_parts(chars, len) };
    let out = utf16_units_to_string(units);
    unsafe { (iface.v1_1.ReleaseStringChars)(env, s, chars) };
    out
}

/// Rust str → Java String（NewString，UTF-16，emoji 安全）。失败返回 null。
///
/// # Safety
///
/// `env` 必须是当前线程有效且非空的 JNIEnv。
pub unsafe fn rust_to_jstring(env: *mut sys::JNIEnv, s: &str) -> jstring {
    if env.is_null() {
        return std::ptr::null_mut();
    }
    let units = string_to_utf16_units(s);
    let iface = unsafe { &**env };
    unsafe { (iface.v1_1.NewString)(env, units.as_ptr(), units.len() as jsize) }
}

/// 构造 `[Ljava/lang/String;` 数组。失败返回 null。
///
/// # Safety
///
/// `env` 必须是当前线程有效且非空的 JNIEnv。
pub unsafe fn string_array(env: *mut sys::JNIEnv, items: Vec<String>) -> sys::jobjectArray {
    if env.is_null() {
        return std::ptr::null_mut();
    }
    let iface = unsafe { &**env };
    let class = unsafe { (iface.v1_1.FindClass)(env, c"java/lang/String".as_ptr()) };
    if class.is_null() {
        return std::ptr::null_mut();
    }
    let arr = unsafe { (iface.v1_1.NewObjectArray)(env, items.len() as jsize, class, std::ptr::null_mut()) };
    if arr.is_null() {
        return std::ptr::null_mut();
    }
    for (i, s) in items.into_iter().enumerate() {
        let js = unsafe { rust_to_jstring(env, &s) };
        if js.is_null() {
            return std::ptr::null_mut();
        }
        unsafe { (iface.v1_1.SetObjectArrayElement)(env, arr, i as jsize, js) };
    }
    arr
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_roundtrip_emoji() {
        let s = "中文😄a";
        let units = string_to_utf16_units(s);
        assert_eq!(utf16_units_to_string(&units).unwrap(), s);
    }

    #[test]
    fn utf16_rejects_surrogate_split() {
        // 高半代理（😄 前半 0xD83D）无配对 → 拆半拒绝
        let units = [0xD83D_u16];
        assert_eq!(utf16_units_to_string(&units), None);
        // 低半代理无配对同样拒绝
        let units = [0xDE04_u16];
        assert_eq!(utf16_units_to_string(&units), None);
    }

    #[test]
    fn utf16_ascii_roundtrip() {
        assert_eq!(utf16_units_to_string(&[b'a' as u16, b'b' as u16]).unwrap(), "ab");
    }
}
