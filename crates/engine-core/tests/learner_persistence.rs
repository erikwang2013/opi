// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 学习持久化的**往返**与一致性（`Learner` / `Engine` 的用户词接口）。
//!
//! 内联单测已覆盖合并语义/上限/畸形输入，这里补三类只靠单点断言抓不住的：
//! - **导出 → 导入 → 导出**必须逐字节相等（幂等）；丢一个词、少一次 `user_words`
//!   插入，只有往返一次才看得出来（Android 每次启动都要走这条路径）。
//! - **删除**必须同时清两棵结构：`freq_of` 归零但 `user_words` 残留的话，导出会带出
//!   `{"text":"好","freq":0}` —— 用户删掉的自造词又冒出来。
//! - **上界**：导入是信任边界，重复导入同一份文件不得让频次随启动次数膨胀。

use engine_core::composer::Mode;
use engine_core::dictionary::InMemoryDictionary;
use engine_core::learner::{MAX_IMPORT_WORDS, UserWordExport};
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Learner};

fn engine() -> Engine {
    let mut d = InMemoryDictionary::new();
    d.insert("hao", "好", 5000);
    d.insert("hao", "号", 1200);
    d.insert("hao", "豪", 800);
    Engine::new(Box::new(d), SymbolEngine::new(Vec::new(), Vec::new()), true)
}

fn type_hao(e: &mut Engine) {
    for c in "hao".chars() {
        e.input_key(c);
    }
}

fn select_word(e: &mut Engine, word: &str) {
    type_hao(e);
    let idx = e
        .candidates(8)
        .iter()
        .position(|c| c.text == word)
        .unwrap_or_else(|| panic!("候选里没有 {word}"));
    assert_eq!(e.select(idx), word);
}

/// 导出 → 导入 → 导出 必须逐字节相同（导入是全有或全无的合并，不得丢词/丢频）。
#[test]
fn export_import_export_is_byte_identical() {
    let mut a = Learner::new(true);
    for w in ["好", "号", "豪", "nihao", "😄"] {
        a.record_selection(w);
        a.record_selection(w);
    }
    a.record_selection("好"); // 好 = 3
    let first = a.export_json();

    let mut b = Learner::new(true);
    assert_eq!(b.import_json(&first).unwrap(), 5);
    let second = b.export_json();
    assert_eq!(first, second, "往返后导出不一致（导入丢词或改名）");

    // 往返后的行为面也必须一致：频次排序不变
    let mut c = Learner::new(true);
    c.import_json(&second).unwrap();
    assert_eq!(c.freq_of("好"), 3);
    assert_eq!(c.freq_of("号"), 2);
    assert_eq!(c.freq_of("nihao"), 2);
}

/// 导出是**确定**的：同一份内存状态的 JSON 必须逐字节相同（BTreeSet 序）。
/// 换成 HashMap 之类非确定容器时，这条会以「两次导出不同」直接红。
#[test]
fn export_is_deterministic_and_codepoint_sorted() {
    let mut a = Learner::new(true);
    for w in ["豪", "好", "😄", "号", "b", "a", "A"] {
        a.record_selection(w);
    }
    let one = a.export_json();
    for _ in 0..8 {
        assert_eq!(a.export_json(), one, "同一状态导出结果不稳定");
    }
    let parsed: UserWordExport = serde_json::from_str(&one).unwrap();
    let texts: Vec<&str> = parsed.words.iter().map(|w| w.text.as_str()).collect();
    let mut sorted = texts.clone();
    sorted.sort();
    assert_eq!(texts, sorted, "导出顺序应按码点升序（BTreeSet）");
}

/// 文件里同一条词出现多次 → 取最大频次（不是累加、不是最后一条）。
#[test]
fn import_takes_max_among_duplicate_entries() {
    let mut l = Learner::new(true);
    let json = r#"{"version":1,"words":[
        {"text":"好","freq":5},{"text":"好","freq":3},{"text":"好","freq":9},{"text":"号","freq":1}
    ]}"#;
    assert_eq!(
        l.import_json(json).unwrap(),
        4,
        "返回值是文件里的条数，重复也计入"
    );
    assert_eq!(l.freq_of("好"), 9);
    // 导出不得重复列出
    let parsed: UserWordExport = serde_json::from_str(&l.export_json()).unwrap();
    assert_eq!(
        parsed.words.len(),
        2,
        "重复词条只应导出一条: {:?}",
        parsed.words
    );
}

/// 删除自造词必须同时清 `freq_of` 与导出的词表 —— 只清前者会让删掉的词带着 freq=0
/// 从导出里复活（Android 下次启动又会导回来）。
#[test]
fn remove_word_clears_both_structures() {
    let mut l = Learner::new(true);
    l.record_selection("好");
    l.record_selection("号");
    l.remove_word("好");
    assert_eq!(l.freq_of("好"), 0);
    let parsed: UserWordExport = serde_json::from_str(&l.export_json()).unwrap();
    assert!(
        parsed.words.iter().all(|w| w.text != "好"),
        "删除的词仍在导出里: {:?}",
        parsed.words
    );
    assert_eq!(parsed.words.len(), 1);
    // 再导入一次（模拟重启）也不得复活
    l.import_json(&l.export_json()).unwrap();
    assert_eq!(l.freq_of("好"), 0);
}

/// 引擎层：删掉自造词后排序必须回到静态序。
#[test]
fn engine_remove_user_word_restores_static_order() {
    let mut e = engine();
    select_word(&mut e, "豪");
    type_hao(&mut e);
    assert_eq!(e.candidates(8)[0].text, "豪", "前置：学过之后应排首位");
    e.remove_user_word("豪");
    type_hao(&mut e);
    assert_eq!(
        e.candidates(8)[0].text,
        "好",
        "删掉自造词后应回到静态词频序"
    );
    assert!(!e.export_user_words().contains("豪"));
}

/// 同一份文件**重复导入进同一个引擎**（还原备份、切换账号、前端重放）：
/// 频次必须幂等 —— 取 max 而非累加。累加会把频次放大到压垮静态词频，且**再也降不下来**
/// （导入只增不减：内存 10 备份 10 → 20，回写后下次再导入 → 40，每同步一轮翻一倍）。
#[test]
fn reimporting_the_same_file_is_idempotent() {
    let mut e = engine();
    select_word(&mut e, "豪");
    let file = e.export_user_words();
    // 同一个会话里连导 5 次（含把导出的文件再喂回去）
    for round in 0..5 {
        e.import_user_words(&file).unwrap();
        assert_eq!(
            e.export_user_words(),
            file,
            "第 {round} 轮重复导入后频次变了（导入在累加而不是取 max）"
        );
        type_hao(&mut e);
        assert_eq!(e.candidates(8)[0].text, "豪");
    }
    // 反复导入也不得把静态词序搞乱：其他词仍按静态频次排列
    type_hao(&mut e);
    assert_eq!(
        e.candidates(8).iter().map(|c| &c.text).collect::<Vec<_>>(),
        vec!["豪", "好", "号"],
        "重复导入后整体排序漂移"
    );
}

/// 上限是**条数**而不是字节数：恰好等于上限接受，超一条拒绝且不留痕。
#[test]
fn import_limit_is_exact() {
    let mut l = Learner::new(true);
    l.record_selection("好");
    let before = l.export_json();
    let one = r#"{"text":"甲","freq":1}"#;
    let over = format!(
        r#"{{"version":1,"words":[{}]}}"#,
        std::iter::repeat_n(one, MAX_IMPORT_WORDS + 1)
            .collect::<Vec<_>>()
            .join(",")
    );
    assert!(l.import_json(&over).is_err(), "超过上限必须拒绝");
    assert_eq!(l.export_json(), before, "拒绝后不得留痕");
    let exact = format!(
        r#"{{"version":1,"words":[{}]}}"#,
        std::iter::repeat_n(one, MAX_IMPORT_WORDS)
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(l.import_json(&exact).unwrap(), MAX_IMPORT_WORDS);
    assert_eq!(l.freq_of("甲"), 1);
    assert_eq!(l.freq_of("好"), 1, "既有词不得被清掉（合并而非替换）");
}

/// 学习关闭时**选词不入库**，但**导入照旧生效**（用户关掉学习再打开，词表不得消失）。
#[test]
fn disabled_learner_still_loads_user_file() {
    let mut e = engine();
    select_word(&mut e, "豪");
    let file = e.export_user_words();

    let mut off = engine();
    off.set_learner(false);
    assert!(off.import_user_words(&file).is_ok());
    type_hao(&mut off);
    assert_eq!(
        off.candidates(8)[0].text,
        "豪",
        "关掉学习不得影响用户词表载入（否则用户关一次开关，词库就没了）"
    );
    // 但新选词不记录
    let before = off.export_user_words();
    select_word(&mut off, "号");
    assert_eq!(off.export_user_words(), before, "关闭学习后选词不得入库");
}

/// 简繁双库时 boost 取两库最大词频 → 学过的词在两个模式下都能反超。
#[test]
fn learning_applies_in_traditional_mode_too() {
    let mut simp = InMemoryDictionary::new();
    simp.insert("hao", "好", 5000);
    let mut trad = InMemoryDictionary::new();
    // 繁体库静态词频远高于简体库：boost 必须按两库最大值缩放
    trad.insert("hao", "好", 3_000_000_000);
    trad.insert("hao", "號", 2_900_000_000);
    let mut e = Engine::with_dictionaries(
        Box::new(simp),
        Some(Box::new(trad)),
        SymbolEngine::new(Vec::new(), Vec::new()),
        true,
    );
    e.switch_mode(Mode::Traditional);
    type_hao(&mut e);
    let idx = e
        .candidates(8)
        .iter()
        .position(|c| c.text == "號")
        .expect("繁体库候选");
    assert_eq!(e.select(idx), "號");
    type_hao(&mut e);
    assert_eq!(
        e.candidates(8)[0].text,
        "號",
        "一次选词必须压过 29 亿静态词频（boost 未按两库最大值缩放）"
    );
    // 切回简体模式：同一份学习记录不得影响简体排序
    e.switch_mode(Mode::Pinyin);
    type_hao(&mut e);
    assert_eq!(e.candidates(8)[0].text, "好");
}
