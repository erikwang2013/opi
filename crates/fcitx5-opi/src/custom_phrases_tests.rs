// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 英文套宿主文件的读取/推送门禁（`custom_phrases.rs`）。
//! 与中文套同 schema、同目录，但**独立文件、独立引擎槽位**。

use super::*;
use std::fs;
use std::path::PathBuf;

fn temp_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "opi-en-phrases-{tag}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("建临时目录");
    dir
}

#[test]
fn english_path_is_a_sibling_of_the_chinese_file() {
    let en = english_custom_phrases_path();
    let cn = custom_phrases_path();
    assert_eq!(en.parent(), cn.parent(), "两套必须在同一数据目录");
    assert_ne!(en, cn, "必须是两个文件");
    assert_eq!(
        en.file_name().and_then(|s| s.to_str()),
        Some(ENGLISH_CUSTOM_PHRASES_FILE_NAME)
    );
}

#[test]
fn missing_or_broken_english_file_defaults() {
    let dir = temp_dir("default");
    assert_eq!(
        load_english_custom_phrases_from(&dir),
        PhraseState::default()
    );
    fs::write(dir.join(ENGLISH_CUSTOM_PHRASES_FILE_NAME), "{").unwrap();
    assert_eq!(
        load_english_custom_phrases_from(&dir),
        PhraseState::default()
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn english_file_is_read_independently_from_chinese() {
    let dir = temp_dir("independent");
    fs::write(
        dir.join(CUSTOM_PHRASES_FILE_NAME),
        r#"{"version":1,"enabled":true,"phrases":[{"trigger":"dz","text":"地址"}]}"#,
    )
    .unwrap();
    fs::write(
        dir.join(ENGLISH_CUSTOM_PHRASES_FILE_NAME),
        r#"{"version":1,"enabled":false,"phrases":[{"trigger":"hello","text":"english phrase"}]}"#,
    )
    .unwrap();

    let cn = load_custom_phrases_file(&dir.join(CUSTOM_PHRASES_FILE_NAME));
    let en = load_english_custom_phrases_from(&dir);
    assert!(cn.enabled);
    assert!(cn.json.contains("地址"));
    assert!(!cn.json.contains("english phrase"));
    assert!(!en.enabled);
    assert!(en.json.contains("english phrase"));
    assert!(!en.json.contains("地址"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn apply_english_pushes_only_the_english_slot() {
    let _g = crate::serial_install();
    let cn = parse_custom_phrases(
        r#"{"version":1,"enabled":false,"phrases":[{"trigger":"dz","text":"地址"}]}"#,
    )
    .unwrap();
    let en = parse_custom_phrases(
        r#"{"version":1,"enabled":true,"phrases":[{"trigger":"hello","text":"english phrase"}]}"#,
    )
    .unwrap();
    crate::with_state(|s| s.apply_custom_phrases(cn)).unwrap();
    crate::with_state(|s| s.apply_english_custom_phrases(en)).unwrap();

    let (cn_json, en_json, cn_on, en_on) = crate::with_state(|s| {
        (
            s.engine.export_custom_phrases(),
            s.engine.export_english_custom_phrases(),
            s.engine.custom_phrases_enabled(),
            s.engine.english_custom_phrases_enabled(),
        )
    })
    .unwrap();
    assert!(!cn_on, "中文启用态不受英文影响");
    assert!(en_on);
    assert!(cn_json.contains("地址"));
    assert!(!cn_json.contains("english phrase"));
    assert!(en_json.contains("english phrase"));
    assert!(!en_json.contains("地址"));
}

#[test]
fn invalid_english_table_falls_back_without_touching_chinese() {
    let _g = crate::serial_install();
    let cn = parse_custom_phrases(r#"{"version":1,"phrases":[{"trigger":"dz","text":"地址"}]}"#)
        .unwrap();
    crate::with_state(|s| s.apply_custom_phrases(cn)).unwrap();
    let bad = parse_custom_phrases(
        r#"{"version":1,"enabled":false,"phrases":[{"trigger":"1","text":"x"}]}"#,
    )
    .expect("外层形状合法");
    crate::with_state(|s| s.apply_english_custom_phrases(bad)).unwrap();

    let (cn_json, en_json, en_on) = crate::with_state(|s| {
        (
            s.engine.export_custom_phrases(),
            s.engine.export_english_custom_phrases(),
            s.engine.english_custom_phrases_enabled(),
        )
    })
    .unwrap();
    assert!(cn_json.contains("地址"), "英文坏表不许清中文");
    assert!(!en_json.contains("\"x\""));
    assert!(!en_on, "坏表退回空表 + 关");
}

/// 2026-10-25 用户报障「桌面端无法用配置的字母匹配」：从**宿主文件**读入 →
/// 推送引擎 → 真按键路径出候选。这条覆盖 fcitx5 的整条桌面读侧（不是只测引擎）。
#[test]
fn enabled_phrase_file_matches_typed_letters() {
    let dir = temp_dir("match");
    let path = dir.join(CUSTOM_PHRASES_FILE_NAME);
    fs::write(
        &path,
        r#"{"version":1,"enabled":true,"phrases":[{"trigger":"dz","text":"地址"}]}"#,
    )
    .unwrap();
    let mut s = crate::candidate::CandidateState {
        engine: engine_core::Engine::new(
            Box::new(engine_core::dictionary::InMemoryDictionary::new()),
            engine_core::symbols::SymbolEngine::builtin(),
            true,
        ),
        page: 0,
        buffer_snapshot: String::new(),
        shift_state: crate::candidate::ShiftState::Off,
        last_printable: None,
        dicts: Default::default(),
        english_pro: false,
        fetch_cache: Default::default(),
        persist: Default::default(),
    };
    s.apply_custom_phrases(load_custom_phrases_file(&path));
    s.switch_mode(engine_core::composer::Mode::Pinyin);
    for c in "dz".chars() {
        crate::input_method::handle_key(&mut s, c as u32, 0);
    }
    assert_eq!(s.candidates()[0], "地址", "配置的字母必须命中短语");
    let _ = fs::remove_dir_all(&dir);
}

/// 2026-10-24：英文短语可命中时，**普通英文模式**（pro=false）也必须缓冲 ——
/// 直出模式下字母已进文档，trigger 永远匹配不到。表空/开关关保持直出。
#[test]
fn english_phrases_make_normal_english_mode_buffer() {
    let mut s = crate::candidate::CandidateState {
        engine: engine_core::Engine::new(
            Box::new(engine_core::dictionary::InMemoryDictionary::new()),
            engine_core::symbols::SymbolEngine::builtin(),
            true,
        ),
        page: 0,
        buffer_snapshot: String::new(),
        shift_state: crate::candidate::ShiftState::Off,
        last_printable: None,
        dicts: Default::default(),
        english_pro: false,
        fetch_cache: Default::default(),
        persist: Default::default(),
    };
    s.apply_english_custom_phrases(PhraseState {
        enabled: true,
        json: r#"{"version":1,"phrases":[{"trigger":"hello","text":"english phrase"}]}"#
            .to_string(),
    });
    s.switch_mode(engine_core::composer::Mode::English);
    for c in "hello".chars() {
        crate::input_method::handle_key(&mut s, c as u32, 0);
    }
    assert_eq!(s.buffer(), "hello", "短语可命中时普通英文必须缓冲");
    assert_eq!(s.candidates()[0], "english phrase");

    s.engine.set_english_custom_phrases_enabled(false);
    s.clear();
    for c in "hello".chars() {
        crate::input_method::handle_key(&mut s, c as u32, 0);
    }
    assert_eq!(s.buffer(), "", "短语关闭后普通英文仍是直出");
}
