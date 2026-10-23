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

/// effectiveChinesePunct()：**生效的**中文标点状态（true = 此刻按标点键出中文标点）。
/// 与 [`opi_fcitx5_set_chinese_punct`] **不是同一个量**：那个是设置值（不随模式变），
/// 这个按模式分派后的生效值（中文类模式 = 设置值；英文/数字/符号恒 false）—— 判据
/// 唯一在引擎（`Engine::effective_chinese_punct`，与 `punct_text` 同一条线），本层只转发。
/// 状态栏 `punct=` 那一格读它（2026-10-15 用户裁决「切换中英文的时候，标点符号也
/// 联动切换」的**显示**面落地；行为本来就对）。
///
/// 直接读 `CandidateState.engine`（crate 内可见）：与 [`opi_fcitx5_scheme`] 同一条
/// 理由 —— 给本轨 façade 加方法会让 `candidate.rs` ↔ `tsf-opi/src/logic.rs` 的逐行
/// 同构漂一格，不值得为一个出口动那个面。
///
/// 引擎未装载 → false（半角）：与 `opi_fcitx5_scheme()` 未装载给 0（全拼）同一个
/// 缺省姿态 —— 空引擎下「半角」是最中性的读数，调用方不用区分这一格。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_effective_chinese_punct() -> bool {
    ensure_panic_hook();
    catch_unwind(AssertUnwindSafe(|| {
        with_state(|s| s.engine.effective_chinese_punct()).unwrap_or(false)
    }))
    .unwrap_or(false)
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

/// reloadCustomPhrases()：从数据目录重读中英两套短语文件并整表推给引擎
/// （2026-10-24）。调用点是 C++ 的 `applyOpiSettings`（构造 / reloadConfig /
/// setConfig 三条冷路径）—— 设置窗保存后经 `ReloadAddonConfig` 走到这里。
/// 任一文件缺失/不合法 → 该套空表 + 启用（见 `custom_phrases::load_*`）。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_reload_custom_phrases() {
    ensure_panic_hook();
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let state = crate::custom_phrases::load_custom_phrases();
        let _ = with_state(|s| s.apply_custom_phrases(state));
        let state_en = crate::custom_phrases::load_english_custom_phrases();
        let _ = with_state(|s| s.apply_english_custom_phrases(state_en));
    }));
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

/// scheme()：读回当前方案码（`Scheme::code`；0=全拼 4=双拼 —— 与 `set_scheme`
/// 同一个值域）。**读侧出口**（2026-10-14 与 set_scheme 同时落地）：opi-ctl 的
/// `mode` 动词要按「mode + scheme」派生对外状态 token（pinyin / en26，未来的
/// wubi 也只靠 scheme 跟全拼分得开）—— 不落本地镜像，见 `cpp/opi_ctl_mode.h`。
/// 直接读 `CandidateState.engine`（crate 内可见）：本轨还没有 scheme 的 façade 读口，
/// 而给它加一个方法会让 `candidate.rs` ↔ `tsf-opi/src/logic.rs` 的逐行同构漂一格
/// （两轨同构纪律），不值得为一个出口动那个面。
///
/// 引擎未装载 → 0（全拼）：与 `opi_fcitx5_mode()` 未装载给 0（Pinyin）同一个缺省
/// 姿态 —— 空引擎下「拼音 + 全拼」是最中性的读数，调用方不用区分这一格。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_scheme() -> i32 {
    ensure_panic_hook();
    catch_unwind(AssertUnwindSafe(|| {
        with_state(|s| s.engine.scheme().code()).unwrap_or(0)
    }))
    .unwrap_or(0)
}

// ---- 五笔旋钮族（2026-10-14，M4 第二波）----
//
// 整数码 ↔ 枚举的换算**不在本层**：`engine_core::{WubiStyle, WubiMixedEntry,
// WubiReturnStyle}::from_code/code` 是四端唯一的真源（本 crate 不依赖 opi-ffi，
// 所以更不能在这里抄第二张表）。写侧越界一律**忽略**（与 `opi_set_scheme` 同款）。

/// loadWubi(path) -> bool：装载五笔码表（C++ 侧按 `WubiPlan` 选 wubi86/98 后传路径）。
/// 坏路径/引擎未装载 → false（方案回退普通路径，与 `opi_fcitx5_load_stroke` 同姿态）。
/// # Safety
///
/// `ptr` 必须指向至少 `len` 字节的有效内存（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_load_wubi(ptr: *const u8, len: usize) -> bool {
    ensure_panic_hook();
    catch_unwind(AssertUnwindSafe(|| {
        let Some(path) = (unsafe { read_utf8(ptr, len) }).filter(|p| !p.is_empty()) else {
            return false;
        };
        with_state(|s| s.load_wubi(&path)).unwrap_or(false)
    }))
    .unwrap_or(false)
}

/// setWubiStyle(style)：0=纯五笔 1=五笔拼音混输；越界忽略。**不清缓冲**。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_set_wubi_style(style: i32) {
    ensure_panic_hook();
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(s) = engine_core::WubiStyle::from_code(style) {
            with_state(|st| st.set_wubi_style(s));
        }
    }));
}

/// setWubiMixedEntry(entry)：0=直接字母 1=z键（只在混输下有语义）；越界忽略。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_set_wubi_mixed_entry(entry: i32) {
    ensure_panic_hook();
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(x) = engine_core::WubiMixedEntry::from_code(entry) {
            with_state(|st| st.set_wubi_mixed_entry(x));
        }
    }));
}

/// setWubiCandidateLimit(n)：1..=9（**负数/0 归 1、过大由引擎钳到 9** —— 坏值钳到
/// 确定值，与 `opi_fcitx5_set_shuangpin_layout` 同纪律）。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_set_wubi_candidate_limit(n: i32) {
    ensure_panic_hook();
    let _ = catch_unwind(AssertUnwindSafe(|| {
        with_state(|st| st.set_wubi_candidate_limit(n.max(1) as usize));
    }));
}

/// setWubiReturn(style)：0=清空 1=上屏原文；越界忽略。**设置位**：消费在路由层。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_set_wubi_return_style(style: i32) {
    ensure_panic_hook();
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(s) = engine_core::WubiReturnStyle::from_code(style) {
            with_state(|st| st.set_wubi_return_style(s));
        }
    }));
}

/// setWubiAutoCommit(on)：四码唯一自动上屏开关（缺省开）。
/// # Safety
///
/// 无外部内存参数；共享单例由内部 Mutex 保护，跨线程调用安全。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fcitx5_set_wubi_auto_commit(on: bool) {
    ensure_panic_hook();
    let _ = catch_unwind(AssertUnwindSafe(|| {
        with_state(|st| st.set_wubi_auto_commit(on));
    }));
}
