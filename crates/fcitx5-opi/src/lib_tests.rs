// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 由 lib.rs 搬出（`#[path]` 引入）以保持本文件 <500 行，与 input_method_tests.rs 同惯例。

use super::*;

/// 读取 OpString 内容（null ptr → 空串）并释放。
fn read_and_free(s: OpString) -> String {
    let out = if s.ptr.is_null() {
        String::new()
    } else {
        let bytes = unsafe { std::slice::from_raw_parts(s.ptr, s.len) };
        String::from_utf8_lossy(bytes).into_owned()
    };
    unsafe { opi_ffi_free_string_utf8(s) };
    out
}

#[test]
fn mode_int_roundtrip() {
    assert_eq!(mode_from_int(0), Some(Mode::Pinyin));
    assert_eq!(mode_from_int(3), Some(Mode::Symbol));
    assert_eq!(mode_from_int(4), Some(Mode::Traditional));
    assert_eq!(mode_from_int(-1), None);
    assert_eq!(mode_to_int(Mode::English), 1);
    assert_eq!(mode_to_int(Mode::Number), 2);
    assert_eq!(mode_to_int(Mode::Traditional), 4);
}

#[test]
fn opstring_roundtrip() {
    let s = OpString::from_utf8("你好");
    let bytes = unsafe { std::slice::from_raw_parts(s.ptr, s.len) };
    assert_eq!(std::str::from_utf8(bytes).unwrap(), "你好");
    unsafe { opi_ffi_free_string_utf8(s) };
    let e = OpString::from_utf8("");
    assert!(e.ptr.is_null());
    assert_eq!(e.len, 0);
}

#[test]
fn install_singleton_fallback_and_path() {
    let _g = serial_install();
    assert_eq!(with_state(|s| s.buffer()), Some(String::new()));
    // 坏路径 → Err（load_or_fallback 原样语义，不回退）
    assert!(install(Some("/nonexistent/opi.dict")).is_err());
    // 单例保持上一次成功装载可用
    assert_eq!(with_state(|s| s.buffer()), Some(String::new()));
}

#[test]
fn input_key_invalid_utf8_is_lossy_and_safe() {
    let _g = serial_install();
    // 单字节无效 UTF-8 → lossy 替换为 U+FFFD，非 ASCII → 引擎忽略 → 空串
    let raw = [0xffu8];
    let out = unsafe { opi_fcitx5_input_key(raw.as_ptr(), raw.len()) };
    assert_eq!(read_and_free(out), "");
    // 多字节无效序列 → lossy 后为多字符 → 边界拒绝 → 空串
    let raw = [0xc3u8, 0x28u8];
    let out = unsafe { opi_fcitx5_input_key(raw.as_ptr(), raw.len()) };
    assert_eq!(read_and_free(out), "");
    // 不 panic、不返回垃圾
    assert_eq!(with_state(|s| s.buffer()), Some(String::new()));
}

#[test]
fn input_key_null_ptr_with_len_returns_empty() {
    let _g = serial_install();
    // read_utf8 先判 null（lib.rs read_utf8 首行），len>0 也不触碰内存
    let out = unsafe { opi_fcitx5_input_key(std::ptr::null(), 5) };
    assert_eq!(read_and_free(out), "");
}

#[test]
fn opstring_empty_roundtrip() {
    let e = OpString::from_utf8("");
    assert!(e.ptr.is_null());
    assert_eq!(e.len, 0);
    // 空句柄 free 为无操作且安全；再读也为空
    unsafe { opi_ffi_free_string_utf8(e) };
    assert_eq!(read_and_free(OpString::empty()), "");
}

#[test]
fn candidates_without_buffer_is_empty_json() {
    let _g = serial_install();
    let out = unsafe { opi_fcitx5_candidates(8) };
    assert_eq!(read_and_free(out), "[]");
}

#[test]
fn key_event_english_pass_through_commits_lowercase() {
    let _g = serial_install();
    // 切英文：直传 'a'（action=2 提交）
    unsafe { opi_fcitx5_switch_mode(1) };
    let r = unsafe { opi_fcitx5_key_event(97, 0) };
    assert_eq!(r.action, 2);
    assert_eq!(read_and_free(r.text), "a");
    // 切回拼音，英文直传不污染缓冲
    unsafe { opi_fcitx5_switch_mode(0) };
    assert_eq!(with_state(|s| s.buffer()), Some(String::new()));
}

#[test]
fn key_event_pinyin_letter_handled_and_space_commits() {
    let _g = serial_install();
    unsafe { opi_fcitx5_switch_mode(0) };
    let r = unsafe { opi_fcitx5_key_event(104, 0) }; // 'h'
    assert_eq!(r.action, 1); // EngineHandled
    assert_eq!(with_state(|s| s.buffer()), Some("h".to_string()));
    // 缓冲非空空格 → 提交并清空（提交文本取决于词库，只断言非空）
    let r = unsafe { opi_fcitx5_key_event(32, 0) };
    assert_eq!(r.action, 2);
    assert!(!read_and_free(r.text).is_empty());
    let r = unsafe { opi_fcitx5_key_event(97, 0) }; // 'a'
    assert_eq!(r.action, 1);
    let r = unsafe { opi_fcitx5_key_event(32, 0) }; // buffer 非空 → 空格提交
    assert_eq!(r.action, 2);
    assert!(!read_and_free(r.text).is_empty());
}

#[test]
fn key_event_ctrl_passes_through_and_shift_consumed() {
    let _g = serial_install();
    // Ctrl+C → 直通
    let r = unsafe { opi_fcitx5_key_event(99, 1 << 2) };
    assert_eq!(r.action, 0);
    // shift 按下（无修饰）→ EngineHandled
    let r = unsafe { opi_fcitx5_key_event(0xffe1, 0) };
    assert_eq!(r.action, 1);
    // 英文空缓冲 + single shift → 大写提交且消费
    unsafe { opi_fcitx5_switch_mode(1) };
    let r = unsafe { opi_fcitx5_key_event(97, 0) };
    assert_eq!(r.action, 2);
    assert_eq!(read_and_free(r.text), "A");
    let r = unsafe { opi_fcitx5_key_event(97, 0) };
    assert_eq!(read_and_free(r.text), "a");
}
