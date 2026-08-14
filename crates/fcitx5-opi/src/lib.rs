//! fcitx5-opi：Linux fcitx5 输入法插件的 Rust 逻辑出口。
//!
//! # 偏差说明（B0 记录，约束性决策）
//!
//! 离线环境下无法取得 fcitx5 Rust 绑定（github.com 不可达、crates.io 无该
//! crate），故本 crate 不实现 AddonInstance 注册（fcitx5 绑定示例不可对照）。
//! 本 crate 以 cdylib 导出 `opi_fcitx5_*` C 函数作为入口面，C++ AddonInstance
//! 胶水（后续任务）调用之。字符串约定：UTF-8 + 长度（ptr: *const u8,
//! len: usize），非 NUL 结尾；返回值由 Rust 侧分配，调用方用
//! `opi_ffi_free_string_utf8` 释放。语义与 opi-ffi 的 cabi.rs 一致。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, Once};

use engine_core::composer::Mode;

mod cabi;
pub mod candidate;
pub mod data_dir;
pub mod input_method;

pub use cabi::opi_fcitx5_init_dict;

use candidate::CandidateState;

/// 引擎单例：load 后可供 C++ 胶水共享（与 opi-ffi 的 SINGLETON 同构）。
static SINGLETON: Mutex<Option<CandidateState>> = Mutex::new(None);

/// 在单例上执行操作；未 load 时返回 None（调用方按哨兵处理）。
/// 毒化恢复：catch_unwind 吞 panic 时锁已毒化，into_inner 取回数据。
fn with_state<R>(f: impl FnOnce(&mut CandidateState) -> R) -> Option<R> {
    let mut g = SINGLETON.lock().unwrap_or_else(|p| p.into_inner());
    g.as_mut().map(f)
}

/// 一次性 panic hook：被 catch_unwind 捕获的 panic 只打印一行简洁日志到
/// stderr，避免向 fcitx5 宿主进程输出整段 backtrace 噪音。Once 保证多线程
/// 下只安装一次（set_hook 在已安装后再次调用会 panic）。
pub(crate) fn ensure_panic_hook() {
    static PANIC_HOOK: Once = Once::new();
    PANIC_HOOK.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            eprintln!("fcitx5-opi: engine panic caught: {info}");
        }));
    });
}

/// 装载引擎（替换单例）。`None`/空串 → 内置回退词库；坏路径 → false。
pub fn install(path: Option<&str>) -> Result<(), String> {
    let state = CandidateState::load(path)?;
    let mut guard = SINGLETON.lock().unwrap_or_else(|p| p.into_inner());
    *guard = Some(state);
    Ok(())
}

/// 0..=3 模式整数 ↔ Mode 转换（0=Pinyin 1=English 2=Number 3=Symbol）。
fn mode_from_int(m: i32) -> Option<Mode> {
    match m {
        0 => Some(Mode::Pinyin),
        1 => Some(Mode::English),
        2 => Some(Mode::Number),
        3 => Some(Mode::Symbol),
        _ => None,
    }
}

fn mode_to_int(m: Mode) -> i32 {
    match m {
        Mode::Pinyin => 0,
        Mode::English => 1,
        Mode::Number => 2,
        Mode::Symbol => 3,
    }
}

/// UTF-8 字符串句柄（Rust 侧分配，调用方负责 opi_ffi_free_string_utf8）。
#[repr(C)]
pub struct OpString {
    pub ptr: *const u8,
    pub len: usize,
}

impl OpString {
    /// 从 &str 分配 UTF-8 缓冲。空串返回空句柄（ptr 为 null）。
    /// 用 into_boxed_slice 使分配布局精确等于 len，free 端
    /// `Vec::from_raw_parts(ptr, len, len)` 的释放布局与之匹配，无 UB。
    pub fn from_utf8(s: &str) -> Self {
        if s.is_empty() {
            return Self::empty();
        }
        let bytes: Box<[u8]> = s.as_bytes().to_vec().into_boxed_slice();
        let ptr = bytes.as_ptr();
        let len = bytes.len();
        std::mem::forget(bytes);
        OpString { ptr, len }
    }

    /// 空句柄（ptr: null, len: 0）——错误/空串哨兵。
    pub fn empty() -> Self {
        OpString {
            ptr: std::ptr::null(),
            len: 0,
        }
    }
}

/// 释放 `opi_fcitx5_*` 返回的 OpString。
/// # Safety
///
/// `s` 必须是 `opi_fcitx5_*` 返回且尚未释放过的句柄（Rust 侧分配）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_ffi_free_string_utf8(s: OpString) {
    if !s.ptr.is_null() && s.len > 0 {
        let v = unsafe { Vec::from_raw_parts(s.ptr as *mut u8, s.len, s.len) };
        drop(v);
    }
}

/// 读取 UTF-8 输入串（ptr 为 null → None）。无效 UTF-8 按 lossy 容错。
///
/// # Safety
///
/// `ptr` 必须指向至少 `len` 个字节的有效内存（或为 null）。
pub(crate) unsafe fn read_utf8(ptr: *const u8, len: usize) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    // Safety: 调用方保证 ptr 指向至少 len 字节的有效内存
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    Some(String::from_utf8_lossy(bytes).into_owned())
}

/// 文本数组 → JSON 字符串（OpString）。
fn texts_to_json(texts: Vec<String>) -> OpString {
    let json = serde_json::to_string(&texts).unwrap_or_default();
    OpString::from_utf8(&json)
}

// ---------- C 入口面（B0 约定 11 个 + B2 新增 key_event；B3 init_dict 见 cabi.rs） ----------

/// load(path: const uint8_t*, len) -> bool。null/空串 → 内置回退词库；坏路径 → false。
/// # Safety
///
/// `ptr` 必须指向至少 `len` 字节的有效内存（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_load(ptr: *const u8, len: usize) -> bool {
    ensure_panic_hook();
    catch_unwind(AssertUnwindSafe(|| {
        let path = unsafe { read_utf8(ptr, len) };
        install(path.as_deref()).is_ok()
    }))
    .unwrap_or(false)
}

/// inputKey(ptr, len) -> OpString：单字符键路由到引擎，返回引擎输出
/// （如英文模式已提交文本，通常为空串）。空串/多字符/非 ASCII → 空串。
/// # Safety
///
/// `ptr` 必须指向至少 `len` 字节的有效内存（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_input_key(ptr: *const u8, len: usize) -> OpString {
    ensure_panic_hook();
    let out = catch_unwind(AssertUnwindSafe(|| {
        let ch = unsafe { read_utf8(ptr, len) }.unwrap_or_default();
        let mut chars = ch.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else {
            return String::new(); // 边界：拒绝空串/多字符
        };
        with_state(|s| match input_method::handle_key(s, c as u32, 0) {
            input_method::KeyAction::Input(out) => out,
            input_method::KeyAction::EngineHandled | input_method::KeyAction::PassThrough => {
                String::new()
            }
        })
        .unwrap_or_default()
    }))
    .unwrap_or_default();
    OpString::from_utf8(&out)
}

/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_backspace() {
    ensure_panic_hook();
    let _ = catch_unwind(AssertUnwindSafe(|| with_state(|s| s.backspace())));
}

/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_clear() {
    ensure_panic_hook();
    let _ = catch_unwind(AssertUnwindSafe(|| with_state(|s| s.clear())));
}

/// select(index) -> OpString：提交当前页第 index 个候选（页内索引）。
/// 越界返回空串。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_select(index: usize) -> OpString {
    ensure_panic_hook();
    let out = catch_unwind(AssertUnwindSafe(|| {
        with_state(|s| s.select(index)).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpString::from_utf8(&out)
}

/// switchMode(mode: i32)。0=Pinyin 1=English 2=Number 3=Symbol，越界忽略。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_switch_mode(mode: i32) {
    ensure_panic_hook();
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(m) = mode_from_int(mode) {
            with_state(|s| s.switch_mode(m));
        }
    }));
}

/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_set_shift(on: bool) {
    ensure_panic_hook();
    let _ = catch_unwind(AssertUnwindSafe(|| with_state(|s| s.set_shift(on))));
}

/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_input_space() -> OpString {
    ensure_panic_hook();
    let out = catch_unwind(AssertUnwindSafe(|| {
        with_state(|s| s.input_space()).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpString::from_utf8(&out)
}

/// candidates(limit) -> JSON 文本数组：当前页候选（上限 min(limit, 8)）。
/// 翻页经 B2 路由表（PageUp/PageDown → prev/next_page），无独立 C 出口。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_candidates(limit: usize) -> OpString {
    ensure_panic_hook();
    let texts = catch_unwind(AssertUnwindSafe(|| {
        with_state(|s| s.candidates().into_iter().take(limit).collect::<Vec<_>>())
            .unwrap_or_default()
    }))
    .unwrap_or_default();
    texts_to_json(texts)
}

/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_buffer() -> OpString {
    ensure_panic_hook();
    let out = catch_unwind(AssertUnwindSafe(|| {
        with_state(|s| s.buffer()).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpString::from_utf8(&out)
}

/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_mode() -> i32 {
    ensure_panic_hook();
    catch_unwind(AssertUnwindSafe(|| {
        with_state(|s| mode_to_int(s.mode())).unwrap_or(0)
    }))
    .unwrap_or(0)
}

/// 按键事件结果（B2 路由：`opi_fcitx5_key_event` 的返回值）。
#[repr(C)]
pub struct KeyEventResult {
    /// 0=PassThrough（转交客户端） 1=EngineHandled（已消费） 2=Commit（提交 text）。
    pub action: i32,
    /// action==2 时携带提交文本（Rust 侧分配，调用方 free）。
    pub text: OpString,
}

/// keyEvent 路由入口（B2）：`keyval` + fcitx5 `KeyState` 修饰位 → 动作 + 提交文本。
/// 语义与 Android `KeyRouter` 一致（详见 input_method 模块文档）。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_key_event(keyval: u32, states: u32) -> KeyEventResult {
    ensure_panic_hook();
    catch_unwind(AssertUnwindSafe(|| {
        with_state(|s| match input_method::handle_key(s, keyval, states) {
            input_method::KeyAction::Input(t) => KeyEventResult {
                action: 2,
                text: OpString::from_utf8(&t),
            },
            input_method::KeyAction::EngineHandled => KeyEventResult {
                action: 1,
                text: OpString::empty(),
            },
            input_method::KeyAction::PassThrough => KeyEventResult {
                action: 0,
                text: OpString::empty(),
            },
        })
    }))
    .ok()
    .flatten()
    .unwrap_or_else(|| KeyEventResult {
        action: 0,
        text: OpString::empty(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 读取 OpString 内容（null ptr → 空串）并释放。
    fn read_and_free(s: OpString) -> String {
        let out = if s.ptr.is_null() {
            String::new()
        } else {
            let bytes = unsafe { std::slice::from_raw_parts(s.ptr, s.len) };
            String::from_utf8_lossy(bytes).into_owned()
        };
        unsafe { opi_ffi_free_string_utf8(s) };
        out
    }

    #[test]
    fn mode_int_roundtrip() {
        assert_eq!(mode_from_int(0), Some(Mode::Pinyin));
        assert_eq!(mode_from_int(3), Some(Mode::Symbol));
        assert_eq!(mode_from_int(4), None);
        assert_eq!(mode_from_int(-1), None);
        assert_eq!(mode_to_int(Mode::English), 1);
        assert_eq!(mode_to_int(Mode::Number), 2);
    }

    #[test]
    fn opstring_roundtrip() {
        let s = OpString::from_utf8("你好");
        let bytes = unsafe { std::slice::from_raw_parts(s.ptr, s.len) };
        assert_eq!(std::str::from_utf8(bytes).unwrap(), "你好");
        unsafe { opi_ffi_free_string_utf8(s) };
        let e = OpString::from_utf8("");
        assert!(e.ptr.is_null());
        assert_eq!(e.len, 0);
    }

    #[test]
    fn install_singleton_fallback_and_path() {
        assert!(install(None).is_ok());
        assert_eq!(with_state(|s| s.buffer()), Some(String::new()));
        // 坏路径 → Err（load_or_fallback 原样语义，不回退）
        assert!(install(Some("/nonexistent/opi.dict")).is_err());
        // 单例保持上一次成功装载可用
        assert_eq!(with_state(|s| s.buffer()), Some(String::new()));
    }

    #[test]
    fn input_key_invalid_utf8_is_lossy_and_safe() {
        assert!(install(None).is_ok());
        // 单字节无效 UTF-8 → lossy 替换为 U+FFFD，非 ASCII → 引擎忽略 → 空串
        let raw = [0xffu8];
        let out = unsafe { opi_fcitx5_input_key(raw.as_ptr(), raw.len()) };
        assert_eq!(read_and_free(out), "");
        // 多字节无效序列 → lossy 后为多字符 → 边界拒绝 → 空串
        let raw = [0xc3u8, 0x28u8];
        let out = unsafe { opi_fcitx5_input_key(raw.as_ptr(), raw.len()) };
        assert_eq!(read_and_free(out), "");
        // 不 panic、不返回垃圾
        assert_eq!(with_state(|s| s.buffer()), Some(String::new()));
    }

    #[test]
    fn input_key_null_ptr_with_len_returns_empty() {
        assert!(install(None).is_ok());
        // read_utf8 先判 null（lib.rs read_utf8 首行），len>0 也不触碰内存
        let out = unsafe { opi_fcitx5_input_key(std::ptr::null(), 5) };
        assert_eq!(read_and_free(out), "");
    }

    #[test]
    fn opstring_empty_roundtrip() {
        let e = OpString::from_utf8("");
        assert!(e.ptr.is_null());
        assert_eq!(e.len, 0);
        // 空句柄 free 为无操作且安全；再读也为空
        unsafe { opi_ffi_free_string_utf8(e) };
        assert_eq!(read_and_free(OpString::empty()), "");
    }

    #[test]
    fn candidates_without_buffer_is_empty_json() {
        assert!(install(None).is_ok());
        let out = unsafe { opi_fcitx5_candidates(8) };
        assert_eq!(read_and_free(out), "[]");
    }

    #[test]
    fn key_event_english_pass_through_commits_lowercase() {
        assert!(install(None).is_ok());
        // 切英文：直传 'a'（action=2 提交）
        unsafe { opi_fcitx5_switch_mode(1) };
        let r = unsafe { opi_fcitx5_key_event(97, 0) };
        assert_eq!(r.action, 2);
        assert_eq!(read_and_free(r.text), "a");
        // 切回拼音，英文直传不污染缓冲
        unsafe { opi_fcitx5_switch_mode(0) };
        assert_eq!(with_state(|s| s.buffer()), Some(String::new()));
    }

    #[test]
    fn key_event_pinyin_letter_handled_and_space_commits() {
        assert!(install(None).is_ok());
        unsafe { opi_fcitx5_switch_mode(0) };
        let r = unsafe { opi_fcitx5_key_event(104, 0) }; // 'h'
        assert_eq!(r.action, 1); // EngineHandled
        assert_eq!(with_state(|s| s.buffer()), Some("h".to_string()));
        // 缓冲非空空格 → 提交并清空（提交文本取决于词库，只断言非空）
        let r = unsafe { opi_fcitx5_key_event(32, 0) };
        assert_eq!(r.action, 2);
        assert!(!read_and_free(r.text).is_empty());
        let r = unsafe { opi_fcitx5_key_event(97, 0) }; // 'a'
        assert_eq!(r.action, 1);
        let r = unsafe { opi_fcitx5_key_event(32, 0) }; // buffer 非空 → 空格提交
        assert_eq!(r.action, 2);
        assert!(!read_and_free(r.text).is_empty());
    }

    #[test]
    fn key_event_ctrl_passes_through_and_shift_consumed() {
        assert!(install(None).is_ok());
        // Ctrl+C → 直通
        let r = unsafe { opi_fcitx5_key_event(99, 1 << 2) };
        assert_eq!(r.action, 0);
        // shift 按下（无修饰）→ EngineHandled
        let r = unsafe { opi_fcitx5_key_event(0xffe1, 0) };
        assert_eq!(r.action, 1);
        // 英文空缓冲 + single shift → 大写提交且消费
        unsafe { opi_fcitx5_switch_mode(1) };
        let r = unsafe { opi_fcitx5_key_event(97, 0) };
        assert_eq!(r.action, 2);
        assert_eq!(read_and_free(r.text), "A");
        let r = unsafe { opi_fcitx5_key_event(97, 0) };
        assert_eq!(read_and_free(r.text), "a");
    }
}
