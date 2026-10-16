// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **五笔引擎侧**的端到端门禁（2026-10-14，M4 计划 3b）。
//!
//! 全部经 `Engine::candidates` / `Engine::input_key` 的公开面走完整链路 —— 不绕到
//! `Dictionary::query` 直接断言（本仓有「排序门禁绕过排序代码」的前例：那样钉不住
//! 装配层）。夹具 = 测试内自建 `InMemoryDictionary`（码表数据由另一条线做，本文件
//! 不等数据；键按真实形态取普通 ASCII 字母）。
//!
//! 覆盖：纯五笔前缀与逐步收窄 / 空槽不假装 / 四码唯一自动上屏（含两条「不自动」+
//! 总闸开关两侧）/ 混输合并与分辨判据 / 混输拼音进入方式（DirectLetters / ZKey）/
//! 候选个数档位 / 编码提示出口 / 空格上屏 / 回车设置位 / 换装口重算 user_boost /
//! 前缀查询成本量级。夹具与 `type_str`/`texts` 在 `common/mod.rs`（共享，见那里头注）。

mod common;

use common::{engine, mixed_engine, no_symbols, pinyin_dict, texts, type_str, wubi_dict};
use engine_core::dictionary::InMemoryDictionary;
use engine_core::symbols::{Block, BlockId, SymbolEngine, SymbolEntry};
use engine_core::{Engine, Scheme, WubiMixedEntry, WubiReturnStyle, WubiStyle};

// ---- 纯五笔：前缀与逐步收窄 ----

#[test]
fn pure_wubi_prefixes_narrow_step_by_step() {
    let mut e = engine();
    e.set_wubi_candidate_limit(9); // 8 条命中要全看得见（默认档 5 见档位用例）
    type_str(&mut e, "a");
    let one = texts(&e);
    assert_eq!(
        one,
        ["工", "戈", "恭", "靴", "式", "节", "世", "范"],
        "精确（码长 1）吃精确加成排最前，其余按词频降序：{one:?}"
    );
    type_str(&mut e, "b");
    let two = texts(&e);
    assert_eq!(two, ["式", "节", "世", "范"], "ab 收窄到 4 条：{two:?}");
    type_str(&mut e, "c");
    let three = texts(&e);
    assert_eq!(three, ["节", "世", "范"], "abc 收窄到 3 条：{three:?}");
    // 逐步收窄（集合包含关系，与上面逐条断言互为佐证）
    assert!(one.len() > two.len() && two.len() > three.len());
}

/// 纯五笔**不许**出拼音候选（主库里有 ni → 你，但这是码不是拼音）。
#[test]
fn pure_wubi_ignores_the_pinyin_dict() {
    let mut e = engine();
    type_str(&mut e, "ni");
    let got = texts(&e);
    assert_eq!(got, ["尼"], "纯五笔只出码表候选：{got:?}");
}

/// 空槽 ⇒ 落普通路径（与 stroke 同一条「可选增强」姿态）：码串查不出东西就是空，
/// **不假装有码表**；而拼音形式的输入在普通路径下仍照常出（那是主库的事）。
#[test]
fn wubi_without_dict_does_not_fake_candidates() {
    let mut e = Engine::new(Box::new(pinyin_dict()), no_symbols(), false);
    e.set_scheme(Scheme::Wubi); // 不装 wubi_dict
    type_str(&mut e, "wqvb"); // 不是任何拼音键的前缀
    assert!(texts(&e).is_empty(), "没码表就得如实为空：{:?}", texts(&e));
    e.clear(); // wqvb 不是拼音前缀，清掉再打（否则缓冲是 wqvbni，那本来也没候选）
    type_str(&mut e, "ni");
    assert!(
        texts(&e).contains(&"你".to_string()),
        "普通路径下拼音输入照常（如实降级，不是假装码表）"
    );
}

// ---- 四码唯一自动上屏 ----

#[test]
fn auto_commit_fires_on_unique_fourth_code() {
    let mut e = engine();
    // 前三条键不许提前提交（1–3 码是简码，绝不自动）
    assert_eq!(e.input_key('a'), "", "1 码不自动");
    assert_eq!(e.input_key('b'), "", "2 码不自动");
    assert_eq!(e.input_key('c'), "", "3 码不自动");
    assert_eq!(e.buffer(), "abc");
    // 第 4 键：abcd 全码唯一（范）⇒ 返回值直接带词、缓冲清空
    assert_eq!(e.input_key('d'), "范", "四码唯一应自动上屏");
    assert_eq!(e.buffer(), "", "自动上屏后缓冲必须清空");
}

/// 同码多词 ⇒ **不**自动（用户要选）。变异自检：把「整档恰好 1 条」放宽成
/// 「整档非空」⇒ 本用例红（第 4 键会返回候选并清空缓冲）。
#[test]
fn auto_commit_skips_when_the_code_has_two_words() {
    let mut e = engine();
    type_str(&mut e, "aa");
    assert_eq!(e.input_key('a'), "");
    assert_eq!(e.input_key('a'), "", "aaaa 同码多词（恭/靴）不得自动");
    assert_eq!(e.buffer(), "aaaa", "缓冲原样留着等用户选");
    let got = texts(&e);
    assert_eq!(got, ["恭", "靴"], "两条都在：{got:?}");
}

/// 还有更长的码以它为先缀（延伸码）⇒ 整档 2 条 ⇒ 不自动。
#[test]
fn auto_commit_skips_codes_with_longer_extensions() {
    let mut e = engine();
    type_str(&mut e, "efg");
    assert_eq!(e.input_key('h'), "", "efgh 整档含延伸码 efghi ⇒ 不自动");
    assert_eq!(e.buffer(), "efgh");
}

/// 整档唯一，但那条命中是**更长的码**（不是精确四码）⇒ 不自动。
/// 变异自检：去掉「精确命中」这半条判据 ⇒ 本用例红（弈会被自动提交）。
#[test]
fn auto_commit_skips_when_the_only_hit_is_a_longer_code() {
    let mut e = engine();
    type_str(&mut e, "mno");
    assert_eq!(
        e.input_key('p'),
        "",
        "mnop 无精确条目（只有 mnopq）⇒ 不自动"
    );
    assert_eq!(e.buffer(), "mnop");
}

/// 符号层的前缀命中也是**候选栏里看得见的候选**（`symbols.search` 是前缀匹配）：
/// 码表唯一但符号栏还挂着东西 ⇒ 同样不自动。判据是「候选层恰好一条」，不是
/// 「码表里恰好一条」。变异自检：去掉 `symbols.search` 那半条 ⇒ 本用例红。
#[test]
fn auto_commit_skips_when_the_symbol_layer_also_hits() {
    let symbols = SymbolEngine::new(
        vec![Block {
            id: BlockId(1),
            start: 0x2600,
            end: 0x26FF,
            name: "杂项符号".into(),
            common: true,
        }],
        vec![SymbolEntry {
            text: "◆".into(),
            name: "菱形".into(),
            keywords: vec!["abcdx".into()], // 与码 abcd 同前缀（前四个字母相同）
            block: BlockId(1),
            emoji: false,
        }],
    );
    let mut e = Engine::new(Box::new(pinyin_dict()), symbols, false);
    e.set_wubi_dict(Some(Box::new(wubi_dict())));
    e.set_scheme(Scheme::Wubi);
    type_str(&mut e, "abcd");
    assert!(
        texts(&e).contains(&"◆".to_string()),
        "前提：符号确实进了候选栏（否则本用例是空转）：{:?}",
        texts(&e)
    );
    assert_eq!(e.buffer(), "abcd", "候选栏不止一条 ⇒ 不自动上屏");
}

/// 混输**不**自动上屏（缓冲可能是拼音，见 candidates_wubi 尾注）。
#[test]
fn auto_commit_is_off_in_mixed_style() {
    let mut e = mixed_engine();
    type_str(&mut e, "abc");
    assert_eq!(
        e.input_key('d'),
        "",
        "混输下 abcd 不自动（DirectLetters 缺省档）"
    );
    assert_eq!(e.buffer(), "abcd");
}

/// 总闸两侧（lead 收货时补的开关）：关 ⇒ 四码唯一也不自动；开回来 ⇒ 照旧自动。
/// 变异自检：删掉 `wubi_autocommit` 开头的 `!self.wubi_auto_commit` 守卫 ⇒ 本用例红。
#[test]
fn auto_commit_can_be_switched_off() {
    let mut e = engine();
    assert!(e.wubi_auto_commit(), "默认开（对齐主流输入法缺省）");
    e.set_wubi_auto_commit(false);
    type_str(&mut e, "abc");
    assert_eq!(e.input_key('d'), "", "关掉后四码唯一也不自动");
    assert_eq!(e.buffer(), "abcd", "缓冲原样留着等用户");
    e.set_wubi_auto_commit(true);
    assert_eq!(e.buffer(), "abcd", "翻开关不清缓冲（同一缓冲继续）");
    e.clear();
    type_str(&mut e, "abc");
    assert_eq!(e.input_key('d'), "范", "开着才自动（另一侧）");
}

// ---- 混输的拼音进入方式（DirectLetters 默认 / ZKey）----

#[test]
fn mixed_z_key_entry_interprets_the_z_prefix_as_pinyin() {
    let mut e = mixed_engine();
    e.set_wubi_mixed_entry(WubiMixedEntry::ZKey);
    type_str(&mut e, "znihao");
    assert_eq!(texts(&e), ["你好"], "z 后按拼音（去前导 z）");
    assert_eq!(e.buffer(), "znihao", "缓冲原文不动：z 只是解释标志");
    e.clear();
    type_str(&mut e, "nihao");
    assert!(
        texts(&e).is_empty(),
        "ZKey 下非 z 段只出码候选（码表无此码 ⇒ 空）"
    );
    e.clear();
    type_str(&mut e, "z");
    assert!(
        texts(&e).is_empty(),
        "单个 z 没有余串 ⇒ 无候选（z 不是输入）"
    );
}

#[test]
fn direct_letters_is_the_default_entry() {
    let mut e = mixed_engine();
    assert_eq!(e.wubi_mixed_entry(), WubiMixedEntry::DirectLetters);
    type_str(&mut e, "znihao");
    assert!(texts(&e).is_empty(), "直接字母下 znihao 两边都不像 ⇒ 空");
    e.set_wubi_mixed_entry(WubiMixedEntry::ZKey); // 换档不清缓冲：同一 z 串重解
    assert_eq!(texts(&e), ["你好"], "两档的差别就是 ZKey 的存在理由");
    // 纯五笔没有拼音侧 ⇒ z 档无效果（若 z 分支漏进 Pure，这里会出「你好」）
    e.set_wubi_style(WubiStyle::Pure);
    e.clear();
    type_str(&mut e, "znihao");
    assert!(texts(&e).is_empty(), "Pure 下 z 档惰性，不假装");
}

// ---- 混输：同一层合并 + 分辨判据 ----

/// 缓冲同时是码前缀（尼）与拼音前缀（你/你好）⇒ 两边候选在**同一层**按**同一分数量纲**
/// 合并。逐条断言顺序：你 = 5000 + 精确加成 5 万 > 尼 = 700 + 5 万 > 你好 = 6000。
/// 若哪边被分组到前/后（不是同一量纲排序），这个顺序当场破。
#[test]
fn mixed_merges_wubi_and_pinyin_in_one_layer() {
    let mut e = mixed_engine();
    type_str(&mut e, "ni");
    let got = texts(&e);
    assert_eq!(
        got,
        ["你", "尼", "你好"],
        "码表与拼音候选同层同量纲：{got:?}"
    );
}

/// 分辨判据（见 engine_wubi.rs 头注 2）：命中由各自词库的**前缀查询自证** ——
/// 五笔专属码只出码候选、拼音专属串只出拼音候选，两边都无命中就是空。
#[test]
fn mixed_resolution_follows_the_dictionary_prefixes() {
    let mut e = mixed_engine();
    type_str(&mut e, "efgh");
    let wubi_only = texts(&e);
    assert_eq!(
        wubi_only,
        ["森", "麓"],
        "拼音库里没有 efgh 前缀：{wubi_only:?}"
    );
    e.clear();
    type_str(&mut e, "hao");
    let pinyin_only = texts(&e);
    assert_eq!(pinyin_only, ["好"], "码表里没有 hao 前缀：{pinyin_only:?}");
    e.clear();
    type_str(&mut e, "zzz");
    assert!(texts(&e).is_empty(), "两边都无命中 ⇒ 空（不假装）");
}

/// 切回 Pure ⇒ 拼音候选消失、缓冲保留（风格只改装配，不清缓冲）。
#[test]
fn switching_back_to_pure_removes_pinyin_candidates() {
    let mut e = mixed_engine();
    type_str(&mut e, "ni");
    assert!(
        texts(&e).contains(&"你".to_string()),
        "前置：混输有拼音候选"
    );
    e.set_wubi_style(WubiStyle::Pure);
    assert_eq!(e.buffer(), "ni", "换风格不清缓冲（同一缓冲按新风格重解）");
    assert_eq!(texts(&e), ["尼"], "Pure 下拼音候选必须消失");
}

// ---- 候选个数档位（引擎侧截断）----

#[test]
fn candidate_limit_caps_and_clamps() {
    let mut e = engine();
    assert_eq!(
        e.wubi_candidate_limit(),
        engine_core::WUBI_CANDIDATE_LIMIT_DEFAULT,
        "默认档 = 5"
    );
    type_str(&mut e, "a");
    assert_eq!(texts(&e).len(), 5, "默认档截到 5（夹具 8 条命中）");
    // 下标空间随档位收窄（截断在引擎侧，select 也看得见）
    assert_eq!(e.select(4), "式", "第 5 项在档内");
    assert_eq!(e.select(5), "", "第 6 项在档外 ⇒ 如实空串");
    e.set_wubi_candidate_limit(3);
    type_str(&mut e, "a"); // select 已提交并清缓冲，重新打（学习器本夹具关闭）
    assert_eq!(texts(&e), ["工", "戈", "恭"], "3 档只留前三");
    // 钳制：0 → 1、超大 → 上限
    e.set_wubi_candidate_limit(0);
    assert_eq!(e.wubi_candidate_limit(), 1);
    assert_eq!(texts(&e).len(), 1);
    e.set_wubi_candidate_limit(100);
    assert_eq!(
        e.wubi_candidate_limit(),
        engine_core::WUBI_CANDIDATE_LIMIT_MAX
    );
    e.set_wubi_candidate_limit(9);
    assert_eq!(texts(&e).len(), 8, "夹具只有 8 条，9 档给全");
}

/// 混输下档位同样生效（合并后的最终列表也按档位定截）。
#[test]
fn candidate_limit_applies_to_mixed_too() {
    let mut e = mixed_engine();
    e.set_wubi_candidate_limit(2);
    type_str(&mut e, "ni");
    assert_eq!(texts(&e), ["你", "尼"], "合并后截到 2");
}

// ---- 编码提示（剩余码；读侧出口）----

#[test]
fn next_keys_hint_follows_the_code_table() {
    let mut e = engine();
    assert_eq!(e.wubi_next_keys(), Vec::<char>::new(), "空缓冲没有当前码");
    type_str(&mut e, "a");
    assert_eq!(
        e.wubi_next_keys(),
        vec!['a', 'b'],
        "a → 只能接 a(aaaa)/b(ab)"
    );
    type_str(&mut e, "b");
    assert_eq!(
        e.wubi_next_keys(),
        vec!['c'],
        "ab → 只接 c(abc)（d 要等 abc）"
    );
    let again = e.wubi_next_keys();
    assert_eq!(e.wubi_next_keys(), again, "确定性：同缓冲同结果");
    e.clear();
    type_str(&mut e, "zz");
    assert!(e.wubi_next_keys().is_empty(), "没有码以 zz 起头 ⇒ 空");
}

#[test]
fn next_keys_hint_only_in_wubi_scheme() {
    let mut e = engine();
    type_str(&mut e, "ab");
    assert_eq!(e.wubi_next_keys(), vec!['c'], "前置：五笔方案下有提示");
    e.set_scheme(Scheme::Full);
    assert!(e.wubi_next_keys().is_empty(), "非五笔方案 ⇒ 空");
}

// ---- 空格上屏（复用既有 input_space）+ 回车设置位 ----

#[test]
fn space_commits_first_candidate_and_clears() {
    let mut e = engine();
    type_str(&mut e, "aaaa"); // 同码多词，不触发自动上屏
    assert_eq!(e.input_space(), "恭", "空格 = 提交首候选");
    assert_eq!(e.buffer(), "", "空格提交后缓冲清空");
    // 无候选的码串：落既有逃生口（提交缓冲原文）—— 与 Full/Stroke 同一条行为
    type_str(&mut e, "zzzz");
    assert_eq!(
        e.input_space(),
        "zzzz",
        "无候选时空格提交原文（既有逃生口）"
    );
}

#[test]
fn return_style_is_a_setting_bit() {
    let mut e = engine();
    assert_eq!(e.wubi_return_style(), WubiReturnStyle::Clear, "默认 Clear");
    e.set_wubi_return_style(WubiReturnStyle::CommitRaw);
    assert_eq!(e.wubi_return_style(), WubiReturnStyle::CommitRaw);
    assert_ne!(WubiReturnStyle::default(), WubiReturnStyle::CommitRaw);
    // 消费判据在 `Engine::enter_action`（2026-10-14 接线，见 `engine_enter.rs`）——
    // 本文件只钉设置位本身；行为级用例在 tests/enter_action.rs。
}

// ---- 换装口重算 user_boost（engine_dicts.rs 的不变量）----

/// `set_wubi_dict` 必须重算 `user_boost`：装了高频码表后，一次选词仍要压过全部
/// 静态词。去掉 `recompute_user_boost()` 调用 ⇒ 本用例红（学过的「乙」压不过
/// 静态高频的「甲」，与 tests/dict_layers.rs 的同类用例同一条判据）。
#[test]
fn set_wubi_dict_recomputes_user_boost() {
    let mut d = InMemoryDictionary::new();
    d.insert("aa", "甲", 1_000_000_000);
    d.insert("aa", "乙", 1);
    let mut e = Engine::new(Box::new(pinyin_dict()), no_symbols(), true); // 学习器开
    e.set_wubi_dict(Some(Box::new(d)));
    e.set_scheme(Scheme::Wubi);
    type_str(&mut e, "aa");
    let cands = e.candidates(9);
    assert_eq!(cands[0].text, "甲", "前置：无学习时静态高频在前");
    assert_eq!(e.select_from(&cands, 1), "乙", "选一次「乙」记学习");
    type_str(&mut e, "aa");
    assert_eq!(
        texts(&e)[0],
        "乙",
        "一次选词须压过码表最大静态词频（boost 按装表后的最大词频重算）"
    );
}

// ---- 性能量级（打印，不断言绝对时间 —— CI 机器负载不定）----

/// 五笔与笔画同型：贵项是前缀查询，成本随**该前缀下的扩展条数**走。
/// 夹具 3 万码，两种分布各测一条（都走公开面 `candidates`，混输 = 两路查询的最贵路）：
/// - 均匀分布（首字母摊到 26 个字母 ≈ 真表形态）：1 码（扇出 ~1.2k）vs 4 码（近全长）；
/// - 全倾分布（3 万码全以 `a` 起头）：1 码即**最坏档**，给上界。
#[test]
fn wubi_prefix_cost_is_reported() {
    use std::time::Instant;
    const N: usize = 30_000;
    let mk = |first: u8, i: usize| {
        let mut s = String::from(first as char);
        let mut x = i;
        for _ in 0..3 {
            s.push((b'a' + (x % 26) as u8) as char);
            x /= 26;
        }
        s
    };
    let build = |skewed: bool| {
        let mut d = InMemoryDictionary::new();
        for i in 0..N {
            let first = if skewed { b'a' } else { b'a' + (i % 26) as u8 };
            d.insert(&mk(first, i), &format!("词{i}"), (N - i) as u32);
        }
        d
    };
    let engine_with = |d: InMemoryDictionary| {
        let mut e = Engine::new(Box::new(pinyin_dict()), no_symbols(), false);
        e.set_wubi_dict(Some(Box::new(d)));
        e.set_scheme(Scheme::Wubi);
        e.set_wubi_style(WubiStyle::Mixed); // 两路合并 = 最贵
        e
    };
    // 中位数（9 次，预热一次）；不用均值 —— 共机并发下离群值会骗人。
    let median = |v: &mut Vec<f64>| {
        v.sort_by(|x, y| x.partial_cmp(y).unwrap());
        v[v.len() / 2]
    };
    let probe = |e: &mut Engine, buf: &str| {
        let mut samples = Vec::new();
        e.clear();
        type_str(e, buf);
        std::hint::black_box(e.candidates(9)); // 预热
        for _ in 0..9 {
            let t = Instant::now();
            std::hint::black_box(e.candidates(9));
            samples.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        median(&mut samples)
    };
    let mut spread = engine_with(build(false));
    let one = probe(&mut spread, "a");
    let four = probe(&mut spread, "abcz");
    let mut skewed = engine_with(build(true));
    let worst = probe(&mut skewed, "a");
    println!(
        "[wubi] candidates 中位数（夹具 {N} 码，混输）：均匀 1 码 {one:.3}ms / 4 码 {four:.3}ms；\
         全倾 1 码（最坏档）{worst:.3}ms（预算 <50ms/键，见 docs/performance.md；\
         debug 档位约 10× 于 release，同一用例两种档位都跑过）"
    );
}
