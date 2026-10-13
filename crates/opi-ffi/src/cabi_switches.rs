// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「属性设置」**旋钮族**的 C 出口（2026-10-13 从 `cabi.rs` 拆出：加拼音纠错一批后
//! 那个文件 505 行撞 500 行门禁；与 `jni_switches.rs` 同一次拆分、同一主题，镜像
//! `cabi_user_words.rs` / `cabi_symbols.rs` 的拆族手续）。
//!
//! ⚠️ 拆出去**不等于脱离契约**：`tests/c_abi/checker.rs` 的 `read_cabi()` 必须把本文件
//! 一起读（否则头文件比对会报一堆假「多出」）；`cabi.rs` 顶部 `pub use` 保持旧路径
//! （`opi_ffi::cabi::opi_set_fuzzy` 等调用方与测试一行不改）。

#![allow(clippy::missing_safety_doc)]
// ⚠️ 同 `cabi.rs` 顶上那条：按模块生效。

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::api;
use crate::cabi::utf16::read_utf16;
use crate::cabi::OpiString;

/// chinesePunct() -> bool：中文标点表开关读侧（`.。`、引号交替那一档）。未装载 → false。
///
/// ⚠️ **与 `opi_fullwidth_state` 不是一档**：本档是**用户偏好**，`opi_switch_mode`
/// **不重置它** —— 硬规则「切模式后必须重读全角」**不适用于本出口**。仍必须有读侧：
/// 设置页与下次启动的持久化都会改动它。关掉本档**不等于半角** —— 表未命中的字符
/// 仍走全角那一档（两闸各管一段，真值表见 `tests/cabi_fullwidth.rs`）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_chinese_punct() -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.chinese_punct()).unwrap_or(false)
    }))
    .unwrap_or(false)
}

/// setChinesePunct(on: bool)：设置中文标点表是否生效。**无返回值** —— 它是设置项
/// （勾选框知道要设成什么值），要拿结果请读 `opi_chinese_punct()`（未装载时是空操作）。
/// **写入口有两个**：设置项走本出口，触发键走 `opi_toggle_chinese_punct()`（那个有返回值，
/// 模糊音开关的当前值（「属性设置」的读侧，供设置界面回读；写侧见 `opi_set_fuzzy`）。
/// 哨兵与 `opi_learner_enabled` 同档：**引擎未装载 → `false`**（模块头那句「未装载
/// 哨兵一律是 Default」）。别把它当成"开关的默认值"读 —— 默认值的真源是
/// `engine-core` 的 `RankOptions::default()`（两个都开）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fuzzy_enabled() -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.fuzzy_enabled()).unwrap_or(false)
    }))
    .unwrap_or(false)
}

/// 简拼开关的当前值（同上）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_jianpin_enabled() -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.jianpin_enabled()).unwrap_or(false)
    }))
    .unwrap_or(false)
}

/// setFuzzy(on)：模糊音开关（「属性设置」四个旋钮之一；同族见 `opi_set_learner` /
/// `opi_set_chinese_punct` / `opi_set_jianpin`）。各端的配置面在各自那一层：
/// Linux = fcitx5 addon 配置、Windows = settings.ini、Android = 设置页。
/// 与触发键那档不同，本出口**不读结果**（客户端知道自己要设什么值）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_fuzzy(on: bool) {
    let _ = catch_unwind(AssertUnwindSafe(|| api::with_engine(|e| e.set_fuzzy(on))));
}

/// setJianpin(on)：简拼开关（同上）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_jianpin(on: bool) {
    let _ = catch_unwind(AssertUnwindSafe(|| api::with_engine(|e| e.set_jianpin(on))));
}

/// 拼音纠错开关的读侧（2026-10-13；写侧见 `opi_set_correction`）。
/// 未装载 → `false`（哨兵语义同族；**真缺省是 true**）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_correction_enabled() -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.correction_enabled()).unwrap_or(false)
    }))
    .unwrap_or(false)
}

/// setCorrection(on)：拼音纠错开关（用户裁决「拼音纠错（混合参与排序）」，
/// 缺省开；变体见 `engine_core::correction`）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_correction(on: bool) {
    let _ = catch_unwind(AssertUnwindSafe(|| api::with_engine(|e| e.set_correction(on))));
}

/// 模糊音**逐对**启用集的读侧（2026-10-13 用户裁决「可多选类似 z=zh」；写侧见
/// `opi_set_fuzzy_pairs`）。位序 = `engine_core::fuzzy::FUZZY_PAIRS` 的数组序
/// （**对外契约**：bit0=zh/z, bit1=ch/c, bit2=sh/s, bit3=an/ang, bit4=en/eng,
/// bit5=in/ing, bit6=n/l, bit7=f/h；0xFF = 全开）。未装载 → 0（哨兵语义同族）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fuzzy_pairs() -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| i32::from(e.fuzzy_pairs())).unwrap_or(0)
    }))
    .unwrap_or(0)
}

/// 模糊音组名的**规范串**（读侧；解析写侧见 `opi_set_fuzzy_pairs_names`）。
/// 返回值随 `opi_fuzzy_pairs` 同族：未装载 → 空串。调用方（含设置界面）应把它当
/// **唯一名源**做多选列表，别手抄名表（加组必漂）。UTF-16、调用方 free。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fuzzy_pair_names() -> OpiString {
    let names = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|_| engine_core::fuzzy::pair_names()).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpiString::from_utf16(&names)
}

/// setFuzzyPairsNames(names)：按**组名串**（逗号分隔，见 `opi_fuzzy_pair_names`）
/// 设置启用集。解析在引擎侧只有一份（`engine_core::fuzzy::mask_from_names`）；
/// 认不得的名字整条忽略。未装载/NULL → 空操作。
///
/// # Safety
/// `ptr`/`len` 必须描述一段**有效、已初始化**的 UTF-16 内存（或 `ptr = null`
/// 且 `len = 0`）；函数只读该段，不接管其所有权。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_fuzzy_pairs_names(ptr: *const u16, len: usize) {
    let Some(names) = (unsafe { crate::cabi::utf16::read_utf16(ptr, len) }) else {
        return;
    };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.set_fuzzy_pairs(engine_core::fuzzy::mask_from_names(&names)))
    }));
}

/// setFuzzyPairs(mask)：模糊音逐对设置（位序同上；越界位钳掉，只留低 8 位）。
/// `mask = 0xFF` 等价旧 `opi_set_fuzzy(true)`、`0` 等价 `false` —— 旧布尔入口
/// 保留为包装（四端配置只写 `Fuzzy=` 的老配置走那条，行为一字不变）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_fuzzy_pairs(mask: i32) {
    let m = (mask as u32 & 0xFF) as u8;
    let _ = catch_unwind(AssertUnwindSafe(|| api::with_engine(|e| e.set_fuzzy_pairs(m))));
}

/// 且客户端本来就不知道该设成什么值）。只翻一个 bool，不改 buffer/候选/页码。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_chinese_punct(on: bool) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.set_chinese_punct(on))
    }));
}

/// toggleChinesePunct() -> bool：中文标点表开关的**触发键入口**，返回**切换后的新状态**
/// （状态栏/勾选框直接拿去刷新，不必再查一次）。与 `setChinesePunct` 是同一档的两个写入口，
/// 语义在引擎层（键位不在本层，客户端在引擎之前截获）。
///
/// ⚠️ 与 `opi_toggle_fullwidth` 的**重读规则不通用**：本档是用户偏好，`opi_switch_mode`
/// / `opi_toggle_symbol` **都不重置它**。只翻一个 bool，不改 buffer/候选/页码。
/// 未装载 → false：与「已装载且表关」共用同一个 false，**不是**在报错
/// （那时按键全走 action=0 交系统，返回值与可观测行为一致）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_toggle_chinese_punct() -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.toggle_chinese_punct()).unwrap_or(false)
    }))
    .unwrap_or(false)
}

