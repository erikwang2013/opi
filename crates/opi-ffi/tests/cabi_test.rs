//! C ABI 集成测试：host 上直接调 opi_* 函数，验证装载/候选/选择/模式/符号主链路。
//! 与 JNI 出口共享同一单例与实现；测试间用 SERIAL 互斥避免单例串扰。

use std::sync::Mutex;

use opi_ffi::cabi::{
    opi_backspace, opi_buffer, opi_candidates, opi_clear, opi_clear_user_words, opi_export_user_words,
    opi_ffi_free_string, opi_input_key, opi_input_space, opi_learner_enabled, opi_load, opi_load_trad, opi_mode,
    opi_select, opi_search_symbols, opi_set_learner, opi_set_shift, opi_switch_mode, opi_symbol_blocks,
    opi_symbols_in_block, OpiString,
};

static SERIAL: Mutex<()> = Mutex::new(());

fn to_units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn read(s: OpiString) -> String {
    if s.ptr.is_null() {
        return String::new();
    }
    let units = unsafe { std::slice::from_raw_parts(s.ptr, s.len) };
    let out = String::from_utf16(units).unwrap_or_default();
    unsafe { opi_ffi_free_string(s) };
    out
}

fn read_texts(s: OpiString) -> Vec<String> {
    serde_json::from_str(&read(s)).unwrap_or_default()
}

/// 装载：优先 luna 词库（存在则真实加载），缺失走内置回退（也必须成功）。
fn load_any() {
    let p = to_units("../../android/app/src/main/assets/luna.opid");
    let ok = unsafe { opi_load(p.as_ptr(), p.len()) };
    if !ok {
        assert!(unsafe { opi_load(std::ptr::null(), 0) }, "内置回退路径必须可用");
    }
}

#[test]
fn cabi_load_null_fallback_ok() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    assert!(unsafe { opi_load(std::ptr::null(), 0) });
}

#[test]
fn cabi_pinyin_candidates_and_select() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    assert_eq!(unsafe { opi_mode() }, 0, "初始应为 Pinyin");
    let w = to_units("w");
    let o = to_units("o");
    unsafe {
        opi_input_key(w.as_ptr(), w.len());
        opi_input_key(o.as_ptr(), o.len());
    }
    assert_eq!(read(unsafe { opi_buffer() }), "wo");
    let texts = read_texts(unsafe { opi_candidates(8) });
    assert!(!texts.is_empty(), "候选非空");
    assert!(texts.contains(&"我".to_string()), "候选应含 我（luna 排名以实际词库为准）");
    assert_eq!(read(unsafe { opi_select(0) }), texts[0], "select(0) 应返回首个候选");
    assert_eq!(read(unsafe { opi_buffer() }), "");
    // select 越界 → 空串
    assert_eq!(read(unsafe { opi_select(999) }), "");
    unsafe { opi_clear() };
    assert_eq!(read(unsafe { opi_buffer() }), "");
}

#[test]
fn cabi_input_key_boundary() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    let empty = to_units("");
    let multi = to_units("ab");
    let hanzi = to_units("你");
    unsafe {
        assert_eq!(read(opi_input_key(empty.as_ptr(), empty.len())), "");
        assert_eq!(read(opi_input_key(multi.as_ptr(), multi.len())), "");
        assert_eq!(read(opi_input_key(hanzi.as_ptr(), hanzi.len())), "");
    }
    assert_eq!(read(unsafe { opi_buffer() }), "");
    unsafe { opi_clear() };
}

#[test]
fn cabi_mode_shift_space_backspace() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(1) };
    assert_eq!(unsafe { opi_mode() }, 1);
    let abc = to_units("abc");
    unsafe {
        for c in ["a", "b", "c"] {
            let u = to_units(c);
            opi_input_key(u.as_ptr(), u.len());
        }
        assert_eq!(read(opi_buffer()), "abc");
        assert_eq!(read(opi_input_space()), "abc");
        assert_eq!(read(opi_buffer()), "");
        opi_set_shift(true);
        let a = to_units("a");
        opi_input_key(a.as_ptr(), a.len());
        assert_eq!(read(opi_buffer()), "A");
        opi_set_shift(false);
        opi_backspace();
        assert_eq!(read(opi_buffer()), "");
        // 越界模式忽略，状态不变
        opi_switch_mode(9);
    }
    assert_eq!(unsafe { opi_mode() }, 1);
    unsafe { opi_switch_mode(0) };
    let _ = abc;
}

#[test]
fn cabi_learner_and_user_words() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    // 默认开（M1 语义）
    assert!(unsafe { opi_learner_enabled() });
    unsafe { opi_set_learner(false) };
    assert!(!unsafe { opi_learner_enabled() });
    unsafe { opi_set_learner(true) };
    assert!(unsafe { opi_learner_enabled() });
    let words = read(unsafe { opi_export_user_words() });
    assert!(!words.is_empty());
    assert!(words.contains("\"version\""));
    unsafe { opi_clear_user_words() };
    assert_eq!(read(unsafe { opi_export_user_words() }), r#"{"version":1,"words":[]}"#);
}

#[test]
fn cabi_symbols_blocks_and_search() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    let blocks = read(unsafe { opi_symbol_blocks() });
    let v: Vec<serde_json::Value> = serde_json::from_str(&blocks).unwrap_or_default();
    assert!(!v.is_empty(), "symbolBlocks 非空");
    assert!(v[0].get("id").is_some(), "JSON 含 id 字段");
    let id = v[0]["id"].as_u64().expect("id 为数字") as u16;
    let syms = read_texts(unsafe { opi_symbols_in_block(id as i16) });
    assert!(!syms.is_empty(), "块内符号非空");
    let he = to_units("he");
    let hits = read_texts(unsafe { opi_search_symbols(he.as_ptr(), he.len()) });
    assert!(hits.iter().any(|s| s == "♥"), "搜索 he 应命中 ♥");
}

#[test]
fn cabi_load_trad_routes_traditional_mode() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    // 坏路径 → false（不影响已装主词典）
    let bad = to_units("/nonexistent/trad.opid");
    assert!(!unsafe { opi_load_trad(bad.as_ptr(), bad.len()) });
    // 真路径：仓库内 trad.opid（Task 1 已提交）
    let p = to_units("../../data/generated/trad.opid");
    assert!(unsafe { opi_load_trad(p.as_ptr(), p.len()) });
    unsafe { opi_switch_mode(4) };
    assert_eq!(unsafe { opi_mode() }, 4);
    let f = to_units("f");
    let a = to_units("a");
    unsafe {
        opi_input_key(f.as_ptr(), f.len());
        opi_input_key(a.as_ptr(), a.len());
    }
    let texts = read_texts(unsafe { opi_candidates(8) });
    assert!(texts.contains(&"發".to_string()), "繁模式 fa 候选应含 發，实际: {texts:?}");
    unsafe {
        opi_switch_mode(0);
        opi_clear();
    }
}
