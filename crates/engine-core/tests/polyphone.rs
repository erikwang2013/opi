// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **多音字补读音**纠偏层门禁（2026-10-15 用户报障「为何单独打『么』候选词没有？」）。
//!
//! 机制与语义边界见 `src/polyphone.rs` 头注；本文件钉**层本身的力学**（用替身词库，
//! 真库的落位在 `crates/engine-data/tests/polyphone_real.rs`）：
//! 补的条目按精确匹配算、参与候选三档、去重不重复。

use engine_core::Engine;
use engine_core::dictionary::InMemoryDictionary;
use engine_core::symbols::SymbolEngine;

/// 量纲锚点：纠偏层的表是按**真实词库量纲**建的（十亿量级），本层只对同档词库生效
/// （`polyphone::applies`）。夹具要测层本身，就得把词库的量纲垫到那一档 ——
/// 这条 `锚` 条目就是干这个的（它自己只在 `miao` 输入下露面，不干扰下面各条）。
const ANCHOR: (&str, &str, u32) = ("miao", "锚", 3_999_000_000);

fn engine(entries: &[(&str, &str, u32)]) -> Engine {
    let mut d = InMemoryDictionary::new();
    for (k, w, f) in entries {
        d.insert(k, w, *f);
    }
    d.insert(ANCHOR.0, ANCHOR.1, ANCHOR.2);
    Engine::new(Box::new(d), SymbolEngine::new(Vec::new(), Vec::new()), true)
}

fn texts(e: &Engine, limit: usize) -> Vec<String> {
    e.candidates(limit).into_iter().map(|c| c.text).collect()
}

/// 同量纲词库里缺这个字 + 打 `mo` ⇒ 纠偏层把 `么` 补出来（正说明「缺口在词库、
/// 不在排序」），且按词频排在补出来的那几条之首。
///
/// ⚠️ 表里同一音节不止一个字（`mo` 还有 `嘿`（嘿然）· `無`（南無）· `萬`（萬俟）——
/// 都是现代词典口径的真读音，只是词库里没挂这个键）。断言按**相对次序**写，
/// 不写死整份表（表是生成物，Unihan 改版会动它）。
#[test]
fn patch_fills_a_reading_the_dictionary_lacks() {
    let mut e = engine(&[]);
    for c in "mo".chars() {
        e.input_key(c);
    }
    let got = texts(&e, 8);
    assert_eq!(
        got[0], "么",
        "缺这个字的库里也该补出 么，且按词频居首：{got:?}"
    );
    assert!(!got.is_empty());
}

/// **量纲门**（2026-10-15 加）：不同量纲的词库（夹具那类几千量级）**不适用**本层 ——
/// 表里的词频是参考库的，量纲差一个数量级以上时「这个词库缺不缺这个读音」无从判断，
/// 补进去只是噪音。判据与由来见 `polyphone::applies`。
#[test]
fn small_scale_dictionaries_are_left_alone() {
    let mut d = InMemoryDictionary::new();
    d.insert("mo", "模", 3900);
    let mut e = Engine::new(Box::new(d), SymbolEngine::new(Vec::new(), Vec::new()), true);
    for c in "mo".chars() {
        e.input_key(c);
    }
    assert_eq!(texts(&e, 8), vec!["模"], "小量纲库里一个补读音都不该出现");
}

/// 词库有别的同音字时，补的条目**与它们同档、按词频排**（不另开一档、也不沉底）。
/// 夹具把 `模` 的词频设得比 `么` 补进来的那条（3999878020，取自语料）更高 ⇒ 模在前、
/// 么紧随 —— 钉的是「同一把尺子」，不是某个字必胜。
#[test]
fn patched_entry_competes_with_dict_entries_in_the_same_tier() {
    let mut e = engine(&[("mo", "模", 4_200_000_000), ("mo", "摸", 1_000_000_000)]);
    for c in "mo".chars() {
        e.input_key(c);
    }
    let got = texts(&e, 8);
    assert_eq!(got[0], "模", "词频更高的词库条目在前：{got:?}");
    assert_eq!(got[1], "么", "补条目紧随其后（同档按词频）: {got:?}");
    assert!(got.contains(&"摸".to_string()));
}

/// 去重：词库**已有**这个读音的字不会被补成第二条（层内不做差集，靠候选层的 seen）。
#[test]
fn patch_does_not_duplicate_dict_entries() {
    // `么` 已在 `mo` 下 ⇒ 纠偏层那条（真表里 `(么, mo)` 是为缺 `mo` 的词库准备的）
    // 若被无脑加入，会出现两条 `么`。
    let mut e = engine(&[("mo", "么", 2_000_000_000)]);
    for c in "mo".chars() {
        e.input_key(c);
    }
    let got = texts(&e, 8);
    assert_eq!(
        got.iter().filter(|t| *t == "么").count(),
        1,
        "不得重复：{got:?}"
    );
}

/// 表里没有的音节：一个字都不多加（层是「缺口补丁」，不是第二张词库）。
#[test]
fn readings_absent_from_the_table_add_nothing() {
    let mut e = engine(&[("hao", "好", 2_000_000_000)]);
    for c in "hao".chars() {
        e.input_key(c);
    }
    assert_eq!(texts(&e, 8), vec!["好"]);
}
