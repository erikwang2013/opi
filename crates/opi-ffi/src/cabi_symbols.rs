// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! C ABI 出口的**符号 / 表情一族**（从 `cabi.rs` 拆出：加表情两出口后到 499/500 ——
//! 一行余量，之后任何微改都会越线。与 `cabi_user_words.rs` / `cabi_scheme.rs` 同一个
//! 拆分先例 —— 它同时是 `jni_symbols.rs` 的镜像：JNI 面 2026-10-12 因 `emojiGroups`
//! 拆的是同一个族，两轨的族边界保持一致）。
//!
//! ⚠️ 拆出去**不等于脱离契约**：`tests/c_abi_contract.rs` 的门禁读本模块**全部** .rs
//! （`checker::read_cabi()`，本次已把本文件加进那份合成文本）—— 漏加就是这一批
//! **静默**不再被比对。
//!
//! 语义与 JNI 侧（`jni_symbols.rs`）逐条一致，共享 `api::` 的同一份实现。
//! ⚠️ 表情两出口（2026-10-13 加：iOS / 鸿蒙的表情页数据面 —— 此前只在 JNI 面有）
//! **不经引擎单例**：编译期静态表，未装载也返回全量。它们是本族（乃至全 ABI）
//! **仅有的**两个「未装载不退化哨兵」的查询出口，别照抄它们的姿态去改别的出口。

#![allow(clippy::missing_safety_doc)]
// ⚠️ 同 `cabi.rs` 顶上那条：按模块生效，拆出来要在本文件里再写一次。

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::api;
use crate::cabi::OpiString;
use crate::cabi::utf16::{read_utf16, texts_to_json};

/// searchSymbols(keyword) -> JSON 文本数组。
/// # Safety
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

/// emojiSymbols() -> JSON 文本数组。表情面板**浏览表**（OpenMoji 常用子集、面板分组序，
/// 不含旗帜/肤色变体/ZWJ 组合；真源 `api::emoji_symbol_texts`，与 JNI 面
/// `opijni_emoji_symbols` 同源）。
/// ⚠️ **不经引擎单例**：编译期静态表，引擎未装载（`opi_load` 返回 false）时上面几条都
/// 退化成 `[]`/空串，本出口**照样返回全表** —— 冷启动第一帧就能画（这条由
/// `tests/cabi_unloaded.rs` 的 `emoji_groups_index_the_full_emoji_list` 钉住）。
/// `opi_emoji_groups()` 的 `start`/`count` 即本数组下标。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_emoji_symbols() -> OpiString {
    let texts = catch_unwind(AssertUnwindSafe(api::emoji_symbol_texts)).unwrap_or_default();
    texts_to_json(texts)
}

/// symbolBlocks() -> JSON：`[{id,start,end,name,common}]`。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_symbol_blocks() -> OpiString {
    let json = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| api::symbol_blocks_json(e)).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpiString::from_utf16(&json)
}

/// emojiGroups() -> JSON：`[{name,start,count}]`（serde **字典序**，未开 preserve_order，
/// 消费方别按书写顺序写正则；表情面板分类条）。`start` 与 `opi_emoji_symbols()` 的
/// 下标对齐：点分类 = 网格滚到 `start`。同上**不经引擎单例**，未装载也有值
/// （与 `opi_symbol_blocks` 未装载返回空串相反）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_emoji_groups() -> OpiString {
    let json = catch_unwind(AssertUnwindSafe(api::emoji_groups_json)).unwrap_or_default();
    OpiString::from_utf16(&json)
}

/// symbolsInBlock(id: i16) -> JSON 文本数组。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_symbols_in_block(id: i16) -> OpiString {
    let texts = catch_unwind(AssertUnwindSafe(|| {
        // 负 id 按越界处理（空数组），不钳成块 0 —— 与 jni.rs 的
        // opijni_symbols_in_block 同语义，两个 ABI 面必须一致。
        api::with_engine(|e| {
            if id < 0 {
                Vec::new()
            } else {
                api::symbol_texts(e, id as u16)
            }
        })
        .unwrap_or_default()
    }))
    .unwrap_or_default();
    texts_to_json(texts)
}
