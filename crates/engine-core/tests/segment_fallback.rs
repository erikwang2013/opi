// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **分词回退**（2026-10-10 用户裁决「输入拼音无匹配时要分词匹配」）的引擎级语义门禁。
//!
//! 语义定案（本文件逐条钉住）：
//! 1. **触发**：整串直接命中为空、整串模糊命中也为空（「更具体的匹配优先」）、缓冲不止
//!    一个音节 —— 三者全成立才回退；
//! 2. **候选**：从最长的音节对齐前缀往短试，第一个「每段都是完整音节 + 词库有命中」的
//!    前缀胜出，候选换成**它**的整档（递归走同一条精确/模糊/简拼分档，不另起一套排序）；
//! 3. **选中**：提交该候选文本、缓冲**只消耗那个前缀**、剩下的后缀留着接着编
//!    （`nihaob` 选「你好」→ 缓冲变 `b`）；提交文本/学习/清缓冲的收尾仍是 `select_from`
//!    那一处（四端自动一致）；
//! 4. **读数**：回退态下 `display_buffer()` 仍是**用户打的整串**（预编辑不缩水）、
//!    `candidate_readings()` 仍是空（那是 9/14 键专用，回退不覆盖 9/14）；
//! 5. **边界**：单音节无匹配维持原「无匹配」；前缀也无候选维持原「无匹配」；
//!    垃圾串（段不是完整音节：`qqq`/`zxcvb`）维持死 —— 「打英文」的逃生口不动；
//!    9/14 键不做回退（缓冲是数字串/紧簇，分词对它们没有意义）。
//!
//! ⚠️ **空格键有意例外**：`Engine::input_space` 是「整段上屏」（无候选才提交原始缓冲的
//! 逃生口）——回退态下它提交首候选并**整段清空**，不收后缀（钉子：
//! tests/adversarial_input.rs 的「提交过就必须清空缓冲」；另有
//! `tests/punctuation.rs` 的英文逃生口）。逐候选选中（点击 / 数字键）才是「选一个留一段」。
//!
//! 夹具是**内联词库 + 空符号表**（计数断言/收录断言不该被符号表掺料，见
//! memory「计数夹具要钉空符号表」）；真词库的验收在 `crates/opi-tools/tests/`。

use engine_core::dictionary::InMemoryDictionary;
use engine_core::router::{KeyAction, KeyRouter};
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Mode, Scheme};

/// 内联词库。`zhongguo` 可选：它是「整串模糊命中优先于分词回退」那条边界的对照组
/// （`zongguo` 的模糊变体 = `zhongguo`）。
fn dict(with_zhongguo: bool) -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    // 回退的目标档：打 `nihaob` 要能出「你好」
    d.insert("nihao", "你好", 9_000);
    d.insert("nihao", "你好啊", 8_000);
    d.insert("nihao", "利好", 100);
    // 逐音节汤的原料（今日的旧行为就是从这里拼出来的）
    d.insert("ni", "你", 7_000);
    d.insert("ni", "尼", 600);
    d.insert("hao", "好", 5_000);
    d.insert("hao", "号", 400);
    // 选中回退候选之后接着编的那一段
    d.insert("bao", "包", 3_000);
    // 模糊档：`zongguo` 的整串变体
    d.insert("zong", "总", 1_000);
    d.insert("guo", "国", 900);
    d.insert("zhong", "中", 9_999);
    if with_zhongguo {
        d.insert("zhongguo", "中国", 9_500);
    }
    d
}

fn engine(with_zhongguo: bool) -> Engine {
    Engine::new(
        Box::new(dict(with_zhongguo)),
        SymbolEngine::new(Vec::new(), Vec::new()),
        true,
    )
}

fn type_str(e: &mut Engine, s: &str) {
    for ch in s.chars() {
        e.input_key(ch);
    }
}

fn texts(e: &Engine, limit: usize) -> Vec<String> {
    e.candidates(limit).into_iter().map(|c| c.text).collect()
}

/// 规则 1–3 的主用例（用户给的那条）：`nihaob` 整串无命中 ⇒ 候选 = 最长前缀 `nihao`
/// 的整档；选中「你好」只消耗前缀，缓冲留 `b` 接着编。
#[test]
fn no_match_falls_back_to_longest_prefix_and_keeps_the_suffix() {
    let mut e = engine(false);
    type_str(&mut e, "nihaob");
    assert_eq!(e.buffer(), "nihaob", "预编辑必须还是用户打的整串");
    let cands = texts(&e, 8);
    assert_eq!(
        cands,
        vec!["你好", "你好啊", "利好"],
        "回退档 = 最长前缀 `nihao` 的整档"
    );
    assert_eq!(e.select(0), "你好");
    assert_eq!(e.buffer(), "b", "只消耗前缀：剩下的后缀留在缓冲里接着编");
    // 接着编：`b` + `ao` = `bao` → 包（后缀是真缓冲，不是摆设）
    type_str(&mut e, "ao");
    assert_eq!(e.buffer(), "bao");
    assert_eq!(texts(&e, 8), vec!["包"]);
}

/// 同一条经**数字选词**（路由层）：`digit_select` 抓自己那份候选表选中，收尾与点击同一处。
#[test]
fn digit_key_keeps_the_suffix_too() {
    let mut r = KeyRouter::new(engine(false));
    r.switch_mode(Mode::Pinyin);
    for c in "nihaob".chars() {
        r.key_event(c as u32, 0);
    }
    assert_eq!(
        r.key_event('1' as u32, 0),
        KeyAction::Input("你好".to_string())
    );
    assert_eq!(r.buffer(), "b", "数字选词同样只消耗前缀");
}

/// 规则 3 的**例外**（有意）：空格 = 整段上屏 —— 提交首候选、缓冲整段清空。
/// 与 `tests/adversarial_input.rs`「提交过就必须清空缓冲」的钉子同源。
#[test]
fn space_flushes_the_whole_buffer_even_in_fallback_state() {
    let mut e = engine(false);
    type_str(&mut e, "nihaob");
    assert_eq!(e.input_key(' '), "你好", "空格提交首候选");
    assert_eq!(e.buffer(), "", "空格是整段上屏，不留回退后缀");
}

/// 规则 4：回退态下两个读数出口自洽 —— 预编辑显示**用户打的整串**（不缩水到前缀），
/// 9 键专用的逐条读音在 26 键下本来就是空的；选中后两者都跟着缓冲走。
#[test]
fn readings_stay_consistent_in_fallback_state() {
    let mut e = engine(false);
    type_str(&mut e, "nihaob");
    assert_eq!(e.display_buffer(), "nihaob", "预编辑是整串，不显示前缀");
    assert!(
        e.candidate_readings().is_empty(),
        "26 键没有逐条读音出口（9/14 专用），回退不改变这一点"
    );
    e.select(0);
    assert_eq!(e.display_buffer(), "b", "选中后读数随缓冲走到后缀");
    assert!(e.candidate_readings().is_empty());
}

/// 边界 ①：单音节无匹配维持原「无匹配」（`seng` 是合法音节但词库没有它）——
/// 且「打英文」的逃生口不动：无候选时空格提交原始缓冲。
#[test]
fn single_syllable_without_match_stays_dead_and_keeps_the_escape_hatch() {
    let mut e = engine(false);
    type_str(&mut e, "seng");
    assert!(texts(&e, 8).is_empty(), "单音节无匹配不得被分词回退救活");
    assert_eq!(
        e.input_key(' '),
        "seng",
        "无候选 ⇒ 空格提交原始缓冲（逃生口）"
    );
    assert_eq!(e.buffer(), "");
}

/// 边界 ①的续：逗号等标点走同一条逃生口收尾（`qqq` 切出的段不是完整音节 ⇒ 死缓冲）。
#[test]
fn garbage_buffer_stays_dead_and_flushes_into_punctuation() {
    let mut e = engine(false);
    type_str(&mut e, "qqq");
    assert!(texts(&e, 8).is_empty(), "`qqq` 不得被分词回退救活");
    assert_eq!(
        e.input_key(','),
        "qqq，",
        "标点先收尾再上屏（与空格同一条逃生口）"
    );
    // `zxcvb` 同理（段全是单字母，不是完整音节）
    let mut e2 = engine(false);
    type_str(&mut e2, "zxcvb");
    assert!(texts(&e2, 8).is_empty());
}

/// 规则 2 的另一半：**前缀也没候选** ⇒ 不回退（维持无匹配），别切到更短的段上。
#[test]
fn prefix_without_candidates_does_not_fall_back() {
    let mut e = engine(false);
    type_str(&mut e, "sengseng");
    assert!(
        texts(&e, 8).is_empty(),
        "每段都是完整音节但都查不到 ⇒ 维持无匹配"
    );
}

/// 边界 ②：整串**模糊**命中优先 —— `zongguo` 有中国档时不许被 `zong` 的整档接管。
/// 判据取**后一个音节在不在候选里**：回退接管时 `guo` 是留着接着编的后缀、它的词不在；
/// 走逐音节汤时它在（`总`/`国` 之后并入整串变体「中国」——「整串变体是主结果」那条既有
/// 分档，见 tests/ranking_invariants.rs::whole_input_variant_suppresses_per_syllable_fuzzy）。
#[test]
fn whole_input_fuzzy_hit_suppresses_the_fallback() {
    // 对照组：词库里没有 `zhongguo` ⇒ 回退接管
    let mut plain = engine(false);
    type_str(&mut plain, "zongguo");
    let plain_texts = texts(&plain, 8);
    assert_eq!(plain_texts[0], "总", "无模糊命中 ⇒ 回退到最长前缀");
    assert!(
        !plain_texts.contains(&"国".to_string()),
        "回退档只含前缀 `zong` 一族（`guo` 是留着接着编的后缀）：{plain_texts:?}"
    );
    // 实验组：多一行 `zhongguo → 中国`，行为换轨（整串变体是主结果，逐音节汤照旧跑）
    let mut wide = engine(true);
    type_str(&mut wide, "zongguo");
    assert_eq!(
        texts(&wide, 8),
        vec!["总", "国", "中国"],
        "整串模糊命中 ⇒ 分词回退不接管（更具体的档优先）"
    );
}

/// `'`（分词键）不挡回退：词库键不含分隔符，整串直接查询本是空的 ——
/// 前缀串取各段拼接，`ni'hao` 出的是「你好」一族（旧行为是逐音节汤：你/尼/好/号）。
#[test]
fn separator_input_joins_segments_for_the_prefix() {
    let mut e = engine(false);
    type_str(&mut e, "ni'hao");
    assert_eq!(e.buffer(), "ni'hao");
    let cands = texts(&e, 8);
    assert_eq!(cands[0], "你好", "拼接后的整串才是用户的本意：{cands:?}");
    assert!(
        !cands.contains(&"号".to_string()),
        "逐音节汤不应再出现：{cands:?}"
    );
    // 前缀覆盖整个缓冲（`'` 不算字符）⇒ 选中后缓冲清空
    assert_eq!(e.select(0), "你好");
    assert_eq!(e.buffer(), "");
}

/// 分词回退只覆盖 26 键 / 笔画：9/14 键走 `rank_and_pick_scheme`（按键类展开），
/// 没有回退档 —— 缓冲是数字串/紧簇，分词对它们没有意义。
#[test]
fn nine_and_fourteen_keys_do_not_fall_back() {
    let mut nine = engine(false);
    nine.set_scheme(Scheme::Nine);
    type_str(&mut nine, "64426");
    assert_eq!(texts(&nine, 8)[0], "你好", "9 键本身照常（按键类展开）");
    // 多打一位 ⇒ 按键类展开无命中；不得回退到 `64426` 那档
    type_str(&mut nine, "9");
    assert!(
        texts(&nine, 8).is_empty(),
        "9 键整串无命中维持无候选（不做分词回退）"
    );
    let mut fourteen = engine(false);
    fourteen.set_scheme(Scheme::Fourteen);
    type_str(&mut fourteen, "bubu");
    assert!(
        texts(&fourteen, 8).is_empty(),
        "14 键整串无命中维持无候选（不做分词回退）"
    );
}
