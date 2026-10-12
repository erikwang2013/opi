// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 9 键读数缓存的门禁（2026-10-09 二期从 `scheme_keys.rs` 拆出 —— 那边加语言切换
//! 用例后越 500 行）。两条钉的都是**「读取不许撒谎」**：读数缓存必须与算它时的缓冲
//! 绑存、缓冲一变即作废，且**不依赖调用方「之后再取一次候选」的次序**。
//! 逐条读数与候选的对齐也在本条覆盖（`candidate_readings` ↔ `candidates`）。

use engine_core::symbols::SymbolEngine;
use engine_core::{Dictionary, Engine, InMemoryDictionary, Scheme};

fn dict(entries: &[(&str, &str, u32)]) -> Box<dyn Dictionary> {
    let mut d = InMemoryDictionary::new();
    for (py, w, f) in entries {
        d.insert(py, w, *f);
    }
    Box::new(d)
}

/// 符号表用空表：本文件测的是读数缓存，符号数据不该参与断言。
fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

fn engine() -> Engine {
    Engine::new(
        dict(&[
            ("ni", "你", 5000),
            ("ni", "尼", 900),
            ("nihao", "你好", 6000),
            ("li", "里", 4000),
            ("hao", "好", 7000),
            ("wang", "王", 3000),
        ]),
        no_symbols(),
        false,
    )
}

fn type_str(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

/// 读数缓存**不许撒谎**（2026-10-09 用户报障「确定候选词后左列不恢复标点」的引擎侧
/// 根因）：缓冲一变（提交/退格），即使**没再取一次候选**，`display_buffer()` 也必须
/// 立刻为空。旧实现里读数是 `candidates()` 的副产物、不随缓冲作废，谁按「先读读数、
/// 再取候选」的次序刷新，谁就为提交后的那一次读到旧拼音 —— 而且挂到下一次按键为止。
#[test]
fn display_buffer_dies_with_the_buffer_not_with_the_next_fetch() {
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64"); // ni
    let _ = e.candidates(8);
    assert_eq!(e.display_buffer(), "ni", "前置：读得到拼音");

    // 提交：内层 candidates 把读数刷成 "64" 那一次算的；提交之后**不再取候选**。
    let _ = e.select_from(&e.candidates(8), 0);
    assert_eq!(e.buffer(), "", "前置：提交后缓冲已清");
    assert_eq!(e.display_buffer(), "", "提交后不得报旧缓冲的读数");

    // 退格改缓冲同理：旧读数立刻作废（下次 candidates() 才会给出新缓冲的读数）。
    type_str(&mut e, "64");
    let _ = e.candidates(8);
    assert_eq!(e.display_buffer(), "ni");
    e.backspace();
    assert_eq!(e.display_buffer(), "", "退格后旧读数立刻作废");
}

/// 读数出口（9 键左列的数据源）**去重保序**（2026-10-12 用户裁决「左列去重」）：
/// 读数按**首次出现序**去重 —— 同一读音只出一行，行的顺序 = 它最好那条候选的名次。
/// 首条仍 = `display_buffer()`（同一对缓存的两个视图）；缓冲一变（退格/提交）⇒ 立刻为空。
///
/// ⚠️ 旧契约是「与 `candidates(limit)` 同序等长、逐条对齐」（`readings[i]` = 第 i 条
/// 候选的拼音）—— 那条契约让左列出现 hao gao gan gao gan… 的重复行（真库 `426` 实测
/// 8 行里 5 行重复），用户裁决改为去重。候选↔读数的**逐条**对应关系随之不再是公共
/// 契约（左列不需要它；钉读音按**字符串**传动，不靠下标）。
#[test]
fn readings_are_deduped_in_first_appearance_order() {
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64"); // 你/尼（ni，精确档在前）与 你好（ni'hao）都命中
    let cands: Vec<String> = e.candidates(8).into_iter().map(|c| c.text).collect();
    assert!(cands.len() >= 3, "前置：夹具 64 应有多条候选（{cands:?}）");
    let readings = e.candidate_readings();
    assert_eq!(
        readings,
        vec!["ni".to_string(), "ni'hao".to_string()],
        "去重保序：ni 只出一行（你/尼 合流），ni'hao 随后 —— 不是逐候选的 ni ni ni'hao"
    );
    assert!(
        readings.len() < cands.len(),
        "去重后行数必然少于候选数（本夹具 3 候选 → 2 读数）"
    );
    assert_eq!(
        readings[0],
        e.display_buffer(),
        "首条读数 = display_buffer（同一对缓存的两个视图）"
    );
    // 钉住一个读音 ⇒ 候选收窄到一族 ⇒ 读数只剩那一行（左列单行高亮，不再数行同亮）。
    // 钉 `ni'hao`（= 左列里「你好」那一行）：你/尼 的 ni 档被收窄掉，只剩 ni'hao。
    e.set_pinned_reading(Some("ni'hao".into()));
    let _ = e.candidates(8);
    assert_eq!(
        e.candidate_readings(),
        vec!["ni'hao".to_string()],
        "钉住后读数只剩被钉读音那一族（单行）"
    );
    e.set_pinned_reading(None);
    // 缓冲一变（退格）⇒ 读数立刻作废（**不**再取候选：作废靠缓冲绑存，不靠调用次序）
    e.backspace();
    assert!(
        e.candidate_readings().is_empty(),
        "退格后读数立刻作废（下次 candidates() 才重算）"
    );
}

/// 左列读数**短→长**（2026-10-12 晚用户裁决「输入的拼音，在左侧排列从短到长」）。
///
/// 怎么造出「名次序 ≠ 长度序」的夹具：先用一次选词把长词 `号码`（键 `haoma`，5 键）
/// 学进用户词，下次取候选它靠 **user_boost** 压过精确档（引擎既有行为：用户词 +
/// boost > 精确加成 boost/2 > 静态词频，见 `rank_score`）。于是名次序 = 号码(长)、
/// 嘎(短、精确)、好、干 —— **长读音排在最前**，名次序照抄的实现必红。
#[test]
fn readings_are_sorted_short_to_long_and_stable() {
    let mut e = Engine::new(
        dict(&[
            ("hao", "好", 9000),
            ("gan", "干", 8000),
            ("ga", "嘎", 100),
            ("haoma", "号码", 7000),
        ]),
        no_symbols(),
        true, // 学习开：下面要先学一次 号码
    );
    e.set_scheme(Scheme::Nine);
    // 学前置：42662 = haoma，选首位（整串精确命中）——把它记进学习器
    type_str(&mut e, "42662");
    let first = e
        .candidates(8)
        .into_iter()
        .map(|c| c.text)
        .collect::<Vec<_>>();
    assert_eq!(
        first.first().map(String::as_str),
        Some("号码"),
        "前置：42662 首位是号码"
    );
    let _ = e.select(0);

    type_str(&mut e, "42"); // 4=ghi 2=abc：ha* / ga* 一族
    let cands: Vec<String> = e.candidates(8).into_iter().map(|c| c.text).collect();
    assert_eq!(
        cands.first().map(String::as_str),
        Some("号码"),
        "前置：学过的长词现在排最前（否则这条测不出排序；{cands:?}）"
    );
    let readings = e.candidate_readings();
    assert_eq!(
        readings.first().map(String::as_str),
        Some("ga"),
        "读数首位必须是**最短**那条（名次序里它在长词之后）：{readings:?}"
    );
    let lens: Vec<usize> = readings.iter().map(|r| r.chars().count()).collect();
    assert!(
        lens.windows(2).all(|w| w[0] <= w[1]),
        "读数必须按长度不降序（同长度保持名次序 —— 稳定）：{readings:?} / {lens:?}"
    );
    assert_eq!(readings.len(), 4, "四条读音各出一行：{readings:?}");
}

/// 钉读音（9 键左列点一行拼音）：候选**收窄**到该读音一族、**不上屏**；解除回全量；
/// 缓冲一变自动作废（配对语义：同一缓冲内有效）。
#[test]
fn pinned_reading_narrows_candidates_without_committing() {
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64"); // ni 与 nihao 都命中
    let all: Vec<String> = e.candidates(8).into_iter().map(|c| c.text).collect();
    assert!(all.len() >= 2, "前置：64 有多条候选（{all:?}）");

    // 钉「你好」那行的读音（带分词的原串 —— 引擎归一为去 `'` 的串）
    e.set_pinned_reading(Some("ni'hao".into()));
    let pinned: Vec<String> = e.candidates(8).into_iter().map(|c| c.text).collect();
    assert_eq!(pinned, vec!["你好".to_string()], "收窄到 nihao 一族");
    assert_eq!(e.buffer(), "64", "钉读音**不上屏**、不动缓冲");

    // 解除 → 回全量
    e.set_pinned_reading(None);
    let back: Vec<String> = e.candidates(8).into_iter().map(|c| c.text).collect();
    assert_eq!(back, all, "解除后回全量列表");

    // 缓冲一变即作废（不用谁去清）：退格后含非 nihao 族
    e.set_pinned_reading(Some("ni'hao".into()));
    e.backspace(); // "6"
    let after: Vec<String> = e.candidates(8).into_iter().map(|c| c.text).collect();
    assert!(after.contains(&"你".to_string()), "退格后钉失效");
    // 配对语义的边界（如实钉住）：缓冲回到 "64"，引擎层的钉**会复活** ——
    // 应用侧在缓冲变化时显式 `pinReading("")` 清（见 EngineController.refresh 的看守）。
    type_str(&mut e, "4");
    let resurrected: Vec<String> = e.candidates(8).into_iter().map(|c| c.text).collect();
    assert_eq!(
        resurrected,
        vec!["你好".to_string()],
        "配对语义：同缓冲内钉仍在"
    );
}
