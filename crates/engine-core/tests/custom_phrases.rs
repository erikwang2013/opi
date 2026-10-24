// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 自定义短语门禁（2026-10-23）：导入/导出、校验原子性、精确优先、9/14 键编码、
//! 与既有候选路径的隔离。全部经 `Engine::candidates` / `Engine::select` 真路径。

use engine_core::candidates::{Candidate, CandidateKind};
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, InMemoryDictionary, Mode, Scheme};

fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

fn engine(entries: &[(&str, &str, u32)]) -> Engine {
    let mut d = InMemoryDictionary::new();
    for (py, w, f) in entries {
        d.insert(py, w, *f);
    }
    Engine::new(Box::new(d), no_symbols(), false)
}

fn type_keys(e: &mut Engine, keys: &str) {
    for ch in keys.chars() {
        e.input_key(ch);
    }
}

fn texts(cands: &[Candidate]) -> Vec<&str> {
    cands.iter().map(|c| c.text.as_str()).collect()
}

const ONE_PHRASE: &str = r#"{"version":1,"phrases":[{"trigger":"dz","text":"地址"}]}"#;

#[test]
fn import_export_round_trip_normalizes_trigger_case() {
    let mut e = engine(&[]);
    // 大写输入、规范导出（小写），schema/顺序与契约一致。
    let n = e
        .set_custom_phrases(r#"{"version":1,"phrases":[{"trigger":"DZ","text":"地址"}]}"#)
        .expect("合法短语应导入成功");
    assert_eq!(n, 1);
    assert_eq!(
        e.export_custom_phrases(),
        r#"{"version":1,"phrases":[{"trigger":"dz","text":"地址"}]}"#
    );
    // 再导入一份空表 = 清空（同样合法）。
    assert_eq!(e.set_custom_phrases(r#"{"version":1,"phrases":[]}"#), Ok(0));
    assert_eq!(e.export_custom_phrases(), r#"{"version":1,"phrases":[]}"#);
}

#[test]
fn invalid_input_leaves_the_old_list_untouched() {
    let mut e = engine(&[]);
    e.set_custom_phrases(ONE_PHRASE).unwrap();
    let before = e.export_custom_phrases();
    let too_long = "a".repeat(33);
    let many = format!(
        r#"{{"version":1,"phrases":[{}]}}"#,
        std::iter::repeat_n(r#"{"trigger":"a","text":"x"}"#, 2001)
            .collect::<Vec<_>>()
            .join(",")
    );
    let bad = [
        "not json".to_string(),
        r#"{"version":2,"phrases":[]}"#.to_string(),
        r#"{"version":1,"phrases":[{"trigger":"","text":"x"}]}"#.to_string(),
        r#"{"version":1,"phrases":[{"trigger":"d1","text":"x"}]}"#.to_string(),
        format!(r#"{{"version":1,"phrases":[{{"trigger":"{too_long}","text":"x"}}]}}"#),
        r#"{"version":1,"phrases":[{"trigger":"dz","text":""}]}"#.to_string(),
        // 归一后重复（DZ vs dz）⇒ 整份拒绝（不是 last-wins）。
        r#"{"version":1,"phrases":[{"trigger":"dz","text":"a"},{"trigger":"DZ","text":"b"}]}"#
            .to_string(),
        many,
    ];
    for json in bad {
        assert!(e.set_custom_phrases(&json).is_err(), "应拒绝：{json:.80}");
        assert_eq!(
            e.export_custom_phrases(),
            before,
            "失败后旧表必须原样保留：{json:.80}"
        );
    }
}

#[test]
fn exact_match_is_first_and_select_commits_phrase_text() {
    let mut e = engine(&[("nihao", "你好", 9000), ("ni", "你", 5000)]);
    e.set_custom_phrases(r#"{"version":1,"phrases":[{"trigger":"nihao","text":"自定义"}]}"#)
        .unwrap();
    e.set_custom_phrases_enabled(true);
    type_keys(&mut e, "nihao");
    let cands = e.candidates(8);
    assert_eq!(cands[0].text, "自定义");
    assert_eq!(cands[0].kind, CandidateKind::Hanzi);
    assert!(
        cands.iter().any(|c| c.text == "你好"),
        "正常候选仍要在：{cands:?}"
    );
    // 选中短语：整串上屏、缓冲清空。
    assert_eq!(e.select(0), "自定义");
    assert_eq!(e.buffer(), "");
    // 空格（移动端主提交键）走同一条：首候选即短语，且不重复拼接。
    type_keys(&mut e, "nihao");
    assert_eq!(e.input_space(), "自定义");
    assert_eq!(e.buffer(), "");
}

#[test]
fn phrase_beats_segment_fallback_and_consumes_the_whole_buffer() {
    // 对照：没有短语时 `zhangsan` 只命中 `zhang` ⇒ 分词回退留下 `san`、本轮不提交。
    let mut plain = engine(&[("zhang", "张", 100)]);
    type_keys(&mut plain, "zhangsan");
    assert_eq!(plain.select(0), "", "前置：回退态本轮不提交");
    assert_eq!(plain.buffer(), "san", "前置：回退只消耗首音节");

    let mut e = engine(&[("zhang", "张", 100)]);
    e.set_custom_phrases(r#"{"version":1,"phrases":[{"trigger":"zhangsan","text":"张三"}]}"#)
        .unwrap();
    e.set_custom_phrases_enabled(true);
    type_keys(&mut e, "zhangsan");
    assert_eq!(e.candidates(8)[0].text, "张三");
    assert_eq!(e.select(0), "张三", "短语必须整段提交");
    assert_eq!(e.buffer(), "", "短语选中后缓冲必须清空");
}

#[test]
fn phrases_are_disabled_until_explicitly_enabled() {
    // 2026-10-25 用户裁决：缺省关；加短语后显式开启即匹配（不必重新导入）。
    let mut e = engine(&[]);
    e.set_custom_phrases(ONE_PHRASE).unwrap();
    assert!(!e.custom_phrases_enabled(), "缺省必须是关");
    type_keys(&mut e, "dz");
    assert!(
        !texts(&e.candidates(8)).contains(&"地址"),
        "未开启不参与匹配"
    );

    e.set_custom_phrases_enabled(true);
    assert!(texts(&e.candidates(8)).contains(&"地址"), "开启后立即命中");
}

#[test]
fn disabled_hides_phrases_and_matching_is_exact() {
    let mut e = engine(&[("nihao", "你好", 9000)]);
    e.set_custom_phrases(r#"{"version":1,"phrases":[{"trigger":"nihao","text":"自定义"}]}"#)
        .unwrap();
    e.set_custom_phrases_enabled(true);
    type_keys(&mut e, "niha"); // 前缀不算
    assert!(!texts(&e.candidates(8)).contains(&"自定义"), "只认精确相等");

    e.clear();
    type_keys(&mut e, "nihao");
    e.set_custom_phrases_enabled(false);
    assert!(!e.custom_phrases_enabled());
    assert!(!texts(&e.candidates(8)).contains(&"自定义"));

    e.set_custom_phrases_enabled(true);
    assert!(e.custom_phrases_enabled());
    assert_eq!(e.candidates(8)[0].text, "自定义");
}

#[test]
fn separator_in_buffer_is_ignored() {
    let mut e = engine(&[("nihao", "你好", 9000)]);
    e.set_custom_phrases(r#"{"version":1,"phrases":[{"trigger":"nihao","text":"自定义"}]}"#)
        .unwrap();
    e.set_custom_phrases_enabled(true);
    type_keys(&mut e, "ni'hao");
    assert_eq!(e.candidates(8)[0].text, "自定义");
}

#[test]
fn nine_key_encodes_trigger_letters_and_keeps_insertion_order() {
    let mut e = engine(&[]);
    // d/e 都在 3 键、z 在 9 键 ⇒ 两条 trigger 都精确命中缓冲 "39"。
    e.set_custom_phrases(
        r#"{"version":1,"phrases":[{"trigger":"dz","text":"地址"},{"trigger":"ez","text":"恶之"}]}"#,
    )
    .unwrap();
    e.set_custom_phrases_enabled(true);
    e.set_scheme(Scheme::Nine);
    type_keys(&mut e, "39");
    let cands = e.candidates(8);
    let got = texts(&cands);
    assert_eq!(
        &got[..2],
        &["地址", "恶之"],
        "命中多条时保持导入顺序：{got:?}"
    );

    e.clear();
    type_keys(&mut e, "38"); // 8 键不含 z ⇒ 不命中
    assert!(!texts(&e.candidates(8)).contains(&"地址"));
}

#[test]
fn fourteen_key_encodes_trigger_letters() {
    let mut e = engine(&[]);
    e.set_custom_phrases(ONE_PHRASE).unwrap();
    e.set_custom_phrases_enabled(true);
    e.set_scheme(Scheme::Fourteen);
    type_keys(&mut e, "dz"); // 14 键 d→d、z→z，键面符号就是字母
    assert_eq!(e.candidates(8)[0].text, "地址");
}

#[test]
fn letter_schemes_match_but_stroke_does_not() {
    // 双拼/五笔的缓冲同样是字母原文 ⇒ 精确整串可匹配（2026-10-25 桌面报障）。
    for scheme in [Scheme::Shuangpin, Scheme::Wubi] {
        let mut e = engine(&[]);
        e.set_custom_phrases(ONE_PHRASE).unwrap();
        e.set_custom_phrases_enabled(true);
        e.set_scheme(scheme);
        type_keys(&mut e, "dz");
        assert!(
            texts(&e.candidates(8)).contains(&"地址"),
            "{scheme:?} 应按字母精确匹配"
        );
    }
    // 笔画串是 hspnz，不是配置的字母表。
    let mut e = engine(&[]);
    e.set_custom_phrases(ONE_PHRASE).unwrap();
    e.set_custom_phrases_enabled(true);
    e.set_scheme(Scheme::Stroke);
    type_keys(&mut e, "dz");
    assert!(!texts(&e.candidates(8)).contains(&"地址"));
}

#[test]
fn phrases_respect_limit() {
    let mut e = engine(&[("dz", "打", 100)]);
    e.set_custom_phrases(ONE_PHRASE).unwrap();
    e.set_custom_phrases_enabled(true);
    type_keys(&mut e, "dz");
    let one = e.candidates(1);
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].text, "地址", "limit=1 仍然先是短语");
    assert!(e.candidates(0).is_empty(), "limit=0 ⇒ 空表");
}

const CN_SAME_TRIGGER: &str = r#"{"version":1,"phrases":[{"trigger":"hello","text":"中文短语"}]}"#;
const EN_SAME_TRIGGER: &str =
    r#"{"version":1,"phrases":[{"trigger":"hello","text":"english phrase"}]}"#;

#[test]
fn chinese_and_english_sets_never_cross_modes() {
    // 两套用**同一个 trigger**：串门的话会立刻看见对方的文本。
    let mut e = engine(&[("hello", "哈喽", 100)]);
    e.set_custom_phrases(CN_SAME_TRIGGER).unwrap();
    e.set_english_custom_phrases(EN_SAME_TRIGGER).unwrap();
    e.set_custom_phrases_enabled(true);
    e.set_english_custom_phrases_enabled(true);

    type_keys(&mut e, "hello");
    let cn_cands = e.candidates(8);
    let cn = texts(&cn_cands);
    assert_eq!(cn[0], "中文短语", "中文键盘只用中文套：{cn:?}");
    assert!(!cn.contains(&"english phrase"), "英文套不许在中文键盘出现");

    e.clear();
    e.switch_mode(Mode::English);
    type_keys(&mut e, "hello");
    let en_cands = e.candidates(8);
    let en = texts(&en_cands);
    assert_eq!(en[0], "english phrase", "英文键盘只用英文套：{en:?}");
    assert!(!en.contains(&"中文短语"), "中文套不许在英文键盘出现");
    // 选中/空格都整段提交英文短语（复用中文套同一条收尾路径）。
    assert_eq!(e.input_space(), "english phrase");
    assert_eq!(e.buffer(), "");
}

#[test]
fn english_and_chinese_enabled_flags_are_independent() {
    let mut e = engine(&[]);
    e.set_custom_phrases(CN_SAME_TRIGGER).unwrap();
    e.set_english_custom_phrases(EN_SAME_TRIGGER).unwrap();
    e.set_custom_phrases_enabled(true);

    e.set_english_custom_phrases_enabled(false);
    assert!(!e.english_custom_phrases_enabled());
    assert!(e.custom_phrases_enabled(), "关英文套不能连带关中文套");

    type_keys(&mut e, "hello");
    assert_eq!(
        e.candidates(8)[0].text,
        "中文短语",
        "中文套不受英文开关影响"
    );

    e.clear();
    e.switch_mode(Mode::English);
    type_keys(&mut e, "hello");
    assert!(
        !texts(&e.candidates(8)).contains(&"english phrase"),
        "英文套关掉后不匹配"
    );

    e.set_custom_phrases_enabled(false);
    e.set_english_custom_phrases_enabled(true);
    e.clear();
    type_keys(&mut e, "hello");
    assert_eq!(
        e.candidates(8)[0].text,
        "english phrase",
        "中文套关掉不影响英文套"
    );
}

#[test]
fn english_set_requires_exact_letters_not_prefix() {
    let mut e = engine(&[]);
    e.set_english_custom_phrases(EN_SAME_TRIGGER).unwrap();
    e.set_english_custom_phrases_enabled(true);
    e.switch_mode(Mode::English);
    type_keys(&mut e, "hell");
    assert!(!texts(&e.candidates(8)).contains(&"english phrase"));
    type_keys(&mut e, "o");
    assert_eq!(e.candidates(8)[0].text, "english phrase");
    // 导出只含英文套自己的表（两套的存储互不影响）。
    assert_eq!(
        e.export_english_custom_phrases(),
        r#"{"version":1,"phrases":[{"trigger":"hello","text":"english phrase"}]}"#
    );
    assert_eq!(e.export_custom_phrases(), r#"{"version":1,"phrases":[]}"#);
}

#[test]
fn no_phrase_means_byte_identical_candidates() {
    let mut plain = engine(&[("nihao", "你好", 9000), ("ni", "你", 5000)]);
    let mut with = engine(&[("nihao", "你好", 9000), ("ni", "你", 5000)]);
    with.set_custom_phrases(ONE_PHRASE).unwrap();
    type_keys(&mut plain, "niha"); // trigger "dz" 不命中
    type_keys(&mut with, "niha");
    assert_eq!(plain.candidates(8), with.candidates(8));
}
