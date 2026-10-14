// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! JNI 出口的**词库导入**一组（M3，2026-10-14；与 C ABI 侧的 `cabi_dict.rs` 同主题、
//! 同一次落地）。语义（含三个哨兵与「停用后数据仍留在内存」的裁决）见 `api/dict.rs`
//! 头注 —— 本文件只做 JNI 类型桥，不重复那份契约。
//!
//! ⚠️ 拆出去**不等于脱离契约**：注册表在 `jni.rs`（`JNI_OnLoad`），函数体在这里；
//! `tests/jni/checker.rs` 的 `JNI_ALL` 必须含本文件 —— 漏了这批出口会从门禁视野里
//! 消失（注册表与宿主声明都比对不到它们）。

#![allow(clippy::missing_safety_doc)]
// ⚠️ 同 `jni.rs` 顶上那条：按模块生效，拆出来要在本文件里再写一次。

use std::panic::{AssertUnwindSafe, catch_unwind};

use jni::sys::{self, jboolean, jint, jstring};

use crate::api;
use crate::jni::{JEnv, guard};
use crate::jni_util;

/// importDict(path: String) -> Int。读一个 UTF-8 的 tsv 词库文件（三列
/// `词<TAB>拼音<TAB>频次`，**词在前** —— 列序真源是 `engine-data` 的解析器
/// `dict_text.rs`，M3 任务书里的「拼音在前」是反的）当**一个导入层**；返回条目数。
/// 失败（读不到 / 格式全坏 / 没有有效条目 / null 入参 / 引擎未装载）→ **-1**，
/// 且不改动既有导入层。名字 = 文件名词干；**同名重导入 = 覆盖**并置为启用。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_import_dict(
    env: JEnv,
    _class: sys::jclass,
    path: jstring,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| -> jint {
        let Some(path) = (unsafe { jni_util::jstring_to_rust(env, path) }) else {
            return -1;
        };
        match api::with_engine(|e| e.import_dict(&path)) {
            Some(Ok(n)) => i32::try_from(n).unwrap_or(-1),
            Some(Err(_)) | None => -1,
        }
    }))
    .unwrap_or(-1)
}

/// dictList() -> String：JSON 数组，系统词典在前（luna/trad/stroke，`system: true`），
/// 导入层随后（导入顺序）；每项 `{name, count, enabled, system}`。
/// **未装载 → `[]`**（数组哨兵，见 `api::DICT_LIST_UNLOADED`）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_dict_list(env: JEnv, _class: sys::jclass) -> jstring {
    let out = guard(|| {
        api::with_engine(|e| e.dict_list()).unwrap_or_else(|| api::DICT_LIST_UNLOADED.to_string())
    });
    unsafe { jni_util::rust_to_jstring(env, &out) }
}

/// setDictEnabled(name: String, on: Boolean) -> Boolean。装载 / 卸载一条**导入**词库
/// （原子：一次全量替换）。已是目标态 → true（幂等）；名字认不得（含系统词典
/// luna/trad/stroke）或引擎未装载 → **false**。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_set_dict_enabled(
    env: JEnv,
    _class: sys::jclass,
    name: jstring,
    on: jboolean,
) -> jboolean {
    guard(|| {
        let name = unsafe { jni_util::jstring_to_rust(env, name) }.unwrap_or_default();
        api::with_engine(|e| e.set_dict_enabled(&name, on)).unwrap_or(false)
    })
}

/// removeDict(name: String) -> Boolean。把一条**导入**词库从内存里删掉；**文件是宿主
/// 的事**（本层不碰磁盘）。名字认不得 / 已经不在 / 引擎未装载 → **false**。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_remove_dict(
    env: JEnv,
    _class: sys::jclass,
    name: jstring,
) -> jboolean {
    guard(|| {
        let name = unsafe { jni_util::jstring_to_rust(env, name) }.unwrap_or_default();
        api::with_engine(|e| e.remove_dict(&name)).unwrap_or(false)
    })
}
