// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! opi-ffi：引擎双 ABI 出口。
//! - api：引擎薄壳 + 共享单例（JNI / C 共用）
//! - jni_util：UTF-16 字符串转换（禁用 modified UTF-8）
//! - jni：JNI_OnLoad + RegisterNatives（宿主类 xyz/erik/opi/jni/OpiEngine）
//! - cabi：C ABI（opi_* 函数，iOS M7 预留）
//!
//! ⚠️ **拆出去的两个模块必须在这里声明**（2026-10-08：两个 ABI 文件都因 500 行门禁
//! 拆过）。踩过一次：文件建好了、契约门禁（按**文本**读）照样绿，而 crate 根本没编
//! 它们 —— 导出面从 .so 里静默少掉一批，只有 `tests/c_abi/run.sh`（按 `nm` 读**产物**、
//! 且零豁免要求每条导出都被 consumer.c 调用）才会红。**两道门禁互补，别只看一道。**
pub mod api;
pub mod cabi;
pub mod cabi_user_words;
pub mod jni;
pub mod jni_user_words;
pub mod jni_util;
