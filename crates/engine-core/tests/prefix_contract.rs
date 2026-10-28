// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **有限窗口 = 全量列表的逐条前缀**（architect 2026-10-28 护栏）。
//!
//! Android 显示取 `candidates(512)`、全局 select 用 `candidates(MAX)`；只有两者
//! 逐条同前缀时，页内下标 `page*PAGE_SIZE + k` 才安全 —— 否则「看到 A、上屏 B」。
//! 现有 `candidates_finite` 的用例只验单键快速窗口本身，没有跨 limit 断言；本门禁
//! 覆盖 26/9/14 键 × 有/无学习词 × k=8/64/512。非精确输入的分词回退档
//! （`segment_fallback` 的 64 截断）另由 `fallback_inputs_are_prefixes_too` 覆盖。
//!
//! 用 InMemoryDictionary（1700 条同音词，分 `ni`/`niha`/`nihao` 三族）：单字的静态
//! 词频**高于**词组，但分档把词组排在延伸单字之前 ⇒ 最终次序 ≠ 词频序。两族「非精确」
//! 条目都多于 512 条：`query_prefix_top` 只截断 rest 那一组，若谁把池子按 limit 先截
//! （把 `prefix_cap` 写成 `limit`），超过 512 的 rest 尾部会掉光 —— 学习词 `词599` 正
//! 落在尾部，全量里它是第 0 档，截断后当场消失。`niha` 族则是**精确**档：非精确输入
//! 会走 `segment_fallback`（`FALLBACK_LIMIT=64`），盖住 512 窗口，测不到主路径。
//! `query_prefix_top` 与 `scheme_entries` 的默认实现正是真实词库覆写的语义基准；
//! 真库（luna.opid）的 26/9 键前缀另有 `crates/engine-data/tests/prefix_contract.rs`。

use engine_core::dictionary::InMemoryDictionary;
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Mode, Scheme};

/// 有限窗口的上界（Android 的显示取数）。
const WINDOW_MAX: usize = 512;
/// `ni`：精确档，500 词组 + 100 单字。
const WORDS: usize = 500;
/// `niha`：精确档（9/14 键的 4 键输入展开到它），300 词组 + 100 单字。
const MID_WORDS: usize = 300;
/// `nihao`：非精确档，600 词组 + 100 单字 —— rest > 512，池子截断才可观测。
const LONG_WORDS: usize = 600;
const CHARS: usize = 100;

fn engine(with_learned: bool) -> Engine {
    let mut d = InMemoryDictionary::new();
    for i in 0..WORDS {
        d.insert("ni", &format!("候选{i:03}"), (10_000 - i) as u32);
    }
    for i in 0..MID_WORDS {
        d.insert("niha", &format!("字{i:03}"), (9_500 - i) as u32);
    }
    for i in 0..LONG_WORDS {
        d.insert("nihao", &format!("词{i:03}"), (9_000 - i) as u32);
    }
    // 单字：词频**高于全部词组**（20000 档），但非精确输入时分档仍把词组排在
    // 延伸单字之前 ⇒ 「先按分数截断再分档」与「先分档再截断」的首 8 条不同。
    for i in 0..CHARS {
        let a = char::from_u32(0x4E00 + i as u32).expect("CJK 区码点");
        let b = char::from_u32(0x5E00 + i as u32).expect("CJK 区码点");
        let c = char::from_u32(0x6E00 + i as u32).expect("CJK 区码点");
        d.insert("ni", &a.to_string(), 20_000 - i as u32);
        d.insert("niha", &b.to_string(), 20_000 - i as u32);
        d.insert("nihao", &c.to_string(), 20_000 - i as u32);
    }
    let mut e = Engine::new(Box::new(d), SymbolEngine::new(Vec::new(), Vec::new()), true);
    e.switch_mode(Mode::Pinyin);
    if with_learned {
        // 学习词改变名次：有限窗口必须在**同一张排好序的表**上截，而不是先按
        // limit 预筛。`词599` 静态词频垫底、却在 rest 尾部 —— 池子按 512 截断时
        // 它第一个掉，全量里它又因学习档排第 0 位，前缀当场分叉。
        e.import_user_words(
            r#"{"version":1,"words":[
                {"text":"候选499","freq":9},{"text":"词599","freq":5},
                {"text":"词298","freq":3}
            ]}"#,
        )
        .unwrap();
    }
    e
}

fn texts(e: &mut Engine, scheme: Scheme, input: &str, limit: usize) -> Vec<String> {
    e.set_scheme(scheme); // 换方案自带清缓冲
    for ch in input.chars() {
        e.input_key(ch);
    }
    e.candidates(limit).into_iter().map(|c| c.text).collect()
}

#[test]
fn finite_candidates_are_prefixes_across_schemes_and_learning() {
    // 26 键：单键走 candidates_finite 的快速窗口；"ni" 是精确档，"niha" 是精确档
    // 且 rest（"nihao" 族）超过 512 条。
    // 9 键：6 = {m,n,o}，64 = {m,n,o}{g,h,i}，6442 展开出 "niha"。
    // 14 键：b = {b,n}，u = {u,i}，g = {g,h}，a = {a,s}（"nihao" = buga，"niha" = buga 的前三键组合之一）。
    let cases: [(Scheme, &[&str]); 3] = [
        (Scheme::Full, &["n", "ni", "niha"]),
        (Scheme::Nine, &["6", "64", "6442"]),
        (Scheme::Fourteen, &["b", "bu", "buga"]),
    ];
    for with_learned in [false, true] {
        let mut e = engine(with_learned);
        for (scheme, inputs) in cases {
            for input in inputs {
                let full = texts(&mut e, scheme, input, usize::MAX);
                assert!(
                    full.len() >= WINDOW_MAX,
                    "夹具前提：{scheme:?} {input} 全量只有 {} 条，撑不起 {WINDOW_MAX} 窗口",
                    full.len()
                );
                for limit in [8usize, 64, WINDOW_MAX] {
                    let got = texts(&mut e, scheme, input, limit);
                    assert_eq!(got.len(), limit, "长度：{scheme:?} {input} limit={limit}");
                    assert_eq!(
                        got,
                        full[..limit],
                        "有限窗口不是全量前缀：{scheme:?} {input} limit={limit} \
                         learner={with_learned}"
                    );
                }
            }
        }
    }
}

/// 分词回退档的夹具：首音节只出现在**更长**的键下（`niya` / `zhongya` / …）。
/// 这样整串纠错变体（`nihaob` 去掉 `b` → `nihao`、`xianz` 去掉 `z` → `xian`）
/// 都不是精确命中 —— 否则默认开着的纠错档会先接管，回退档根本不执行。真库
/// `xianz` 实测 638 条 > `FALLBACK_LIMIT=64` ⇒ 那个样例走的是主/纠错路径，
/// 回退档的 64 截断在真库上看不到。
fn fallback_engine() -> Engine {
    let mut d = InMemoryDictionary::new();
    // ni/xian 超过 64（截断可观测）；zhong/sheng 不足 64（窗口按全量长度截）。
    for (key, n) in [
        ("niya", 100usize),
        ("zhongya", 3),
        ("shengya", 7),
        ("xianya", 100),
    ] {
        for i in 0..n {
            d.insert(key, &format!("{key}词{i:03}"), (9_000 - i) as u32);
        }
    }
    // 隔离词只挂在第二个音节下：回退档只取**首音节**，它绝不出现；若回退没接管
    // （逐音节汤兜底），ni/hao/b 的汤会把它带进候选。
    d.insert("hao", "隔离词", 10_000);
    let mut e = Engine::new(Box::new(d), SymbolEngine::new(Vec::new(), Vec::new()), true);
    e.switch_mode(Mode::Pinyin);
    e
}

#[test]
fn fallback_inputs_are_prefixes_too() {
    // 非精确输入走 `segment_fallback`：候选换成**第一个音节**的整档，取回上限是
    // `limit.min(FALLBACK_LIMIT=64)` —— 512/MAX 两条腿都落到 64。有限窗口仍必须是
    // 全量前缀（Android 的页内/全局下标靠它）；k 大于全量长度时按实际长度截。
    let mut e = fallback_engine();
    let mut capped = 0;
    for input in ["nihaob", "zhongguor", "nihaoa", "shengrikuai", "xianz"] {
        let full = texts(&mut e, Scheme::Full, input, usize::MAX);
        assert!(
            !full.is_empty(),
            "夹具前提：{input} 必须走分词回退（首音节有前缀命中）"
        );
        assert!(
            !full.iter().any(|t| t == "隔离词"),
            "{input}：第二音节的档混进来了 —— 分词回退没接管"
        );
        if full.len() == 64 {
            capped += 1;
        }
        for limit in [8usize, 64, WINDOW_MAX] {
            let got = texts(&mut e, Scheme::Full, input, limit);
            let want = &full[..limit.min(full.len())];
            assert_eq!(got, want, "回退档不是全量前缀：{input} limit={limit}");
        }
    }
    assert!(
        capped > 0,
        "夹具前提：至少一条样例须撞到 FALLBACK_LIMIT=64，否则截断没被测到"
    );
}
