// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `opi_fcitx5_search_symbols` 的门禁（2026-10-13）：验的是**投影**——
//! 「引擎搜出什么，出口就吐什么」。
//!
//! ⚠️ 断言用的参照物是**引擎自己**（`SymbolEngine::builtin().search()`），不是
//! 写死的数据行：本仓有过一次「词库整合把真词库测试的前提静默翻面」的事故
//! （memory: dict-integration-flips-real-dict-test-premises），写死 `"dun" → 、`
//! 会让一次正当的数据更新变成一道红。关键字也**从数据里现取**（第一个常用类目的
//! 首条的首个关键字），保证「非空」不是靠巧合。
//!
//! 这层测到的是：JSON 形状（serde_json 的 `["…"]`）、`text` 字段（不是 name/
//! keywords）、**顺序与去重**（逐条相等）、空关键字 = 全部、nullptr = 空串。
//! 测不到的：C++ 侧的 D-Bus 编排（那边有自己的 driver 与 opi_ctl_cli.rs）。

use engine_core::symbols::SymbolEngine;
use serde_json::Value;

use crate::opi_fcitx5_search_symbols;
use crate::serial_install;

/// 取一个**一定存在**的关键字：第一个常用类目的首条的首个关键字。
/// 数据换了关键字也跟着换，不会假红也不会空转（调用方断言结果非空）。
fn live_keyword(engine: &SymbolEngine) -> String {
    let block = engine
        .common_blocks()
        .into_iter()
        .next()
        .expect("内置符号表应有常用类目");
    let entry = engine
        .entries_in_block(block.id)
        .into_iter()
        .next()
        .expect("常用类目应非空");
    entry
        .keywords
        .first()
        .expect("符号条目应带关键字（生成器保证）")
        .clone()
}

/// 出口的 JSON → Vec<String>。serde_json 是 lib.rs 的正式依赖（texts_to_json 用
/// 它），测试里直接用它当解析器 —— 手工切串会把「转义/空数组」这类形状问题蒙掉。
fn json_texts(raw: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(raw).expect("出口应是 serde_json 的字符串数组")
}

fn call(kw: &str) -> String {
    let out = unsafe { opi_fcitx5_search_symbols(kw.as_ptr(), kw.len()) };
    // 与调用方（C++ 的 take()）同一条释放契约：读一次、释放一次。
    let s = if out.ptr.is_null() {
        String::new()
    } else {
        let v = unsafe { std::slice::from_raw_parts(out.ptr, out.len) };
        String::from_utf8(v.to_vec()).expect("出口应是 UTF-8")
    };
    unsafe { crate::opi_ffi_free_string_utf8(out) };
    s
}

#[test]
fn search_projects_engine_hits_verbatim() {
    let _g = serial_install();
    let engine = SymbolEngine::builtin();
    let kw = live_keyword(&engine);
    let expected: Vec<String> = engine.search(&kw).into_iter().map(|e| e.text).collect();
    assert!(!expected.is_empty(), "现取的关键字 {kw:?} 应至少命中一条");
    assert_eq!(json_texts(&call(&kw)), expected);
}

#[test]
fn search_result_is_a_json_string_array() {
    let _g = serial_install();
    let raw = call("dun");
    // 形状：serde_json 能解析成字符串数组（空数组也是合法形状 —— 这一条只钉形状）。
    let parsed = serde_json::from_str::<Value>(&raw).expect("应是合法 JSON");
    assert!(parsed.is_array(), "应是数组，实得 {raw:?}");
    assert!(
        parsed.as_array().unwrap().iter().all(|v| v.is_string()),
        "元素应全是字符串：{raw:?}"
    );
}

#[test]
fn no_hit_keyword_is_empty_array_not_error() {
    let _g = serial_install();
    // 无命中 = 空数组（与「引擎未装载」同一个哨兵）—— 调用方据此判「没搜到」，
    // 不是错误（错误在协议层另有一套 err: 表达）。
    assert_eq!(call("zzzznosuchkeywordzzz"), "[]");
}

#[test]
fn empty_keyword_projects_the_whole_table() {
    let _g = serial_install();
    // 空关键字 = 全部条目（引擎既有语义，同 Android searchSymbols('')）：
    // 这一层**不**把它当错误 —— 「空 = 用法错误」是协议层（C++/opi-ctl）的裁决。
    let all: Vec<String> = SymbolEngine::builtin()
        .search("")
        .into_iter()
        .map(|e| e.text)
        .collect();
    assert!(
        all.len() > 100,
        "内置符号表应远不止百条，实得 {}",
        all.len()
    );
    assert_eq!(json_texts(&call("")), all);
}

#[test]
fn null_ptr_is_treated_as_empty_keyword() {
    let _g = serial_install();
    // read_utf8 的 null 分支（C 侧惯用法：空串就给 null/0）—— 不是 panic，
    // 也与空串同义。
    let out = unsafe { opi_fcitx5_search_symbols(std::ptr::null(), 0) };
    let raw = if out.ptr.is_null() {
        String::new()
    } else {
        let v = unsafe { std::slice::from_raw_parts(out.ptr, out.len) };
        String::from_utf8(v.to_vec()).unwrap()
    };
    unsafe { crate::opi_ffi_free_string_utf8(out) };
    assert_eq!(raw, call(""));
}
