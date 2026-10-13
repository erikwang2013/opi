// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「属性设置」**旋钮族**的 C 出口（2026-10-14 从 `lib.rs` 拆出：加拼音方式后那个
//! 文件 526 行撞 500 行门禁；与 `cabi.rs` 同一条拆分手续）。
//!
//! 调用方是 C++ 的 `applyOpiSettings`（`cpp/opi_config.h`）：**冷路径**（构造 /
//! reloadConfig / setConfig 三处），不进按键热路径。语义与 opi-ffi 的同名出口逐条
//! 一致（共享同一个 engine-core 实现，只是单例与错误姿态各按本轨的来）。
//!
//! ⚠️ 拆出去**不等于脱离契约**：这些函数仍以 `#[unsafe(no_mangle)]` 导出（符号名
//! 不变），`lib.rs` 的 `pub use switches::*;` 让测试与内部调用方照旧引用 —— 头文件
//! 声明面（`cpp/opi_rust_ffi.h`）跟着 `lib.rs` 的导出表走，不跟着文件走。

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::{ensure_panic_hook, read_utf8, with_state};

/// setLearner(on)：学习开关（「属性设置」的旋钮之一；值来自 addon 配置的 `Learn`）。
/// 配对：`tsf-opi/src/logic.rs` 的同名方法。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_set_learner(on: bool) {
    ensure_panic_hook();
    let _ = catch_unwind(AssertUnwindSafe(|| with_state(|s| s.set_learner(on))));
}

/// setChinesePunct(on)：中文标点开关（值来自 addon 配置的 `ChinesePunct`；与全角
/// 是**两个独立开关**，见 engine.rs 的 `punct_text`）。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_set_chinese_punct(on: bool) {
    ensure_panic_hook();
    let _ = catch_unwind(AssertUnwindSafe(|| with_state(|s| s.set_chinese_punct(on))));
}

/// setFuzzy(on)：模糊音开关（「属性设置」的旋钮之一）。
/// 配置来源 = fcitx5 的 **原生 addon 配置**（`~/.config/fcitx5/conf/opi_fcitx5.conf`）：
/// C++ 侧 `OpiConfig` 声明字段、`reloadConfig()`/`setConfig()` 里把它推到这里
/// （见 `cpp/opi_fcitx5.cpp`），配置工具改一下即时生效。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_set_fuzzy(on: bool) {
    ensure_panic_hook();
    let _ = catch_unwind(AssertUnwindSafe(|| with_state(|s| s.set_fuzzy(on))));
}

/// setJianpin(on)：简拼开关（同上）。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_set_jianpin(on: bool) {
    ensure_panic_hook();
    let _ = catch_unwind(AssertUnwindSafe(|| with_state(|s| s.set_jianpin(on))));
}

/// setCorrection(on)：拼音纠错开关（2026-10-13；配置键 `Correction`）。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_set_correction(on: bool) {
    ensure_panic_hook();
    let _ = catch_unwind(AssertUnwindSafe(|| with_state(|s| s.set_correction(on))));
}

/// setFuzzyPairsNames(names)：模糊音**逐对**列表（逗号分隔组名，名字表见
/// `engine_core::fuzzy::pair_names`）。**空串 = 空操作** —— 回落旧布尔的裁决在
/// C++ 的 `applyOpiSettings` 里（两档都写这里的话，老配置的 `Fuzzy=False` 会被
/// 默认串覆盖）；名字认不得整条忽略。`(ptr, len)` = UTF-8 串，null/空 = 空操作。
/// # Safety
///
/// `ptr` 必须指向至少 `len` 字节的有效内存（或为 null，视为空操作）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_set_fuzzy_pairs(ptr: *const u8, len: usize) {
    ensure_panic_hook();
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let Some(names) = (unsafe { read_utf8(ptr, len) }).filter(|n| !n.is_empty()) else {
            return;
        };
        let _ = with_state(|s| s.set_fuzzy_pairs(&names));
    }));
}

/// setShuangpinLayout(index)：双拼布局（2026-10-13；配置键 `ShuangpinLayout`）。
/// 下标口径与 `opi_set_shuangpin_layout` 逐值一致（0=自然码…5=拼音加加，
/// `engine_core::shuangpin_tables::SHUANGPIN_TABLES` 表序）；**过大钳到末位、负数钳到 0**
/// （同那个出口）。C++ 侧传的是它自己的枚举序，两者同序由「四端名字锁步门禁」
/// （desktop `SettingsTest` 现读 `opi_config.h` 的枚举与 i18n 名）钉住。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_set_shuangpin_layout(index: i32) {
    ensure_panic_hook();
    let _ = catch_unwind(AssertUnwindSafe(|| {
        with_state(|s| s.set_shuangpin_layout(index.max(0) as usize));
    }));
}

/// setScheme(code)：拼音方式 / 输入方案（2026-10-14；配置键 `PinyinMethod`）。
/// 取值 = `Scheme::code`（0=全拼 4=双拼 —— C++ 的 `OpiPinyinMethod` 枚举值就是这两个
/// 码，见 opi_config.h）；**越界 = 空操作**（配置枚举永远合法，越界只可能来自手改的
/// .so/宿主错配，静默不动比跳回全拼安全）。**清缓冲**（`Engine::set_scheme` 的语义）。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_set_scheme(code: i32) {
    ensure_panic_hook();
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(s) = engine_core::Scheme::from_code(code) {
            with_state(|st| st.set_scheme(s));
        }
    }));
}
