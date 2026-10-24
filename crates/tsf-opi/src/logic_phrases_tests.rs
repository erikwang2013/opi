// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 自定义短语宿主文件的读取/推送门禁（`custom_phrases.rs`）。
//! 路径与解析是平台中立的（本机 Linux 全跑）；引擎侧只看 export/enabled 与候选。

use super::*;
use crate::custom_phrases;
use engine_core::composer::Mode;
use engine_core::dictionary::InMemoryDictionary;

fn logic() -> TsfLogic {
    let mut s = TsfLogic::with_dict(Box::new(InMemoryDictionary::new()));
    s.switch_mode(Mode::Pinyin);
    s
}

fn temp_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "opi-phrases-{tag}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建临时目录");
    dir
}

// ---- 解析 ----

#[test]
fn parse_reads_enabled_and_strips_it_for_the_engine() {
    let state =
        parse(r#"{"version":1,"enabled":false,"phrases":[{"trigger":"dz","text":"地址"}]}"#)
            .expect("合法文件");
    assert!(!state.enabled);
    assert!(!state.json.contains("enabled"), "{}", state.json);
    assert!(state.json.contains(r#""trigger":"dz""#), "{}", state.json);

    let no_enabled = parse(r#"{"version":1,"phrases":[]}"#).expect("enabled 可缺省");
    assert!(!no_enabled.enabled, "缺省 = 关（2026-10-25 裁决）");
}

#[test]
fn parse_rejects_every_broken_shape() {
    for text in [
        "",
        "not json",
        "[]",
        r#"{"phrases":[]}"#,                               // 缺 version
        r#"{"version":2,"phrases":[]}"#,                   // 版本不支持
        r#"{"version":1,"phrases":{}}"#,                   // phrases 不是数组
        r#"{"version":1,"enabled":"false","phrases":[]}"#, // enabled 类型错
    ] {
        assert!(parse(text).is_none(), "应拒收: {text}");
    }
}

#[test]
fn missing_or_broken_file_falls_back_to_empty_defaults() {
    let dir = temp_dir("missing");
    let missing = load_file(&dir.join("nope.json"));
    assert_eq!(missing, PhraseState::default());
    assert!(!missing.enabled);

    let path = dir.join(FILE_NAME);
    std::fs::write(&path, "{").unwrap();
    assert_eq!(load_file(&path), PhraseState::default());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn path_is_beside_the_dicts_directory() {
    let dicts = Path::new("/home/u/.local/share/opi/dicts");
    assert_eq!(
        path_from_dicts_dir(dicts),
        Path::new("/home/u/.local/share/opi/custom_phrases.json")
    );
    // 退化路径（空 dicts_dir）不 panic，落在当前目录。
    assert_eq!(path_from_dicts_dir(Path::new("")), Path::new(FILE_NAME));

    // 英文套：同一父目录、独立文件。
    assert_eq!(
        english_path_from_dicts_dir(dicts),
        Path::new("/home/u/.local/share/opi/custom_phrases_en.json")
    );
    assert_eq!(
        english_path_from_dicts_dir(Path::new("")),
        Path::new(EN_FILE_NAME)
    );
}

// ---- 推给引擎 ----

#[test]
fn apply_pushes_phrases_and_enabled_into_the_engine() {
    let mut s = logic();
    let state = parse(r#"{"version":1,"enabled":true,"phrases":[{"trigger":"dz","text":"地址"}]}"#)
        .unwrap();
    s.apply_custom_phrases(state);

    assert!(s.engine.custom_phrases_enabled(), "启用态必须分开推");
    assert!(s.engine.export_custom_phrases().contains("地址"));

    // 输入完整 trigger：短语候选前插（引擎语义由 engine-core 门禁钉住，这里只证接线）。
    s.input_key('d' as u32, 0);
    s.input_key('z' as u32, 0);
    assert!(s.candidates().iter().any(|w| w == "地址"));
}

#[test]
fn english_apply_pushes_only_the_english_slot() {
    let mut s = logic();
    s.apply_custom_phrases(
        parse(r#"{"version":1,"enabled":false,"phrases":[{"trigger":"dz","text":"地址"}]}"#)
            .unwrap(),
    );
    s.apply_english_custom_phrases(
        parse(
            r#"{"version":1,"enabled":true,"phrases":[{"trigger":"hello","text":"english phrase"}]}"#,
        )
        .unwrap(),
    );

    assert!(!s.engine.custom_phrases_enabled(), "中文启用态不受英文影响");
    assert!(s.engine.english_custom_phrases_enabled());
    assert!(s.engine.export_custom_phrases().contains("地址"));
    assert!(!s.engine.export_custom_phrases().contains("english phrase"));
    assert!(
        s.engine
            .export_english_custom_phrases()
            .contains("english phrase")
    );
    assert!(!s.engine.export_english_custom_phrases().contains("地址"));
}

#[test]
fn enabled_phrase_file_matches_typed_letters() {
    // 2026-10-25 用户报障「桌面端无法用配置的字母匹配」：宿主文件 → 推引擎 → 按键。
    let dir = temp_dir("match");
    let path = dir.join(FILE_NAME);
    std::fs::write(
        &path,
        r#"{"version":1,"enabled":true,"phrases":[{"trigger":"dz","text":"地址"}]}"#,
    )
    .unwrap();
    let mut s = logic();
    s.apply_custom_phrases(custom_phrases::load_file(&path));
    for c in "dz".chars() {
        s.input_key(c as u32, 0);
    }
    assert_eq!(s.candidates()[0], "地址", "配置的字母必须命中短语");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn enabling_reloads_the_same_table_and_matches_immediately() {
    // 2026-10-25 用户报障「开启也无法匹配」：开启只改启用位，**不能要求重新导入**。
    let mut s = logic();
    let off = parse(r#"{"version":1,"enabled":false,"phrases":[{"trigger":"dz","text":"地址"}]}"#)
        .unwrap();
    s.apply_custom_phrases(off);
    s.input_key('d' as u32, 0);
    s.input_key('z' as u32, 0);
    assert!(!s.candidates().iter().any(|w| w == "地址"), "关闭时不匹配");

    let on = parse(r#"{"version":1,"enabled":true,"phrases":[{"trigger":"dz","text":"地址"}]}"#)
        .unwrap();
    s.apply_custom_phrases(on);
    assert!(
        s.candidates().iter().any(|w| w == "地址"),
        "开启后同一缓冲立即命中"
    );
}

#[test]
fn english_phrase_matches_in_normal_english_mode() {
    let mut s = logic();
    s.apply_english_custom_phrases(
        parse(
            r#"{"version":1,"enabled":true,"phrases":[{"trigger":"hello","text":"english phrase"}]}"#,
        )
        .unwrap(),
    );

    // 2026-10-24：普通英文模式（pro=false）也把字母送进缓冲，否则字母直出进文档、
    // trigger 永远匹配不到；候选里短语排在最前，空格整段提交。
    s.switch_mode(Mode::English);
    assert!(!s.english_pro());
    for c in "hello".chars() {
        s.input_key(c as u32, 0);
    }
    assert_eq!(s.buffer(), "hello", "有英文短语时普通模式必须缓冲");
    assert_eq!(s.candidates()[0], "english phrase");

    // 关掉英文短语 ⇒ 回到直出：字母不留在缓冲。
    s.engine.set_english_custom_phrases_enabled(false);
    s.clear();
    for c in "hello".chars() {
        s.input_key(c as u32, 0);
    }
    assert_eq!(s.buffer(), "", "短语关闭后普通英文仍是直出");
    assert!(!s.candidates().iter().any(|w| w == "english phrase"));

    // 中文模式永不匹配英文套。
    s.engine.set_english_custom_phrases_enabled(true);
    s.switch_mode(Mode::Pinyin);
    for c in "hello".chars() {
        s.input_key(c as u32, 0);
    }
    assert!(!s.candidates().iter().any(|w| w == "english phrase"));
}

#[test]
fn english_phrase_switch_with_empty_table_keeps_direct_typing() {
    // 开关开但表为空 ⇒ 不改变英文直出（`active` 判据的第二半）。
    let mut s = logic();
    s.switch_mode(Mode::English);
    assert!(!s.english_phrases_active());
    for c in "hi".chars() {
        s.input_key(c as u32, 0);
    }
    assert_eq!(s.buffer(), "", "空表不得把普通英文变成缓冲模式");
}

#[test]
fn invalid_phrase_table_falls_back_to_empty_and_enabled() {
    let mut s = logic();
    s.apply_custom_phrases(
        parse(r#"{"version":1,"phrases":[{"trigger":"dz","text":"地址"}]}"#).unwrap(),
    );
    assert!(s.engine.export_custom_phrases().contains("地址"));

    // 形状合法、条目非法（trigger 不是字母）⇒ 引擎整表拒收 ⇒ 宿主退回空表 + 启用。
    let bad = parse(r#"{"version":1,"enabled":false,"phrases":[{"trigger":"1","text":"x"}]}"#)
        .expect("外层形状合法");
    s.apply_custom_phrases(bad);
    assert!(!s.engine.export_custom_phrases().contains("地址"));
    assert!(!s.engine.export_custom_phrases().contains("\"x\""));
    assert!(!s.engine.custom_phrases_enabled(), "坏表退回空表 + 关");
}

#[test]
fn load_reads_the_file_next_to_the_dicts_directory_end_to_end() {
    let dir = temp_dir("e2e");
    let dicts = dir.join("dicts");
    std::fs::create_dir_all(&dicts).unwrap();
    std::fs::write(
        path_from_dicts_dir(&dicts),
        r#"{"version":1,"enabled":false,"phrases":[{"trigger":"dz","text":"地址"}]}"#,
    )
    .unwrap();

    let mut s = logic();
    s.apply_custom_phrases(custom_phrases::load(&dicts));
    assert!(!s.engine.custom_phrases_enabled());
    assert!(s.engine.export_custom_phrases().contains("地址"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn invalid_english_table_falls_back_without_touching_chinese() {
    let mut s = logic();
    s.apply_custom_phrases(
        parse(r#"{"version":1,"phrases":[{"trigger":"dz","text":"地址"}]}"#).unwrap(),
    );
    let bad = parse(r#"{"version":1,"enabled":false,"phrases":[{"trigger":"1","text":"x"}]}"#)
        .expect("外层形状合法");
    s.apply_english_custom_phrases(bad);

    assert!(!s.engine.export_english_custom_phrases().contains("\"x\""));
    assert!(
        !s.engine.english_custom_phrases_enabled(),
        "坏表退回空表 + 关"
    );
    assert!(s.engine.export_custom_phrases().contains("地址"));
    assert!(
        !s.engine.custom_phrases_enabled(),
        "中文套缺省关，英文坏表不许把它打开"
    );
}

#[test]
fn load_english_reads_the_en_file_next_to_the_dicts_directory() {
    let dir = temp_dir("en-e2e");
    let dicts = dir.join("dicts");
    std::fs::create_dir_all(&dicts).unwrap();
    std::fs::write(
        english_path_from_dicts_dir(&dicts),
        r#"{"version":1,"enabled":false,"phrases":[{"trigger":"hello","text":"english phrase"}]}"#,
    )
    .unwrap();

    let mut s = logic();
    s.apply_english_custom_phrases(custom_phrases::load_english(&dicts));
    assert!(!s.engine.english_custom_phrases_enabled());
    assert!(
        s.engine
            .export_english_custom_phrases()
            .contains("english phrase")
    );
    assert_eq!(
        s.engine.export_custom_phrases(),
        r#"{"version":1,"phrases":[]}"#,
        "读英文文件不许碰中文槽位"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
