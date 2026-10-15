// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **候选三档 · 真库组**（2026-10-14 用户裁决；夹具组在 `engine-core/tests/candidate_tiers.rs`）。
//!
//! 为什么真库组落在 `engine-data` 而不是 `engine-core`：读 `.opid` 文件的能力在
//! `engine-data`（`engine-core` **不许有 IO**，也不依赖它 —— 依赖方向反着），
//! 而本组测的正是「真库数据下这三档长什么样」—— 夹具测不出生僻字海（那正是这次
//! 报障的成因：`ni` 一族在真库里有 580 个单字、1195 个词）。引擎侧的档位规则与
//! 全部理由见 `crates/engine-core/src/candidates_tiers.rs` 头注。
//!
//! 真库是**入库资产**（`android/app/src/main/assets/luna.opid`）—— 缺文件直接红，
//! 不静默跳过。

use engine_core::Engine;
use engine_core::Scheme;
use engine_core::composer::Mode;
use engine_core::symbols::SymbolEngine;

fn type_str(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

fn texts(e: &Engine, limit: usize) -> Vec<String> {
    e.candidates(limit).into_iter().map(|c| c.text).collect()
}

/// 真库（luna.opid）：用户报障的那条输入 —— `ni`。
///
/// **2026-10-15 用户裁决「字全出完再出词」后改写**（原先钉的是 10-14 的「常用单字 5 槽」
/// 形态：你好 在 26 位）。现在：**所有**拼音精确匹配的单字排在一切词组之前 ⇒
/// `ni` 的精确字有 **182** 个（含生僻），词组从第 182 位起。
/// ⚠️ **这是裁决的代价、不是回归**：单音节下常用词组要翻页才到（多音节输入不受影响 ——
/// 打全拼 `nihao` 没有精确单字 ⇒ 词组直接顶上，见本文件最后一条）。门禁如实钉住它。
#[test]
fn real_dict_ni_letters_words_and_associations_are_all_reachable() {
    let mut e = real_engine();
    type_str(&mut e, "ni");
    let got = texts(&e, 400);
    let pos = |w: &str| got.iter().position(|t| t == w);

    // ① 精确单字区：首位是 你，且前 8 位**全**是单字（10-15 前是只给 5 槽）。
    assert_eq!(got[0], "你");
    for t in &got[..8] {
        assert_eq!(t.chars().count(), 1, "精确单字区应是单字：{t:?}");
    }
    // ② 单字区到 182 为止（实测；口径 = 「第一个多字候选取位」，别写死具体字）。
    let first_word = got
        .iter()
        .position(|t| t.chars().count() > 1)
        .expect("ni 必有词组候选");
    assert!(
        (100..400).contains(&first_word),
        "精确单字区长度应在百量级（实测 182）：{first_word}"
    );
    // ③ 词组区：与用户设置无关地可达（不钉具体名次 —— 词条随词库数据漂）。
    let nihao = pos("你好").expect("你好 应可达（400 内）");
    assert!(nihao > first_word - 1, "词组应在单字区之后：{nihao}");
    // ④ 拼音延伸的单字（联想，如 您/年/娘）在**所有**精确单字与词组之后。
    if let Some(nin) = pos("您") {
        assert!(nin > first_word, "联想（您）应在词组之后：{nin}");
    }
}

/// 真库：单音节另一条最常见的输入 `wo`（词组一族 `我们 / 我的 / 我国`）。
#[test]
fn real_dict_wo_puts_common_words_in_reach() {
    let mut e = real_engine();
    type_str(&mut e, "wo");
    let got = texts(&e, 400);
    let pos = |w: &str| {
        got.iter()
            .position(|t| t == w)
            .unwrap_or_else(|| panic!("候选里没有 {w}（前 400 位）：{:?}", &got[..20]))
    };
    assert_eq!(got[0], "我");
    for t in &got[..8] {
        assert_eq!(t.chars().count(), 1, "精确单字区应是单字：{t:?}");
    }
    // 词组在单字区之后（2026-10-15 裁决「字全出完再出词」；`wo` 一族的精确字实测 101 个）。
    let first_word = got
        .iter()
        .position(|t| t.chars().count() > 1)
        .expect("wo 必有词组");
    assert!(
        (40..400).contains(&first_word),
        "精确单字区长度应在百量级（实测 101）：{first_word}"
    );
    assert!(pos("我的") >= first_word, "词组应在单字区之后");
    assert!(pos("我们") >= first_word, "词组应在单字区之后");
}

/// 真库：多音节输入**行为不变**（它们本来就以词为主，三档不该把它们改坏）。
#[test]
fn real_dict_multi_syllable_still_leads_with_the_word() {
    let mut e = real_engine();
    type_str(&mut e, "nihao");
    let got = texts(&e, 8);
    assert_eq!(got[0], "你好", "{got:?}");
    assert_eq!(got[1], "拟好", "精确同音词仍在第二位（按分）：{got:?}");
}

/// 真库：**9 键宽松兜底档的位置**（用户裁决 2026-10-16）—— 尾巴候选让位给严格解读：
/// 排在**词组之后、联想单字之前**；`你` 仍是首位（尾巴不许顶掉报障串的首选）。
///
/// 报障串 `6444444` = `64`（ni/mi 一族）+ 尾巴 `44444`（g 一族）。实测（2026-10-16）：
/// 严格族 2418 条（精确单字 386 + 词组/联想），尾巴 12 条在第 1634 位、联想紧随其后；
/// 位置随词库与打分漂 ⇒ **只钉相对次序**。
#[test]
fn real_dict_nine_key_tail_sits_between_words_and_associations() {
    let mut e = real_engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "6444444");
    let got = texts(&e, 4096);
    let pos = |w: &str| {
        got.iter()
            .position(|t| t == w)
            .unwrap_or_else(|| panic!("候选里没有 {w}：{:?}", &got[..20]))
    };
    // ① 尾巴不许顶掉严格解读的首选（2026-10-16 裁决的核心理由）。
    assert_eq!(got[0], "你", "{:?}", &got[..20]);
    // ② 尾巴在词组之后（你好 是词组的头几条）。
    assert!(
        pos("个") > pos("你好"),
        "尾巴应在词组之后：个@{} 你好@{}",
        pos("个"),
        pos("你好")
    );
    // ③ 尾巴在联想单字之前（您 = nin，拼音延伸 3 字母 ⇒ 联想档）。
    assert!(
        pos("个") < pos("您"),
        "尾巴应在联想之前：个@{} 您@{}",
        pos("个"),
        pos("您")
    );
}

/// 真库引擎（入库资产 `android/app/src/main/assets/luna.opid`）。
/// 缺文件**直接红**：本文件后半段的价值全在真库数据上，静默跳过等于没有门禁。
fn real_engine() -> Engine {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../android/app/src/main/assets/luna.opid");
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "读不到真库 {}（{e}）—— 它是入库资产，不该缺；缺了这条门禁就没在测东西",
            path.display()
        )
    });
    let dict = engine_data::load_bytes(bytes).expect("luna.opid 解析");
    let mut e = Engine::new(Box::new(dict), SymbolEngine::builtin(), true);
    e.switch_mode(Mode::Pinyin);
    e
}
