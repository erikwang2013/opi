// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 模糊拼音的**引擎级**门禁（走 `Engine::candidates`，不绕到 `dict.query` 取排序：
//! 排序规则全长在 `rank_and_pick` 那一层，绕过它缺陷就不可见 ——
//! memory: ranking-gate-bypasses-ranking-code）。
//!
//! 词频一律取真实值（`data/raw/trad_hanzi.tsv`）——「模糊命中抢精确首位」这类问题
//! 只在真实词频的量级差下才是真问题，合成的小数字会把门槛调没了。

use engine_core::Engine;
use engine_core::dictionary::{Dictionary, InMemoryDictionary};
use engine_core::fuzzy::{MAX_VARIANTS, input_variants};
use engine_core::symbols::SymbolEngine;
use std::collections::HashSet;

fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

fn engine(d: InMemoryDictionary) -> Engine {
    Engine::new(Box::new(d), no_symbols(), false)
}

/// 敲入拼音并取候选，`limit` 条。
fn cands_n(e: &mut Engine, input: &str, limit: usize) -> Vec<String> {
    e.clear();
    for ch in input.chars() {
        e.input_key(ch);
    }
    e.candidates(limit).into_iter().map(|c| c.text).collect()
}

/// 取**全量**候选（`usize::MAX`：本文件看的是排序与分档，不是截断）。
fn cands(e: &mut Engine, input: &str) -> Vec<String> {
    cands_n(e, input, usize::MAX)
}

fn pos(list: &[String], word: &str) -> usize {
    list.iter()
        .position(|t| t == word)
        .unwrap_or_else(|| panic!("「{word}」不在候选里：{list:?}"))
}

// ---------- 模糊生效 ----------

/// 打 `zong` 要能出「中」（zh↔z 变体 `zhong`）。且**精确的「总」仍在「中」之前** ——
/// 「中」的静态词频（3996349672）比「总」（3986560156）**更高**，这条断言因此不是
/// 词频的巧合，而是「精确在前、模糊在后」这条分档规则的直接检验。
#[test]
fn zong_reaches_zhong_but_after_the_exact_zong() {
    let mut d = InMemoryDictionary::new();
    d.insert("zong", "总", 3_986_560_156);
    d.insert("zhong", "中", 3_996_349_672);
    let mut e = engine(d);
    let got = cands(&mut e, "zong");
    assert!(pos(&got, "总") < pos(&got, "中"), "{got:?}");
    assert!(pos(&got, "中") < got.len(), "{got:?}");
}

/// 多音节也要按音节替换：`zongguo` → `zhongguo` 才命中「中国」。
/// 整串无词条时先走逐音节回退（总/国），模糊结果**追加在它们之后**。
#[test]
fn zongguo_reaches_zhongguo_after_the_fallback() {
    let mut d = InMemoryDictionary::new();
    d.insert("zong", "总", 3_986_560_156);
    d.insert("guo", "国", 3_997_884_469);
    d.insert("zhongguo", "中国", 3_996_000_000);
    let mut e = engine(d);
    let got = cands(&mut e, "zongguo");
    assert!(
        got.contains(&"中国".into()),
        "打 zongguo 必须能到「中国」：{got:?}"
    );
    assert!(
        pos(&got, "总") < pos(&got, "中国") && pos(&got, "国") < pos(&got, "中国"),
        "「中国」必须排在逐音节回退的精确候选之后：{got:?}"
    );
}

/// 多音节输入的模糊命中要能进**默认 8 槽候选栏** —— 「存在」不够，用户看得见才算数。
/// 逐音节回退给每音节 3 个精确字，模糊结果接在精确档之后：2 音节 = 6 精确 + 模糊，
/// 第 7 槽就该是「中」。这条同时钉住「模糊档不吃 exact_bonus」（吃了就会插到精确档前面）。
#[test]
fn fallback_fuzzy_is_visible_in_the_default_bar() {
    let mut d = InMemoryDictionary::new();
    d.insert("zong", "总", 3_986_560_156);
    d.insert("zong", "宗", 3_985_000_000);
    d.insert("zong", "综", 3_980_000_000);
    d.insert("guo", "国", 3_997_884_469);
    d.insert("guo", "过", 3_990_000_000);
    d.insert("guo", "果", 3_980_000_000);
    d.insert("zhong", "中", 3_996_349_672);
    let mut e = engine(d);
    let bar = cands_n(&mut e, "zongguo", 8);
    assert!(bar.contains(&"中".into()), "8 槽里没有模糊命中：{bar:?}");
    assert!(
        pos(&bar, "总") < pos(&bar, "中"),
        "模糊命中插到精确档前面了：{bar:?}"
    );
}

// ---------- ⚠️ 变体命中不得冒充精确匹配 ----------

/// 本文件最重要的一条。`n↔l` 是**保长**替换：`ni` 与 `li` 同为 2 字节，变体查询回来的
/// 「你」满足 `e.pinyin_len == input_len`，naive 实现会判它精确、吃满 `exact_bonus` ——
/// 于是打 `li` 的首位被模糊命中「你」顶掉（真实词频：你 3999128899 > 裏 3998755570）。
/// 精确档被模糊命中改写，`ranking_quality.rs` 的 NEAR（li→裡 前 3）随之只剩 0 余量。
#[test]
fn length_preserving_fuzzy_hit_never_wins_the_exact_slot() {
    let mut d = InMemoryDictionary::new();
    d.insert("li", "裏", 3_998_755_570);
    d.insert("li", "裡", 3_998_714_089);
    d.insert("ni", "你", 3_999_128_899);
    let mut e = engine(d);
    let got = cands(&mut e, "li");
    assert_eq!(got[0], "裏", "「li」首位必须是精确匹配的「裏」：{got:?}");
    assert_eq!(got[1], "裡", "精确档内部仍按词频排：{got:?}");
    assert!(
        pos(&got, "你") > pos(&got, "裡"),
        "模糊命中必须在精确候选之后：{got:?}"
    );
}

/// 变体查询的**前缀扩展**也会撞上等长：`shu` 的变体 `su` 会带回键 `suo`
/// （3 字节 == len("shu")），真实词频下 `suo` 组（3992160091）比「书」（3985025359）更高
/// —— 这正是 `ranking_quality.rs` STRICT `shu` 在 naive 实现下翻车的机制。
/// 这里用等价的小词库钉死它：**变体前缀下的任何命中一律非精确**。
#[test]
fn variant_prefix_extension_is_not_exact_either() {
    let mut d = InMemoryDictionary::new();
    d.insert("shu", "书", 3_985_025_359);
    d.insert("suo", "所", 3_992_160_091); // 变体 "su" 的前缀扩展，且等长
    let mut e = engine(d);
    let got = cands(&mut e, "shu");
    assert_eq!(got[0], "书", "「书」必须是首位：{got:?}");
    assert!(got.contains(&"所".into()), "模糊结果仍要可达：{got:?}");
}

/// 变体命中不得因**键长巧合**冒充精确匹配。`e.pinyin_len == input_len` 这条判定对
/// 变体查询同样成立：变体的**前缀扩展**只要键长恰好等于输入的字节数就命中 ——
/// 于是它在模糊档里吃满 `exact_bonus`，把真正高频的模糊命中挤下去。
///
/// 输入 `shu`（3 字节）、词库只有变体 `su` 的两组命中：
/// 速（键 `su`，2 字节）与 所（键 `suo`，3 字节，恰好等长）。
/// 模糊档内部必须按真实词频排（都非精确），不得让 `suo` 靠键长上位。
#[test]
fn fuzzy_tier_orders_by_frequency_not_by_key_length_coincidence() {
    let mut d = InMemoryDictionary::new();
    d.insert("su", "速", 3_992_000_000);
    d.insert("suo", "所", 3_000_000_000);
    let mut e = engine(d);
    let got = cands(&mut e, "shu");
    assert_eq!(
        got,
        vec!["速", "所"],
        "模糊档必须按真实词频排（键长巧合不得冒充精确）：{got:?}"
    );
}

// ---------- 只增不删：模糊只追加，不插队 ----------

/// 同一输入下，**精确路径可达的候选必须整体集中在开头**（= 开模糊后的列表以未开模糊的
/// 列表为前缀），且一个都不少。
///
/// 归属判定用 `dict.query_all` —— 它只回答「这个文本是不是精确路径给的」（**成员**），
/// 顺序断言完全落在引擎输出上。
#[test]
fn fuzzy_only_appends_after_the_exact_candidates() {
    let mut e = engine(real_freq_dict());
    let probe = real_freq_dict();
    for input in [
        "zong", "zhong", "li", "ni", "shu", "nai", "fa", "hao", "xian", "zongguo",
    ] {
        let exact: HashSet<String> = probe.query_all(input).into_iter().map(|x| x.word).collect();
        let got = cands(&mut e, input);
        let flags: Vec<bool> = got.iter().map(|t| exact.contains(t)).collect();
        let k = flags.iter().position(|f| !f).unwrap_or(flags.len());
        assert!(
            flags[k..].iter().all(|f| !f),
            "input={input}: 模糊命中插在精确候选之前：{got:?}"
        );
        for w in &exact {
            assert!(
                got.contains(w),
                "input={input}: 精确候选「{w}」被模糊挤掉了：{got:?}"
            );
        }
    }
}

/// 真实词频小词库（`data/raw/trad_hanzi.tsv`），覆盖 n↔l、f↔h、zh↔z、sh↔s、in↔ing
/// 五种变体与它们的精确对手。两次调用返回同一份内容（一次给引擎，一次留作成员判定）。
fn real_freq_dict() -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    for (py, w, f) in [
        ("ni", "你", 3_999_128_899u32),
        ("li", "裏", 3_998_755_570),
        ("li", "裡", 3_998_714_089),
        ("li", "里", 3_998_672_608),
        ("zong", "总", 3_986_560_156),
        ("zhong", "中", 3_996_349_672),
        ("guo", "国", 3_997_884_469),
        ("zhongguo", "中国", 3_996_000_000),
        ("shu", "书", 3_985_025_359),
        ("suo", "所", 3_992_160_091),
        ("lai", "来", 3_999_419_266),
        ("nai", "奶", 3_958_975_291),
        ("fa", "发", 3_996_640_039),
        ("ha", "哈", 3_944_249_536),
        ("hao", "好", 3_997_635_583),
        ("xian", "现", 3_995_188_204),
        ("xiang", "想", 3_995_976_343),
        ("xin", "心", 3_993_000_000),
        ("xing", "行", 3_998_000_000),
        ("song", "送", 3_974_986_957),
    ] {
        d.insert(py, w, f);
    }
    d
}

// ---------- 变体合法性 ----------

/// 变体必须是合法音节：`song` 套 `s→sh` 得 `shong`。词库里**故意**挂一条 `shong`
/// 行 —— 变体不过滤就会把它查出来（真机表现是拿非法串白查一次库）。
#[test]
fn illegal_variant_is_never_queried() {
    let mut d = InMemoryDictionary::new();
    d.insert("song", "送", 3_974_986_957);
    d.insert("shong", "泄漏", 9_000_000);
    let mut e = engine(d);
    let got = cands(&mut e, "song");
    assert_eq!(got, vec!["送"], "非法音节的变体被拿去查库了：{got:?}");
}

/// 上限在**引擎路径**上也成立：长输入不得让变体数爆掉。
/// `zhang` 每个音节 3 个变体（zhang/zang/zhan），8 音节 = 3^8 = 6561 的笛卡尔积。
#[test]
fn long_input_variants_stay_capped() {
    let long = "zhang".repeat(8);
    let vs = input_variants(&long);
    assert_eq!(vs.len(), MAX_VARIANTS, "{vs:?}");
    // 每个变体都由合法音节拼成（拼不出非法串去查库）
    for v in &vs {
        assert!(v.chars().all(|c| c.is_ascii_lowercase()), "{v}");
    }
    let mut e = engine(real_freq_dict());
    let got = cands(&mut e, &long);
    assert!(
        got.len() <= MAX_VARIANTS * 8,
        "候选表随变体爆了：{}",
        got.len()
    );
}
