// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

use engine_core::composer::Mode;

use super::*;

/// 单例串行化：install/install_trad 与 install_singleton_fallback_and_path
/// 共享 SINGLETON，并行执行时相互复位会制造竞态，加锁串行。
static SINGLETON_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn api() -> Api {
    Api::load_fallback_sync().expect("fallback load")
}

#[test]
fn load_fallback_sync_ok() {
    let a = api();
    assert_eq!(a.buffer(), "");
    assert_eq!(a.mode(), ApiMode::Pinyin);
    assert!(a.learner_enabled()); // M1 默认开
}

#[test]
fn pinyin_input_produces_candidates() {
    let mut a = api();
    a.input_key("w".into());
    a.input_key("o".into());
    assert_eq!(a.buffer(), "wo");
    let cands = a.candidates(3);
    assert_eq!(cands[0].text, "我");
    assert_eq!(cands[0].kind, ApiCandidateKind::Hanzi);
    assert!(cands[0].score > 0);
}

#[test]
fn select_commits_and_records_learner() {
    let mut a = api();
    for c in ["w", "o"] {
        a.input_key(c.into());
    }
    let text = a.select(0);
    assert_eq!(text, "我");
    assert_eq!(a.buffer(), "");
    assert!(a.export_user_words().contains("我"));
    a.remove_user_word("我".into());
    assert!(!a.export_user_words().contains("我"));
    a.clear_user_words();
    assert_eq!(a.export_user_words(), r#"{"version":1,"words":[]}"#);
}

#[test]
fn input_key_boundary_rejects_non_single_char() {
    let mut a = api();
    assert_eq!(a.input_key("".into()), "");
    assert_eq!(a.input_key("ab".into()), "");
    assert_eq!(a.input_key("你".into()), ""); // 非 ASCII 也拒绝
    assert_eq!(a.buffer(), "");
}

#[test]
fn select_out_of_range_returns_empty() {
    let mut a = api();
    for c in ["w", "o"] {
        a.input_key(c.into());
    }
    assert_eq!(a.select(999), "");
}

#[test]
fn mode_and_shift_and_space() {
    let mut a = api();
    a.switch_mode(ApiMode::English);
    for c in ["a", "b", "c"] {
        a.input_key(c.into());
    }
    assert_eq!(a.buffer(), "abc");
    assert_eq!(a.input_space(), "abc");
    assert_eq!(a.buffer(), "");
    a.set_shift(true);
    a.input_key("a".into());
    assert_eq!(a.buffer(), "A");
    a.set_shift(false);
    a.switch_mode(ApiMode::Pinyin);
}

#[test]
fn backspace_and_clear() {
    let mut a = api();
    a.input_key("w".into());
    a.backspace();
    assert_eq!(a.buffer(), "");
    a.input_key("w".into());
    a.clear();
    assert_eq!(a.buffer(), "");
}

#[test]
fn symbol_search_and_blocks() {
    let a = api();
    let blocks = a.symbol_blocks();
    assert!(!blocks.is_empty());
    let hits = a.search_symbols("he".into());
    assert!(hits.iter().any(|s| s.text == "♥"));
    let b0 = blocks[0].clone();
    assert!(!a.symbols_in_block(b0.id).is_empty());
}

#[test]
fn set_learner_toggles() {
    let mut a = api();
    a.set_learner(false);
    assert!(!a.learner_enabled());
    a.set_learner(true);
    assert!(a.learner_enabled());
}

#[test]
fn install_singleton_fallback_and_path() {
    let _g = SINGLETON_GUARD.lock().unwrap_or_else(|p| p.into_inner());
    // 空串/None → 内置回退
    assert!(install(None).is_ok());
    assert!(with_engine(|a| a.buffer()).is_some());
    // 坏路径 → Err（load_or_fallback 原样语义，不回退）
    assert!(install(Some("/nonexistent/opi.dict")).is_err());
    // 单例保持上一次成功装载可用
    assert!(with_engine(|a| a.buffer()).is_some());
}

#[test]
fn mode_int_roundtrip() {
    assert_eq!(mode_from_int(0), Some(Mode::Pinyin));
    assert_eq!(mode_from_int(3), Some(Mode::Symbol));
    assert_eq!(mode_from_int(4), Some(Mode::Traditional));
    assert_eq!(mode_from_int(5), None);
    assert_eq!(mode_from_int(-1), None);
    assert_eq!(mode_to_int(Mode::English), 1);
    assert_eq!(mode_to_int(Mode::Number), 2);
    assert_eq!(mode_to_int(Mode::Traditional), 4);
}

#[test]
fn install_trad_hooks_trad_dict() {
    let _g = SINGLETON_GUARD.lock().unwrap_or_else(|p| p.into_inner());
    // 引擎未加载 → Err
    *SINGLETON.lock().unwrap() = None;
    assert!(install_trad("/nonexistent/trad.opid").is_err());
    // 先装主库：坏路径 → Err 且简体模式不受影响
    install(None).unwrap();
    assert!(install_trad("/nonexistent/trad.opid").is_err());
    // 真路径：临时编译一个小词典（engine_data 序列化，opi-ffi 不依赖 opi-tools）
    let dict = engine_data::format::OpDict {
        entries: vec![engine_data::format::RawEntry {
            pinyin: "fa".into(),
            word: "發".into(),
            freq: 4000,
        }],
        pinyin_total: 2,
    };
    let tmp = std::env::temp_dir().join("opi_trad_test.opid");
    std::fs::write(&tmp, engine_data::serialize(&dict)).unwrap();
    install_trad(tmp.to_str().unwrap()).unwrap();
    // 繁体模式候选走 trad 词典
    let top = with_engine(|a| {
        a.switch_mode(ApiMode::Traditional);
        a.input_key("f".into());
        a.input_key("a".into());
        a.candidates(8)[0].text.clone()
    })
    .expect("engine loaded");
    assert_eq!(top, "發");
    std::fs::remove_file(&tmp).ok();
    with_engine(|a| a.switch_mode(ApiMode::Pinyin));
}

#[test]
fn install_trad_recomputes_user_boost() {
    let _g = SINGLETON_GUARD.lock().unwrap_or_else(|p| p.into_inner());
    // 高静态词频（4e9，trad.opid 同量级）安装后，learner 选词仍压过静态词。
    *SINGLETON.lock().unwrap() = None;
    install(None).unwrap();
    let dict = engine_data::format::OpDict {
        entries: vec![
            engine_data::format::RawEntry {
                pinyin: "fa".into(),
                word: "發".into(),
                freq: 4_000_000_000,
            },
            engine_data::format::RawEntry {
                pinyin: "fa".into(),
                word: "髮".into(),
                freq: 3_999_000_000,
            },
        ],
        pinyin_total: 4, // 两条 "fa" 拼音 blob 共 4 字节（serialize debug_assert 校验）
    };
    let tmp = std::env::temp_dir().join("opi_trad_boost.opid");
    std::fs::write(&tmp, engine_data::serialize(&dict)).unwrap();
    install_trad(tmp.to_str().unwrap()).unwrap();
    std::fs::remove_file(&tmp).ok();
    with_engine(|a| {
        a.set_learner(true);
        a.switch_mode(ApiMode::Traditional);
        a.input_key("f".into());
        a.input_key("a".into());
        assert_eq!(a.candidates(8)[0].text, "發");
        a.select(1); // 选 髮
        a.input_key("f".into());
        a.input_key("a".into());
        assert_eq!(
            a.candidates(8)[0].text,
            "髮",
            "learner 选词应压过 4e9 静态词"
        );
        a.set_learner(false);
    });
    with_engine(|a| a.switch_mode(ApiMode::Pinyin));
}
