// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

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

/// 全局 `SINGLETON` 是进程级共享状态，而 `cargo test` 默认多线程并行跑用例 ——
/// 触碰它的用例（本文件 tests 与 data_dir 的 tests 在**同一个测试二进制**里）
/// 都会 `install()` 重置它，因此会互相踩状态。实测（12 次连跑）：约 40% 的运行
/// 会随机红一条，使「全绿」门禁失去意义（真回归与假失败无法区分）。
/// 故本 crate 只留这一把测试锁，跨模块共用；锁与 install 绑成一次调用，
/// 避免新增用例漏加。
#[cfg(test)]
pub(crate) static SERIAL: Mutex<()> = Mutex::new(());

/// 取串行锁并重置单例。返回值必须绑定到变量活到用例结束（`let _g = ...`），
/// 写成 `let _ = ...` 会立即释放锁，等于没加。
#[cfg(test)]
pub(crate) fn serial_install() -> std::sync::MutexGuard<'static, ()> {
    let guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    assert!(install(None).is_ok());
    guard
}

/// 0..=4 模式整数 ↔ Mode 转换（0=Pinyin 1=English 2=Number 3=Symbol 4=Traditional，
/// 与 opi-ffi 的 JNI/C 模式整数约定一致）。
fn mode_from_int(m: i32) -> Option<Mode> {
    match m {
        0 => Some(Mode::Pinyin),
        1 => Some(Mode::English),
        2 => Some(Mode::Number),
        3 => Some(Mode::Symbol),
        4 => Some(Mode::Traditional),
        _ => None,
    }
}

fn mode_to_int(m: Mode) -> i32 {
    match m {
        Mode::Pinyin => 0,
        Mode::English => 1,
        Mode::Number => 2,
        Mode::Symbol => 3,
        Mode::Traditional => 4,
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
        // 键值空间只在 ASCII 段等于码点：真实 fcitx5 的非 ASCII keysym 是
        // `0x0100_0000 | 码点`。而 U+FF00–U+FFFF 里 9 个字符的码点**恰好等于**
        // 某个 keysym（`－`=KEY_RETURN、`（`=KEY_BACK_SPACE…），此前被误当控制键。
        if !c.is_ascii() {
            return String::new();
        }
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

/// switchMode(mode: i32)。0=Pinyin 1=English 2=Number 3=Symbol 4=Traditional，越界忽略。
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

/// 全角 ⇄ 半角切换（用户裁决「除自动全角外，再加一个切换键」）。
/// 返回**切换后**的状态（true = 全角）。
///
/// **键位不在本层**（与 `toggle_symbol`/`toggle_fullwidth` 同一条：引擎只出语义）：
/// 键位在插件侧 `cpp/opi_fcitx5.cpp` 的 `handleFullwidthHotkey`（Shift+Space）。
/// 本函数是 C++ 触达引擎的那条缝 —— 没有它，C++ 侧没有任何办法翻这个开关。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_toggle_fullwidth() -> bool {
    ensure_panic_hook();
    catch_unwind(AssertUnwindSafe(|| {
        with_state(|s| s.toggle_fullwidth()).unwrap_or(false)
    }))
    .unwrap_or(false)
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

// 单测独立成文件（`#[path]` 引入）以保持本文件 <500 行，与 input_method_tests.rs 同惯例。
#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
