// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! JNI 出口的**符号 / 表情一族**（从 `jni.rs` 拆出：那个文件加 `emojiGroups` 后越
//! 500 行门禁；与 `jni_candidates.rs` / `jni_scheme.rs` / `jni_user_words.rs` 同一个
//! 拆分先例）。
//!
//! ⚠️ 拆出去**不等于脱离契约**：`tests/jni_contract.rs` 读的是本模块**全部** .rs 的
//! 合成文本（注册表仍在 `jni.rs`，函数体在这里 —— 两侧都在文本里，门禁才看得全）。
//! `guard` / `JEnv` 仍是 `jni.rs` 的（pub(crate)），语义与 C ABI 同源。

#![allow(clippy::missing_safety_doc)]
// ⚠️ 同 `jni.rs` 顶上那条：按模块生效，拆出来要在本文件里再写一次。

use jni::sys::{self, jshort, jstring};

use crate::api;
use crate::jni::{JEnv, guard};
use crate::jni_util;

/// searchSymbols(keyword: String) -> String[]。仅文本数组。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_search_symbols(
    env: JEnv,
    _class: sys::jclass,
    keyword: jstring,
) -> sys::jobjectArray {
    let texts = guard(|| {
        let kw = unsafe { jni_util::jstring_to_rust(env, keyword) }.unwrap_or_default();
        api::with_engine(|e| api::search_symbol_texts(e, &kw)).unwrap_or_default()
    });
    unsafe { jni_util::string_array(env, texts) }
}

/// emojiSymbols() -> String[]。表情面板浏览全量（官方分组序；见 `api::emoji_symbol_texts`）。
/// **不过引擎单例**：编译期静态表，引擎没装载也返回得了。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_emoji_symbols(
    env: JEnv,
    _class: sys::jclass,
) -> sys::jobjectArray {
    let texts = guard(api::emoji_symbol_texts);
    unsafe { jni_util::string_array(env, texts) }
}

/// symbolBlocks() -> String。JSON：`[{id,start,end,name,common}]`。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_symbol_blocks(env: JEnv, _class: sys::jclass) -> jstring {
    let json = guard(|| api::with_engine(|e| api::symbol_blocks_json(e)).unwrap_or_default());
    unsafe { jni_util::rust_to_jstring(env, &json) }
}

/// emojiGroups() -> String。JSON：`[{name,start,count}]`（表情面板分类条；2026-10-12）。
/// 未装载也照样有值（纯静态表，与 `Api` 无关）—— 不会像 symbolBlocks 那样返回空串。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_emoji_groups(env: JEnv, _class: sys::jclass) -> jstring {
    let json = guard(api::emoji_groups_json);
    unsafe { jni_util::rust_to_jstring(env, &json) }
}

/// symbolsInBlock(id: Short) -> String[]。仅文本数组。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_symbols_in_block(
    env: JEnv,
    _class: sys::jclass,
    id: jshort,
) -> sys::jobjectArray {
    let texts = guard(|| {
        // 负 id 同理按越界处理（空数组），不钳成块 0。jshort 转 u16 前必须先判负，
        // 否则 -1 会变成 65535（一个合法但错误的块号）。
        api::with_engine(|e| {
            if id < 0 {
                Vec::new()
            } else {
                api::symbol_texts(e, id as u16)
            }
        })
        .unwrap_or_default()
    });
    unsafe { jni_util::string_array(env, texts) }
}
