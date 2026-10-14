// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! C ABI 出口的**词库导入**一组（M3，2026-10-14；与 JNI 侧的 `jni_dict.rs` 同主题、
//! 同一次落地）。语义（含三个哨兵与「停用后数据仍留在内存」的裁决）见 `api/dict.rs`
//! 头注 —— 本文件只做 UTF-16 类型桥，不重复那份契约。
//!
//! ⚠️ 拆出去**不等于脱离契约**：`tests/c_abi/checker.rs` 的 `read_cabi()` 必须含本文件
//! （集合相等的比对只读它列的那些）——漏了这批出口会从门禁视野里**静默**消失。

#![allow(clippy::missing_safety_doc)]
// ⚠️ 上面那条 `allow` 与 `cabi.rs` 顶上那条是同一件事、同样的范围（按模块生效）：
// 拆出来之后**必须在本文件里再写一次**（同 `cabi_user_words.rs` 的说明）。

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::api::{self, DICT_LIST_UNLOADED};
use crate::cabi::OpiString;
use crate::cabi::utf16::read_utf16;

/// import_dict(path: const uint16_t*, len) -> i32。读一个 UTF-8 的 tsv 词库文件
/// （三列 `词<TAB>拼音<TAB>频次`，**词在前** —— 列序真源是 `engine-data` 的解析器
/// `dict_text.rs`，M3 任务书里的「拼音在前」是反的）当**一个导入层**；返回条目数。
/// 失败（读不到 / 格式全坏 / 没有有效条目 / null 入参 / 引擎未装载）→ **-1**，
/// 且不改动既有导入层。名字 = 文件名词干；**同名重导入 = 覆盖**并置为启用。
/// # Safety
/// `ptr` 必须指向至少 `len` 个有效 `u16`（或为 null，视为失败）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_import_dict(ptr: *const u16, len: usize) -> i32 {
    catch_unwind(AssertUnwindSafe(|| -> i32 {
        let Some(path) = (unsafe { read_utf16(ptr, len) }) else {
            return -1;
        };
        match api::with_engine(|e| e.import_dict(&path)) {
            Some(Ok(n)) => i32::try_from(n).unwrap_or(-1),
            Some(Err(_)) | None => -1,
        }
    }))
    .unwrap_or(-1)
}

/// dict_list() -> OpiString（JSON 数组）：系统词典在前（luna/trad/stroke，
/// `system: true`），导入层随后（导入顺序）；每项 `{name, count, enabled, system}`。
/// **未装载 → `[]`**（数组哨兵 = [`DICT_LIST_UNLOADED`]，不是空句柄 —— 宿主 parse
/// 就能拿到空列表）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_dict_list() -> OpiString {
    // ⚠️ 兜底不能用 `unwrap_or_default()`：`String::default()` 是空串，会把哨兵
    // 从 `[]` 悄悄换成空串（panic 路径上也要是 `[]`）。
    let out = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.dict_list()).unwrap_or_else(|| DICT_LIST_UNLOADED.to_string())
    }))
    .unwrap_or_else(|_| DICT_LIST_UNLOADED.to_string());
    OpiString::from_utf16(&out)
}

/// set_dict_enabled(name: const uint16_t*, len, on) -> bool。装载 / 卸载一条**导入**
/// 词库（原子：一次全量替换）。已是目标态 → true（幂等）；名字认不得（含系统词典
/// luna/trad/stroke）/ null 入参 / 引擎未装载 → **false**。
/// # Safety
/// `ptr` 必须指向至少 `len` 个有效 `u16`（或为 null，视为名字认不得 → false）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_dict_enabled(ptr: *const u16, len: usize, on: bool) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        let name = unsafe { read_utf16(ptr, len) }.unwrap_or_default();
        api::with_engine(|e| e.set_dict_enabled(&name, on)).unwrap_or(false)
    }))
    .unwrap_or(false)
}

/// remove_dict(name: const uint16_t*, len) -> bool。把一条**导入**词库从内存里删掉；
/// **文件是宿主的事**（本层不碰磁盘）。名字认不得 / 已经不在 / null 入参 /
/// 引擎未装载 → **false**。
/// # Safety
/// `ptr` 必须指向至少 `len` 个有效 `u16`（或为 null，视为名字认不得 → false）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_remove_dict(ptr: *const u16, len: usize) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        let name = unsafe { read_utf16(ptr, len) }.unwrap_or_default();
        api::with_engine(|e| e.remove_dict(&name)).unwrap_or(false)
    }))
    .unwrap_or(false)
}
