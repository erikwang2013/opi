//! C ABI 出口（iOS M7 预留）：`opi_*` 函数，UTF-16 字符串（ptr+len），
//! Rust 侧分配，调用方用 `opi_ffi_free_string` 释放。语义与 JNI 出口（jni.rs）
//! 完全一致，共享 api::SINGLETON 与内部实现，无重复逻辑。
//! 多字符串返回值（candidates/searchSymbols/symbolsInBlock）编码为 JSON 文本数组。

use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::api;

/// UTF-16 字符串句柄（Rust 侧分配，调用方负责 opi_ffi_free_string）。
#[repr(C)]
pub struct OpiString {
    pub ptr: *const u16,
    pub len: usize,
}

impl OpiString {
    /// 从 &str 分配 UTF-16 缓冲。空串返回空句柄（ptr 为 null）。
    /// 用 into_boxed_slice 使分配布局精确等于 len，free 端
    /// `Vec::from_raw_parts(ptr, len, len)` 的释放布局与之匹配，无 UB。
    pub fn from_utf16(s: &str) -> Self {
        if s.is_empty() {
            return Self::empty();
        }
        let units: Box<[u16]> = s.encode_utf16().collect::<Vec<u16>>().into_boxed_slice();
        let ptr = units.as_ptr();
        let len = units.len();
        std::mem::forget(units);
        OpiString { ptr, len }
    }

    /// 空句柄（ptr: null, len: 0）——错误/空串哨兵。
    pub fn empty() -> Self {
        OpiString { ptr: std::ptr::null(), len: 0 }
    }
}

/// 释放 `opi_*` 返回的 OpiString。
/// # Safety
///
/// `s` 必须是 `opi_*` 返回且尚未释放过的句柄（Rust 侧分配）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_ffi_free_string(s: OpiString) {
    if !s.ptr.is_null() && s.len > 0 {
        let v = unsafe { Vec::from_raw_parts(s.ptr as *mut u16, s.len, s.len) };
        drop(v);
    }
}

/// 读取 UTF-16 输入串（ptr 为 null → None）。
///
/// # Safety
///
/// `ptr` 必须指向至少 `len` 个 u16 的有效内存（或为 null）。
unsafe fn read_utf16(ptr: *const u16, len: usize) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    // Safety: 调用方保证 ptr 指向至少 len 个 u16 的有效内存
    let units = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf16(units).ok()
}

/// 共享文本数组 → JSON 字符串（OpiString）。
fn texts_to_json(texts: Vec<String>) -> OpiString {
    OpiString::from_utf16(&api::texts_json(&texts))
}

// ---------- 19 个 C 函数 ----------

/// load(path: const uint16_t*, len) -> bool。null/空串 → 内置回退词库；坏路径 → false。
/// # Safety
///
/// `ptr` 必须指向至少 `len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_load(path: *const u16, len: usize) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        let path = unsafe { read_utf16(path, len) };
        api::install(path.as_deref()).is_ok()
    }))
    .unwrap_or(false)
}

/// loadTrad(path: const uint16_t*, len) -> bool。空/坏路径/引擎未加载 → false
/// （繁体模式回退简体库，见 spec 错误处理）。
/// # Safety
///
/// `ptr` 必须指向至少 `len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_load_trad(path: *const u16, len: usize) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        let path = unsafe { read_utf16(path, len) }.unwrap_or_default();
        api::install_trad(&path).is_ok()
    }))
    .unwrap_or(false)
}

/// inputKey(ch) -> OpiString。永不 panic。单字符外返回空串。
/// # Safety
///
/// `ptr` 必须指向至少 `len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_input_key(ptr: *const u16, len: usize) -> OpiString {
    let out = catch_unwind(AssertUnwindSafe(|| {
        let ch = unsafe { read_utf16(ptr, len) }.unwrap_or_default();
        api::with_engine(|e| e.input_key(ch)).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpiString::from_utf16(&out)
}

/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_backspace() {
    let _ = catch_unwind(AssertUnwindSafe(|| api::with_engine(|e| e.backspace())));
}

/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clear() {
    let _ = catch_unwind(AssertUnwindSafe(|| api::with_engine(|e| e.clear())));
}

/// select(index) -> OpiString。越界返回空串（旧语义）。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_select(index: usize) -> OpiString {
    let out = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.select(index)).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpiString::from_utf16(&out)
}

/// switchMode(mode: i32)。0=Pinyin 1=English 2=Number 3=Symbol，越界忽略。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_switch_mode(mode: i32) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(m) = api::mode_from_int(mode) {
            api::with_engine(|e| e.switch_mode(m.into()));
        }
    }));
}

/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_shift(on: bool) {
    let _ = catch_unwind(AssertUnwindSafe(|| api::with_engine(|e| e.set_shift(on))));
}

/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_input_space() -> OpiString {
    let out = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.input_space()).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpiString::from_utf16(&out)
}

/// candidates(limit) -> JSON 文本数组。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_candidates(limit: usize) -> OpiString {
    let texts = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| api::candidate_texts(e, limit)).unwrap_or_default()
    }))
    .unwrap_or_default();
    texts_to_json(texts)
}

/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_buffer() -> OpiString {
    let out = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.buffer()).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpiString::from_utf16(&out)
}

/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_mode() -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| api::mode_to_int(e.mode().into())).unwrap_or(0)
    }))
    .unwrap_or(0)
}

/// searchSymbols(keyword) -> JSON 文本数组。
/// # Safety
///
/// `ptr` 必须指向至少 `len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_search_symbols(ptr: *const u16, len: usize) -> OpiString {
    let texts = catch_unwind(AssertUnwindSafe(|| {
        let kw = unsafe { read_utf16(ptr, len) }.unwrap_or_default();
        api::with_engine(|e| api::search_symbol_texts(e, &kw)).unwrap_or_default()
    }))
    .unwrap_or_default();
    texts_to_json(texts)
}

/// symbolBlocks() -> JSON：`[{id,start,end,name,common}]`。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_symbol_blocks() -> OpiString {
    let json = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| api::symbol_blocks_json(e)).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpiString::from_utf16(&json)
}

/// symbolsInBlock(id: i16) -> JSON 文本数组。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_symbols_in_block(id: i16) -> OpiString {
    let texts = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| api::symbol_texts(e, id.max(0) as u16)).unwrap_or_default()
    }))
    .unwrap_or_default();
    texts_to_json(texts)
}

/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_learner_enabled() -> bool {
    catch_unwind(AssertUnwindSafe(|| api::with_engine(|e| e.learner_enabled()).unwrap_or(false))).unwrap_or(false)
}

/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_learner(enabled: bool) {
    let _ = catch_unwind(AssertUnwindSafe(|| api::with_engine(|e| e.set_learner(enabled))));
}

/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clear_user_words() {
    let _ = catch_unwind(AssertUnwindSafe(|| api::with_engine(|e| e.clear_user_words())));
}

/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_export_user_words() -> OpiString {
    let out = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.export_user_words()).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpiString::from_utf16(&out)
}

