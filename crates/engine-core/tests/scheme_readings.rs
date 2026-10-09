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

/// 逐条读数（9 键左列的数据源，2026-10-09 用户裁决「一列多个可滑动查看」）：
/// 与 `candidates(limit)` **同序等长**、同一次排名产出；首条 = `display_buffer()`
/// （同一对缓存的两个视图）；缓冲一变（退格/提交）⇒ 双双立刻为空。
#[test]
fn per_candidate_readings_align_with_candidates() {
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64"); // ni 与 nihao 都命中（键类前缀匹配）
    let cands: Vec<String> = e.candidates(8).into_iter().map(|c| c.text).collect();
    let readings = e.candidate_readings();
    assert_eq!(readings.len(), cands.len(), "逐条读数与候选同序等长");
    assert_eq!(
        readings[0],
        e.display_buffer(),
        "首条读数 = display_buffer（同一对缓存的两个视图）"
    );
    // 逐条对齐：你好 的读音带分词（ni'hao），你/尼 是 ni —— 不是把首条读一抄到底。
    let idx = cands
        .iter()
        .position(|c| c == "你好")
        .expect("夹具里 64 应命中 nihao⇒你好");
    assert_eq!(readings[idx], "ni'hao", "候选与读音逐条对齐");
    // 缓冲一变（退格）⇒ 逐条读数立刻作废
    e.backspace();
    assert!(
        e.candidate_readings().is_empty(),
        "退格后逐条读数立刻作废（下次 candidates() 才重算）"
    );
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
