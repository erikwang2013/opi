//! C ABI 集成测试：host 上直接调 opi_* 函数，验证装载/候选/选择/模式/符号主链路。
//! 与 JNI 出口共享同一单例与实现；测试间用 SERIAL 互斥避免单例串扰。

use std::sync::Mutex;

use opi_ffi::cabi::{
    opi_backspace, opi_buffer, opi_candidates, opi_clear, opi_clear_user_words, opi_export_user_words,
    opi_ffi_free_string, opi_import_user_words, opi_input_key, opi_input_space, opi_learner_enabled, opi_load,
    opi_load_trad, opi_mode, opi_remove_user_word, opi_select, opi_search_symbols, opi_set_learner,
    opi_set_shift, opi_switch_mode, opi_symbol_blocks, opi_symbols_in_block, OpiString,
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

/// 仓库内资产路径：以 `CARGO_MANIFEST_DIR` 为锚，不得用相对路径 —— 相对路径
/// 依赖进程 cwd，直接跑测试二进制（或换目录跑）时会读到别处，断言全红。
/// 与 crates/opi-tools/tests/trad_coverage.rs 的写法一致。
const LUNA_OPID: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../android/app/src/main/assets/luna.opid");
const TRAD_OPID: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/generated/trad.opid");

/// 装载：优先 luna 词库（存在则真实加载），缺失走内置回退（也必须成功）。
fn load_any() {
    let p = to_units(LUNA_OPID);
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
fn cabi_symbols_in_block_negative_id_is_empty() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    let blocks = read(unsafe { opi_symbol_blocks() });
    let v: Vec<serde_json::Value> = serde_json::from_str(&blocks).unwrap_or_default();
    let id0 = v[0]["id"].as_u64().expect("id 为数字") as i16;
    // 前置：首块非空 —— 若钳位把 -1 变成某个真实块，这条就会红
    assert!(!read_texts(unsafe { opi_symbols_in_block(id0) }).is_empty());
    // 负 id 与 JNI 侧 opijni_symbols_in_block 同语义：越界 → 空数组，不钳成块 0
    assert!(
        read_texts(unsafe { opi_symbols_in_block(-1) }).is_empty(),
        "负 id 必须返回空数组（iOS 侧与 Android 语义需一致）"
    );
}

/// 长按删词（B4）：C ABI 出口 opi_remove_user_word。
/// 前置用 select 造一条真实学习词，再按 UI 路径删；负例（删不存在的词）不得改动状态。
#[test]
fn cabi_remove_user_word_deletes_and_is_noop_on_miss() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe {
        opi_clear_user_words();
        opi_set_learner(true);
        opi_switch_mode(0);
    }
    assert!(!read(unsafe { opi_export_user_words() }).contains("我"), "前置：清空后不含 我");

    // 走真实路径造学习词：输入 wo 并选首候选
    let w = to_units("w");
    let o = to_units("o");
    unsafe {
        opi_input_key(w.as_ptr(), w.len());
        opi_input_key(o.as_ptr(), o.len());
    }
    assert!(!read(unsafe { opi_select(0) }).is_empty());
    let words = read(unsafe { opi_export_user_words() });
    assert!(words.contains("我"), "学习词应含 我，实际: {words}");

    // 命中：删除后导出不再含 我
    let wo_units = to_units("我");
    unsafe { opi_remove_user_word(wo_units.as_ptr(), wo_units.len()) };
    let after = read(unsafe { opi_export_user_words() });
    assert!(!after.contains("我"), "删除后不应再含 我，实际: {after}");

    // 负例：删不存在的词 → 无操作（不 panic，状态逐字节不变）
    let miss = to_units("鸝");
    unsafe { opi_remove_user_word(miss.as_ptr(), miss.len()) };
    assert_eq!(read(unsafe { opi_export_user_words() }), after, "删不存在的词不得改动状态");

    // null 指针同空串；空串删除不 panic 且不改状态
    unsafe { opi_remove_user_word(std::ptr::null(), 0) };
    assert_eq!(read(unsafe { opi_export_user_words() }), after, "null 入参不得改动状态");

    unsafe {
        opi_clear_user_words();
        opi_clear();
    }
}

/// 学习持久化（B4）：导入合法 JSON → 条数正确且候选排序真的受影响。
#[test]
fn cabi_import_user_words_valid_json_affects_ranking() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe {
        opi_clear_user_words();
        opi_switch_mode(0);
        opi_clear();
    }
    let w = to_units("w");
    let o = to_units("o");

    // 前置：取 "wo" 的第 2 个候选作为导入目标（非首位，排序变化才可观测）
    unsafe {
        opi_input_key(w.as_ptr(), w.len());
        opi_input_key(o.as_ptr(), o.len());
    }
    let before = read_texts(unsafe { opi_candidates(8) });
    assert!(before.len() >= 2, "需要 ≥2 个候选才能证明排序变化，实际: {before:?}");
    let target = before[1].clone();
    unsafe { opi_clear() };

    // freq=1 即足够：rank_score = 静态 + user_freq×boost，boost ≥ 2×最大静态词频，
    // 故 1×boost 已压过全部静态词（见 engine-core/src/candidates.rs:31 与 engine.rs:33）。
    let json = format!(r#"{{"version":1,"words":[{{"text":"{target}","freq":1}}]}}"#);
    let ju = to_units(&json);
    assert_eq!(
        unsafe { opi_import_user_words(ju.as_ptr(), ju.len()) },
        1,
        "导入条数应为 1"
    );
    assert!(
        read(unsafe { opi_export_user_words() }).contains(&target),
        "导入的词应出现在导出中"
    );

    unsafe {
        opi_input_key(w.as_ptr(), w.len());
        opi_input_key(o.as_ptr(), o.len());
    }
    let after = read_texts(unsafe { opi_candidates(8) });
    assert_eq!(after[0], target, "导入的词应升到首位：before={before:?} after={after:?}");

    unsafe {
        opi_clear();
        opi_clear_user_words();
    }
}

/// 导入失败必须原子：返回负值，且既有状态逐字节不变（先解析成功后提交）。
#[test]
fn cabi_import_user_words_invalid_json_is_negative_and_atomic() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_clear_user_words() };

    // 造一份「既有状态」，后续所有失败都必须不碰它
    let good = r#"{"version":1,"words":[{"text":"鸝","freq":7}]}"#;
    let gu = to_units(good);
    assert_eq!(unsafe { opi_import_user_words(gu.as_ptr(), gu.len()) }, 1);
    let baseline = read(unsafe { opi_export_user_words() });
    assert!(baseline.contains("鸝"), "前置：既有状态含 鸝，实际: {baseline}");

    for bad in [
        "",                                        // 空串
        "not json",                                // 非 JSON
        r#"{"version":2,"words":[]}"#,             // 版本不符
        r#"{"version":1,"words":[{"text":"","freq":1}]}"#, // 空词条
        r#"{"version":1}"#,                        // 缺 words
    ] {
        let bu = to_units(bad);
        let n = unsafe { opi_import_user_words(bu.as_ptr(), bu.len()) };
        assert!(n < 0, "非法输入应返回负值，实际 {n}（输入 {bad:?}）");
        assert_eq!(
            read(unsafe { opi_export_user_words() }),
            baseline,
            "失败不得改动既有状态（输入 {bad:?}）"
        );
    }

    // null 指针同样按失败处理且不改状态
    assert!(unsafe { opi_import_user_words(std::ptr::null(), 0) } < 0, "null 应返回负值");
    assert_eq!(read(unsafe { opi_export_user_words() }), baseline, "null 不得改动状态");

    // 幂等：再导入同一份合法 JSON，频次取 max 而非累加
    let gu2 = to_units(good);
    assert_eq!(unsafe { opi_import_user_words(gu2.as_ptr(), gu2.len()) }, 1);
    assert_eq!(
        read(unsafe { opi_export_user_words() }),
        baseline,
        "重复导入应幂等（频次取 max）"
    );

    unsafe { opi_clear_user_words() };
}

#[test]
fn cabi_load_trad_routes_traditional_mode() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    // 坏路径 → false（不影响已装主词典）
    let bad = to_units("/nonexistent/trad.opid");
    assert!(!unsafe { opi_load_trad(bad.as_ptr(), bad.len()) });
    // 真路径：仓库内 trad.opid（Task 1 已提交）
    let p = to_units(TRAD_OPID);
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
