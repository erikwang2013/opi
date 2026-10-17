// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **分词回退**（2026-10-10 用户裁决「输入拼音无匹配时要分词匹配」）的引擎级语义门禁。
//!
//! 语义定案（本文件逐条钉住）：
//! 1. **触发**：整串直接命中为空、整串模糊命中也为空（「更具体的匹配优先」）、缓冲不止
//!    一个音节 —— 三者全成立才回退；
//! 2. **候选**：从最长的音节对齐前缀往短试，第一个「每段都是完整音节 + 词库有命中」的
//!    前缀胜出，候选换成**它**的整档（递归走同一条精确/模糊/简拼分档，不另起一套排序）；
//! 3. **选中**：~~提交该候选文本~~ **【订正 2026-10-17】** 逐字确认落地后，回退态的
//!    选中是**延迟提交**（`Enum`：本轮 `select` 返回空串、文本进 `pending_text()`
//!    留在候选区；最后一字确认才整串提交 —— 见 tests/segment_commit.rs 与本文件的
//!    逐条更新）。缓冲**只消耗那个前缀**、剩下的后缀留着接着编
//!    （`nihaob` 选「你好」→ 缓冲变 `b`）；提交文本/学习/清缓冲的收尾仍是 `select_from`
//!    那一处（四端自动一致）；
//! 4. **读数**：26 键回退态下 `display_buffer()` 仍是**用户打的整串**（预编辑不缩水）、
//!    `candidate_readings()` 仍是空（那是 9/14 键专用）；9 键回退态下读数照旧是
//!    「首条候选的拼音」（`644269` → `ni'hao`，左列不因回退而空白）；
//! 5. **边界**：单音节无匹配维持原「无匹配」；前缀也无候选维持原「无匹配」；
//!    垃圾串维持死（26 键：段不是完整音节 `qqq`/`zxcvb`；9/14 键：没有音节对齐的
//!    见证者）—— 「打英文」的逃生口不动。
//!
//! **9/14 键是同一语义的数字空间版**（2026-10-10 裁决「回退扩到 9 键」）：按键串无命中
//! 时丢掉尾部凑不满音节的键，候选换成最长「音节对齐且查得到」键前缀的整档、选中只消耗
//! 前缀的键（`64426`→`644269` = nihao + 下一音节首键，用户报障的现场）。判定唯一实现在
//! `scheme::scheme_fallback_len`（复用 `Scheme::matches` 的按键类与 `pinyin::segment`
//! 分段源 —— 与 9 键读数链同一套，不另起一套）；14 键走同一判定自然覆盖（`bugaom`）。
//!
//! ⚠️ **2026-10-15 收口（用户报障「确认候选后再打字没反应、候选栏无匹配」）**：9/14 键的
//! 尾巴**≥3 键且自己出不来候选时不留**（1–2 键照旧留 —— 「下一音节刚起头」的主场景）。
//! 理由与判据见 `Engine::digit_tail_to_keep`：数字尾巴在 9 键上**看不见**（拼音行按读数
//! 显示、读数空就留空），一段整读不出来的长尾巴 = 用户无从察觉的死局。
//! 26 键不吃这条：它的尾巴是**可见的**预编辑文本。
//!
//! ⚠️ **空格键有意例外**（两条回退腿同一条代码）：`Engine::input_space` 是「整段上屏」
//! （无候选才提交原始缓冲的逃生口）——回退态下它提交首候选并**整段清空**，不收后缀
//! （钉子：tests/adversarial_input.rs 的「提交过就必须清空缓冲」；另有
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
    // 2026-10-17 逐字确认：回退态选中 = 延迟提交（片段留在候选区）
    assert_eq!(e.select(0), "", "回退态选中延迟提交");
    assert_eq!(e.pending_text(), "你好");
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
    // 2026-10-17 逐字确认：回退态的数字选词同样延迟（不提交，片段留在候选区）
    assert_eq!(r.key_event('1' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(r.pending_text(), "你好");
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
    // 2026-10-17 逐字确认：显示串前缀已确认的字（片段 + 剩余缓冲）
    assert_eq!(
        e.display_buffer(),
        "你好b",
        "选中后显示 = 已确认片段 + 后缀"
    );
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

/// 9 键数字空间回退（用户报障的现场：`64426`→`644269` = nihao + 下一音节首键 `9`）：
/// 整串无命中 ⇒ 丢掉尾部凑不满音节的键，候选换成最长音节对齐键前缀 `64426`（nihao）
/// 的整档；选中只消耗前缀的键，尾部键留在缓冲里接着编。判定复用 9 键既有 machinery
/// （`Scheme::matches` 的按键类 + `boundary_aligned`/`pinyin::segment` 分段源）。
#[test]
fn nine_key_drops_the_incomplete_tail_and_keeps_it_in_the_buffer() {
    let mut nine = engine(false);
    nine.set_scheme(Scheme::Nine);
    type_str(&mut nine, "64426");
    assert_eq!(texts(&nine, 8)[0], "你好", "前置：命中态照旧（按键类展开）");
    type_str(&mut nine, "9");
    assert_eq!(nine.buffer(), "644269", "预编辑仍是用户打的整串");
    assert_eq!(
        texts(&nine, 8),
        vec!["你好", "你好啊", "利好"],
        "回退档 = 键前缀 64426 的整档"
    );
    assert_eq!(
        nine.display_buffer(),
        "ni'hao",
        "9 键读数照旧是首条候选的拼音（回退不把左列打空）"
    );
    assert_eq!(nine.select(0), "", "回退态选中延迟提交（2026-10-17）");
    assert_eq!(nine.pending_text(), "你好");
    assert_eq!(nine.buffer(), "9", "只消耗前缀的键：尾部键留下接着编");
}

/// 14 键同问的答案（2026-10-10 裁决「14 键若同机制自然适用」）：能 —— `bugaom` =
/// bugao（= nihao 的键）+ 尾部 `m`（凑不满音节），同一个 `scheme_fallback_len` 给出
/// k=5，候选 = nihao 整档、缓冲留 `m`，不需要第二套实现。
#[test]
fn fourteen_key_covers_the_same_fallback() {
    let mut e14 = engine(false);
    e14.set_scheme(Scheme::Fourteen);
    type_str(&mut e14, "bugaom");
    assert_eq!(
        texts(&e14, 8),
        vec!["你好", "你好啊", "利好"],
        "14 键同一条回退：bugao（nihao 的键）+ 尾 m"
    );
    assert_eq!(e14.select(0), "", "回退态选中延迟提交（2026-10-17）");
    assert_eq!(e14.pending_text(), "你好");
    assert_eq!(e14.buffer(), "m", "尾部键留在缓冲里接着编");
}

/// **长垃圾尾巴不留**（2026-10-15 用户报障的回归门禁；口径 = 用户裁决的折中版）。
///
/// 现场：9 键连按同一个键按出长串 → 回退只吃前缀 → 尾巴 ≥3 键且**整段读不出来**，
/// 而它在 9 键上**看不见**（拼音行按读数显示、读数空就留空）⇒ 用户看到「无匹配」、
/// 之后打什么都像没反应。判据与两条理由见 `Engine::digit_tail_to_keep`。
#[test]
fn dead_long_digit_tail_is_dropped_whole() {
    let mut e = engine(false);
    e.set_scheme(Scheme::Nine);
    // `64` = ni（整档有命中）+ `444` = `hi` 之后再凑不成段 ⇒ 三键死尾巴
    type_str(&mut e, "64444");
    let cands = texts(&e, 8);
    assert_eq!(
        cands.first().map(String::as_str),
        Some("你"),
        "回退档 = 前缀 `64`（ni）的整档（首位是 你，长词按今天的分档排在后面）"
    );
    assert_eq!(e.select(0), "你");
    assert_eq!(
        e.buffer(),
        "",
        "长死尾巴不许留：选中后整段清空（留它就复现「再打别的拼音没反应」）"
    );
    type_str(&mut e, "64");
    assert!(
        texts(&e, 8).contains(&"你".to_string()),
        "清空之后接着打必须是活的"
    );
}

/// 长尾巴的**反面**：≥3 键但**自己读得出来**（`226` = bao）⇒ 照旧留着接着编。
/// 与上一条配对 —— 少了它，「清死尾巴」很容易被写成「凡长尾巴一律清」。
#[test]
fn live_digit_tail_is_still_kept() {
    let mut e = engine(false);
    e.set_scheme(Scheme::Nine);
    // nihao（`64426`，5 键）+ `226`（= bao，夹具里有「包」）—— 别写成 `644269`：
    // 那是「ni + hao」的分段写法，多出来的 `9` 会把尾巴变成 `9226`。
    type_str(&mut e, "64426226");
    let cands = texts(&e, 8);
    assert!(
        cands.contains(&"你好".to_string()),
        "前缀档仍是 nihao 一族：{cands:?}"
    );
    assert_eq!(e.select(0), "", "回退态选中延迟提交（2026-10-17）");
    assert_eq!(e.pending_text(), "你好");
    assert_eq!(
        e.buffer(),
        "226",
        "尾巴自己读得出来（bao → 包）⇒ 留着接着编"
    );
    assert!(
        texts(&e, 8).contains(&"包".to_string()),
        "留下的尾巴必须能出候选"
    );
}

/// 短尾巴（1–2 键）**不探**、照旧留：那是「下一音节刚起头」的主场景，判定不许押在
/// 词库覆盖上（真库里 `bugaoml` 的 `ml` 就靠这条 —— `crates/opi-tools/tests/
/// m2_integration.rs::scheme_fallback_on_real_luna_dictionary` 钉着）。
#[test]
fn short_digit_tail_is_kept_without_probing() {
    let mut e = engine(false);
    e.set_scheme(Scheme::Nine);
    // nihao（`64426`）+ `44`（两键：夹具里「44」读得出 hi 但 hi 无词条 ——
    // 若探针管到两键，这条尾巴就会被清）
    type_str(&mut e, "6442644");
    let _ = e.select(0);
    assert_eq!(e.buffer(), "44", "两键尾巴不探、照旧留");
}

/// 边界沿用：单键/垃圾串维持死（没有「音节对齐且查得到」的见证者）；`'` 是用户
/// **显式**的音节边界主张，回退免谈 —— `matches_segmented` 的严格语义不动
/// （断点语义的钉子还在 tests/scheme_keys.rs::segment_key_forces_the_syllable_boundary）。
#[test]
fn scheme_fallback_boundaries_stay_dead() {
    // 单键：没有更短的前缀可丢（`5` = jkl，夹具里没有 j/k/l 起头的键）
    let mut nine = engine(false);
    nine.set_scheme(Scheme::Nine);
    type_str(&mut nine, "5");
    assert!(texts(&nine, 8).is_empty(), "单键输入维持死");
    // 乱按：没有见证者（`55` 的每一段都凑不出音节）
    type_str(&mut nine, "5");
    assert_eq!(nine.buffer(), "55");
    assert!(texts(&nine, 8).is_empty(), "无见证者的乱按维持死");

    let mut e14 = engine(false);
    e14.set_scheme(Scheme::Fourteen);
    type_str(&mut e14, "ll");
    assert!(texts(&e14, 8).is_empty(), "14 键垃圾串维持死");

    // 分词符输入不做数字空间回退（显式边界，与 matches_segmented 同语义）
    let mut seg_input = engine(false);
    seg_input.set_scheme(Scheme::Nine);
    type_str(&mut seg_input, "644'26");
    assert!(
        texts(&seg_input, 8).is_empty(),
        "`644'26` 断在 nih 后（非音节边界）谁都不匹配，回退也不救"
    );
}

/// 空格例外对 9 键同一条代码（`input_space`）：回退态下空格提交首候选并**整段**清空
/// ——尾部键随之丢弃，与 26 键 `nihaob` 的空格行为逐字一致（钉子见本文件头注记）。
#[test]
fn nine_key_space_flushes_the_whole_buffer_too() {
    let mut nine = engine(false);
    nine.set_scheme(Scheme::Nine);
    type_str(&mut nine, "644269");
    assert_eq!(nine.input_key(' '), "你好", "空格提交首候选");
    assert_eq!(nine.buffer(), "", "空格是整段上屏：尾部键不留");
}

/// 9 键**无候选**时空格什么都不上屏：缓冲是按键编码不是文本，「无候选提交原文」的
/// 逃生口对它不成立 —— 提交原文 = 把数字打进字段（2026-10-11 用户报障「9 键没匹配
/// 的时候会变成数字输入」）。返回空串且**不清缓冲**：退格改一键还能重来（与符号
/// 模式同一条理由）。
#[test]
fn nine_key_space_with_no_match_commits_nothing() {
    let mut nine = engine(false);
    nine.set_scheme(Scheme::Nine);
    type_str(&mut nine, "644'26");
    assert!(
        texts(&nine, 8).is_empty(),
        "前提：该串无候选（同上一条测试）"
    );
    assert_eq!(nine.input_key(' '), "", "无候选：空格不上屏任何文本");
    assert_eq!(nine.buffer(), "644'26", "不清缓冲：退格改一键仍可重来");
}
