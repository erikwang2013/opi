// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **候选三档**门禁（2026-10-14 用户裁决：「候选词里优先给符合拼音的候选字，然后是词组，
//! 最后是联想的，联想的字、词组按照拼音往后排」）。
//!
//! 实现与全部理由（含三次实测翻车的记录）在 `crates/engine-core/src/candidates_tiers.rs`
//! 头注 —— 本文件只钉**可观察次序**，不重复那段推理。
//!
//! 两条路都走 `Engine`（不是直接调 `rank_and_pick`）：门禁必须与被测调用链同路，
//! 否则「排好序」这件事在候选层成立、在产品层不成立（本仓有过这种门禁，见
//! `ranking_gate_bypasses_ranking_code` 那条教训）。
//!
//! 本文件 = **夹具组**（确定性替身词库）；**真库组**（生僻字海只有在真数据上才出现）
//! 在 `crates/engine-data/tests/candidate_tiers_real.rs` —— 那边才有读 `.opid` 的能力
//! （`engine-core` 不许有 IO，依赖方向也反着）。

use engine_core::dictionary::InMemoryDictionary;
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Scheme};

fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

/// 夹具：7 个精确单字（> 5 个槽位）+ 拼音延伸的字与词 + 一个 3 字母的词。
///
/// 词频刻意让「延伸」比「精确」高（`您` 9500 > `你` 9000）：档位判据是**拼音结构与
/// 长度**，不是分数 —— 分数排序是旧行为，正是本裁决要改掉的那条。
fn fixture() -> Engine {
    let mut d = InMemoryDictionary::new();
    for (w, f) in [
        ("你", 9000u32),
        ("泥", 8000),
        ("拟", 7000),
        ("呢", 6000),
        ("腻", 5000),
        ("逆", 4000), // 第 6 个精确单字 ⇒ 进不了 5 槽 ⇒ 沉底
        ("祢", 3000), // 同上
    ] {
        d.insert("ni", w, f);
    }
    d.insert("niu", "牛牛", 5500); // 3 字母的**词**（与 `您` 同长度 ⇒ 钉「同级先词后字」）
    d.insert("nin", "您", 9500); // 3 字母的**字**
    d.insert("nian", "年", 8500); // 4 字母的字
    d.insert("nihao", "你好", 8000); // 5 字母的词
    d.insert("nimen", "你们", 7000);
    Engine::new(Box::new(d), no_symbols(), true)
}

fn type_str(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

fn texts(e: &Engine, limit: usize) -> Vec<String> {
    e.candidates(limit).into_iter().map(|c| c.text).collect()
}

/// 夹具：三档的完整次序一次钉死（1 档 5 槽 → 联想按拼音长度、同级先词后字 → 沉底）。
#[test]
fn fixture_pins_the_three_tiers() {
    let mut e = fixture();
    type_str(&mut e, "ni");
    assert_eq!(
        texts(&e, 12),
        vec![
            "你", "泥", "拟", "呢", "腻", "逆", "祢", "牛牛", "你好", "你们", "您", "年"
        ],
        "1 档 = **全部**精确单字（不封顶；用户 2026-10-15 裁决「字全出完再出词」）⇒ 逆 祢 也在这档；\
         2 档 = 词组按拼音长度升序（牛牛 3 < 你好/你们 5，同长按分）；3 档 = 拼音延伸的单字（您 3、年 4）"
    );
}

/// 学过的候选**不受三档约束**（第 0 档，按分排在最前）—— README 缘起 #3：
/// 「你最常用的那个词」必须还能排到首位，否则学习这个功能等于作废。
#[test]
fn learned_candidate_outranks_the_tiers() {
    let mut e = fixture();
    type_str(&mut e, "ni");
    let idx = texts(&e, 12).iter().position(|t| t == "你好").unwrap();
    assert_eq!(e.select(idx), "你好", "前置：先把 你好 学进用户词");

    type_str(&mut e, "ni");
    assert_eq!(
        texts(&e, 3)[0],
        "你好",
        "选过一次的词必须回到首位（第 0 档），不被「字先于词」压住"
    );
}

/// 码类方案（笔画 / 五笔）**不分档**：缓冲是编码不是拼音，`RankOptions::tiers` 关着。
/// 判据：旧次序「精确单字全出完才轮到延伸」在无码表回退路上仍然成立。
#[test]
fn code_schemes_keep_the_old_order() {
    let mut e = fixture();
    e.set_scheme(Scheme::Wubi); // 没装码表 ⇒ 走 plain 回退路（tiers=false）
    type_str(&mut e, "ni");
    let got = texts(&e, 12);
    assert!(
        got.iter().position(|t| t == "你好").unwrap() > got.iter().position(|t| t == "祢").unwrap(),
        "五笔路不许分档：延伸的 你好 仍排在**全部**精确单字之后（旧次序）：{got:?}"
    );
}

/// 9/14 键与全键盘**同一套档位**（`candidates_scheme.rs` 头注那条「两套键盘的候选行为
/// 一致」的契约）：同一夹具、同一输入，经按键类展开（6=n 4=i）应给出同一档次序。
#[test]
fn nine_key_path_tiers_identically() {
    let mut e = fixture();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64"); // n=6, i=4
    assert_eq!(
        texts(&e, 12),
        vec![
            "你", "泥", "拟", "呢", "腻", "逆", "祢", "牛牛", "你好", "你们", "您", "年"
        ],
        "9 键与全键盘逐条同序"
    );
}
