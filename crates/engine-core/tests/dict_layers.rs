// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 导入词库层（M3 计划 Task 2b，2026-10-14）的门禁。
//!
//! 全部**经 `Engine::candidates`** 走真路径 —— 不许绕到 `dict.query`：本仓有
//! 「排序门禁绕过排序代码」的先例（对 `candidates.rs` 层的改动失明），这里锁的是
//! 用户能看到的候选表与分数。
//!
//! 契约（`dict_layers.rs` 头）对应关系：
//! - 导入层参与合并 / 停用即消失 / 全集替换 → 前两条测试；
//! - 同键取**最大**词频（变异成 min 必红）→ duplicate_word_takes_max_freq；
//! - 精确档语义不被合并破坏（打全拼音仍吃 exact_bonus）→ exact_rank_survives；
//! - user_boost 随层重算（变异：去掉重算必红）→ import_high_freq_word_recomputes…；
//! - 导入层只叠主库（裁决 3）→ trad_mode_does_not_see_import_layer。

use engine_core::candidates::Candidate;
use engine_core::composer::Mode;
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, InMemoryDictionary};

fn dict(entries: &[(&str, &str, u32)]) -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    for (py, w, f) in entries {
        d.insert(py, w, *f);
    }
    d
}

/// 符号表用空表：本文件测的是词典层，符号数据来自生成的文件，不该参与断言。
fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

fn engine(base: &[(&str, &str, u32)]) -> Engine {
    let mut e = Engine::with_dictionaries(Box::new(dict(base)), None, no_symbols(), false);
    // 预算门禁是**时间量**：机器被并发构建占满时 20ms（动态档 10ms）余量会把导入层
    // 合法地整层丢弃 —— 本文件其余用例只断词典层语义，不测预算，会被读成假红。
    // 超预算语义由两条专门用例显式设零预算覆盖。
    e.set_import_scan_budget(None);
    e
}

/// 敲一串按键（走真 composer 路径，候选才知道缓冲是什么）。
fn type_keys(e: &mut Engine, keys: &str) {
    for ch in keys.chars() {
        e.input_key(ch);
    }
}

fn score_of(cands: &[Candidate], text: &str) -> u64 {
    cands
        .iter()
        .find(|c| c.text == text)
        .unwrap_or_else(|| panic!("候选表里没有 {text:?}"))
        .score
}

/// 导入层的词真出现在候选表里；`extra_dicts` 报名字与条数。
#[test]
fn import_layer_words_appear_in_candidates() {
    let mut e = engine(&[("ni", "你", 3900), ("hao", "好", 3000)]);
    e.set_extra_dicts(vec![(
        "测试表".into(),
        dict(&[("nihao", "你好呀", 9_000_000)]),
    )]);
    type_keys(&mut e, "nihao");
    let cands = e.candidates(8);
    assert!(
        cands.iter().any(|c| c.text == "你好呀"),
        "导入层的词必须能出候选（真经 Engine::candidates）：{cands:?}"
    );
    assert_eq!(e.extra_dicts(), vec![("测试表".to_string(), 1)]);
}

/// 停用（不在集合里）即从候选消失；**全集替换**（不是追加、不嵌套）。
#[test]
fn disable_and_replace_drop_the_layer() {
    let base = &[("nihao", "你好", 3900)];
    let mut e = engine(base);
    e.set_extra_dicts(vec![(
        "甲表".into(),
        dict(&[("nihao", "你好呀", 9_000_000)]),
    )]);
    type_keys(&mut e, "nihao");
    assert!(e.candidates(8).iter().any(|c| c.text == "你好呀"));

    // 停用：空集合 = 回到纯系统库。
    e.set_extra_dicts(vec![]);
    assert!(
        e.candidates(8).iter().all(|c| c.text != "你好呀"),
        "停用后必须消失"
    );
    assert!(e.extra_dicts().is_empty());

    // 替换：第二次 set 是全集替换 —— 甲表的词不能借「层层嵌套」活下来。
    e.set_extra_dicts(vec![(
        "甲表".into(),
        dict(&[("nihao", "你好呀", 9_000_000)]),
    )]);
    e.set_extra_dicts(vec![(
        "乙表".into(),
        dict(&[("nihao", "你好吗", 9_000_000)]),
    )]);
    let cands = e.candidates(8);
    assert!(cands.iter().any(|c| c.text == "你好吗"), "新集合的生效");
    assert!(
        cands.iter().all(|c| c.text != "你好呀"),
        "旧集合必须整体消失（全集替换）"
    );
    assert_eq!(e.extra_dicts(), vec![("乙表".to_string(), 1)]);
}

/// 同 (拼音, 词) 与系统库重复时取**最大**词频 —— 变异成「取最小」⇒ 本测试必红。
///
/// 2026-10-17「抢位」加成是**广覆盖**（重复项也抬，见 `boosted_import_freq` 头注
/// 的蘒实证：只抬独有条目时它出不了手机抓取窗），所以本测试用**两组导入词频**
/// 对比同一词：取 max 时高表那轮分更高；取 min 时两轮都退回 base 的原词频、
/// 分相同（红）。另补「加成后绝不低过纯内置」与「更高频重复顶上来」两支。
#[test]
fn duplicate_word_takes_max_freq() {
    // base_max = 5_000_000 ⇒ 加成带 ≈ [4.96e6, 5e6)：顶上的 好 永远压过加成
    // （严格低于顶），非顶的 号 会被抬进带内 —— 两个方向都能观察。
    let base = &[("hao", "好", 5_000_000), ("hao", "号", 50)];
    let mut plain = engine(base);
    type_keys(&mut plain, "hao");
    let p = plain.candidates(8);

    let run = |freq: u32| {
        let mut e = engine(base);
        e.set_extra_dicts(vec![(
            "对比表".into(),
            dict(&[("hao", "好", freq), ("hao", "号", freq)]),
        )]);
        type_keys(&mut e, "hao");
        e.candidates(8)
    };
    let low = run(300);
    let high = run(30_000);

    assert!(
        score_of(&high, "号") > score_of(&low, "号"),
        "取最大：导入 30_000 的轮次必须比导入 300 的轮次分高（取 min 会相等）"
    );
    assert!(
        score_of(&low, "号") > score_of(&p, "号"),
        "抢位广覆盖 ⇒ 加成后的重复词高于纯内置（绝不低过）"
    );
    assert_eq!(
        score_of(&low, "好"),
        score_of(&p, "好"),
        "量纲顶上的条目照旧在其上：加成严格低于顶 ⇒ max 保持原值"
    );
}

/// 精确档语义（`candidates.rs` 的 exact_bonus）不被合并破坏：
/// 打全拼音的精确匹配仍排在前缀扩展之前，哪怕前缀扩展来自导入层且词频压到极限。
#[test]
fn exact_rank_survives_the_layer() {
    let mut e = engine(&[("hao", "好", 100)]);
    e.set_extra_dicts(vec![(
        "前缀表".into(),
        dict(&[("haoma", "好妈", 900_000_000)]),
    )]);
    type_keys(&mut e, "hao");
    let cands = e.candidates(8);
    assert_eq!(
        cands[0].text, "好",
        "精确匹配（hao）必须压过导入层的前缀扩展（haoma）"
    );
    assert_eq!(cands[1].text, "好妈", "前缀扩展仍要出现在候选表里");
}

/// 导入层只叠主库（裁决 3）：繁体模式仍查 trad 库，导入层的词**不可见**；
/// 切回拼音模式立即可见。
#[test]
fn trad_mode_does_not_see_import_layer() {
    let mut e = Engine::with_dictionaries(
        Box::new(dict(&[("hao", "好", 100)])),
        Some(Box::new(dict(&[("hao", "發", 4000)]))),
        no_symbols(),
        false,
    );
    e.set_extra_dicts(vec![("导入表".into(), dict(&[("hao", "壕", 200)]))]);

    type_keys(&mut e, "hao");
    assert!(
        e.candidates(8).iter().any(|c| c.text == "壕"),
        "拼音模式可见"
    );

    // 切模式**清缓冲**是 composer 的既有语义（`Engine::switch_mode`）：每次都重打。
    e.switch_mode(Mode::Traditional);
    type_keys(&mut e, "hao");
    let trad = e.candidates(8);
    assert!(trad.iter().any(|c| c.text == "發"), "繁体模式仍查 trad 库");
    assert!(
        trad.iter().all(|c| c.text != "壕"),
        "导入层不叠繁体库（裁决 3）"
    );

    e.switch_mode(Mode::Pinyin);
    type_keys(&mut e, "hao");
    assert!(
        e.candidates(8).iter().any(|c| c.text == "壕"),
        "切回来立即可见"
    );
}

/// `user_boost` 随导入层**重算**：`set_extra_dicts` 必须调 `recompute_user_boost`
/// —— boost 档 = max(USER_BOOST, **全库**最大原始词频×2)，导入层的数据词频也在
/// 「全库」里。
///
/// ⚠️ 2026-10-17 抢位机制落地后**场景语义已变**（测试随之重写，别照旧注释理解）：
/// 导入数据写多大都被夹在「顶 − 0.8%」带里（`boosted_import_freq`）——「导入高频词
/// 占首位」在新机制下**不再成立**（顶上的条目不可逾越，这是特性不是回归）。
/// 留下的判据是 boost 的**档位**：学一次后目标词的分必须 ≥ 2×全库最大原始词频
/// （= 2×3e9）。变异：`set_extra_dicts` 去掉 `recompute_user_boost()` ⇒ boost 留在
/// 构造期档位 ⇒ 分掉回 1e5 档 ⇒ 本测试红。
#[test]
fn import_high_freq_word_recomputes_user_boost() {
    // learner_enabled = true：本测试要的正是「学一次」这条路径。
    let mut e = Engine::with_dictionaries(
        Box::new(dict(&[("hao", "好", 100), ("hao", "号", 50)])),
        None,
        no_symbols(),
        true,
    );
    e.set_extra_dicts(vec![(
        "高频表".into(),
        dict(&[("hao", "壕", 3_000_000_000)]),
    )]);
    type_keys(&mut e, "hao");
    let before = e.candidates(8);
    assert_eq!(
        before[0].text, "好",
        "新机制：导入数据词频被夹紧，压不过量纲顶上的条目"
    );
    assert!(
        before.iter().any(|c| c.text == "壕"),
        "导入条目仍须在候选表里（只是不占首位）"
    );

    let at = before
        .iter()
        .position(|c| c.text == "好")
        .expect("好 在候选表里");
    assert_eq!(e.select(at), "好");

    type_keys(&mut e, "hao");
    let after = e.candidates(8);
    assert_eq!(after[0].text, "好", "学过的词仍居首");
    assert!(
        score_of(&after, "好") >= 6_000_000_000,
        "user_boost 档位 = 2×全库最大原始词频（含导入层 3e9）——去掉 recompute 会掉回 1e5 档"
    );
}

/// 合并开销实测（契约：分层不能把 `query_prefix_top` 拖慢一个量级）。
///
/// 口径沿用 `symbol_coverage.rs::builtin_build_time_is_reported`：**打印中位数、
/// 不断言绝对时间**（CI 机器负载不定）。A/B 在**同一进程内逐轮交叉**（共享机器的
/// 跨进程对比不可信），三组：
/// - A 无导入层（直通 base）；
/// - B 导入层命中当前前缀（走真合并：逐条查表 + 归并 + 重截断）；
/// - C 导入层存在但不命中当前前缀（快路径：导入层一查为空即直通 base）。
#[test]
fn layer_merge_cost_is_reported() {
    use std::time::Instant;

    const N: usize = 30_000;
    let mut base = InMemoryDictionary::new();
    for i in 0..N {
        base.insert(
            &format!("zh{i:05}"),
            &format!("词{i}"),
            (N as u32) - i as u32,
        );
    }
    let mut extras_hit = InMemoryDictionary::new();
    for i in 0..500 {
        extras_hit.insert(
            &format!("zh{i:05}"),
            &format!("导{i}"),
            10_000_000 - i as u32,
        );
    }
    let mut extras_cold = InMemoryDictionary::new();
    for i in 0..500 {
        extras_cold.insert(&format!("yang{i:04}"), &format!("别{i}"), i as u32);
    }

    let mk = |extras: Option<(&str, InMemoryDictionary)>| {
        let mut e = Engine::with_dictionaries(
            Box::new({
                let mut d = InMemoryDictionary::new();
                for i in 0..N {
                    d.insert(
                        &format!("zh{i:05}"),
                        &format!("词{i}"),
                        (N as u32) - i as u32,
                    );
                }
                d
            }),
            None,
            no_symbols(),
            false,
        );
        if let Some((name, dict)) = extras {
            e.set_extra_dicts(vec![(name.into(), dict)]);
        }
        type_keys(&mut e, "zh");
        e
    };
    let a = mk(None);
    let b = mk(Some(("命中表", extras_hit)));
    let c = mk(Some(("冷表", extras_cold)));

    // 预热（惰性页错误 / 分支预测）。
    let _ = a.candidates(8);
    let _ = b.candidates(8);
    let _ = c.candidates(8);

    let mut sa = Vec::new();
    let mut sb = Vec::new();
    let mut sc = Vec::new();
    for _ in 0..9 {
        let t = Instant::now();
        std::hint::black_box(a.candidates(8));
        sa.push(t.elapsed().as_secs_f64() * 1000.0);
        let t = Instant::now();
        std::hint::black_box(b.candidates(8));
        sb.push(t.elapsed().as_secs_f64() * 1000.0);
        let t = Instant::now();
        std::hint::black_box(c.candidates(8));
        sc.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let median = |v: &mut Vec<f64>| {
        v.sort_by(|x, y| x.partial_cmp(y).unwrap());
        v[v.len() / 2]
    };
    let (ma, mb, mc) = (median(&mut sa), median(&mut sb), median(&mut sc));
    println!(
        "[dict_layers] candidates('zh', 30k base) 中位数：无层 {ma:.3}ms / 层命中 {mb:.3}ms（{:.2}×）/ 层冷 {mc:.3}ms（{:.2}×）",
        mb / ma,
        mc / ma
    );
}

/// 2026-10-17 用户裁决「是词库就要抢位」：导入层条目获得机制性加成 ——
/// 同读音下压过「内置普通候选」，但压不过站在**量纲顶**的内置高频头
/// （加成锚 = 主库最大静态词频：导入条目落在「顶 − 0.8% .. 顶 − 1」的窄带里
/// —— 完整代数见 `boosted_import_freq`；重复项的抬升在 `duplicate_word_takes_max_freq`）。
///
/// 数据侧不再需要写大词频（康熙的 3.98e9 hack 在本裁决后回退成真实小词频）；
/// 这里用 50 这种「真实小词频」当被测量。
#[test]
fn import_entries_take_rank_above_ordinary_builtin() {
    // 内置：一个高频头（40_000 = 本夹具的量纲顶，代表「压不过的那类」）
    // 与一个低频尾巴（5_000，代表「普通候选」）。
    let base = &[("mou", "谋", 40_000), ("mou", "眸", 5_000)];
    let mut plain = engine(base);
    type_keys(&mut plain, "mou");
    let before = plain.candidates(8);
    assert!(
        score_of(&before, "谋") > score_of(&before, "眸"),
        "前置：无导入层时 谋 > 眸"
    );

    let mut e = engine(base);
    e.set_extra_dicts(vec![("方言表".into(), dict(&[("mou", "冇", 50)]))]);
    type_keys(&mut e, "mou");
    let after = e.candidates(8);

    assert!(
        after.iter().any(|c| c.text == "冇"),
        "导入层独有条目必须出候选：{after:?}"
    );
    assert!(
        score_of(&after, "冇") > score_of(&after, "眸"),
        "抢位：导入条目（数据词频仅 50）必须压过内置的低频尾巴 眸"
    );
    assert!(
        score_of(&after, "谋") > score_of(&after, "冇"),
        "边界：「内置高频头」仍在导入条目之上（可压过的是普通候选，不是全部）"
    );
}

/// 抢位的**上界**：学过的词（用户档）永远在导入层之上 —— 包括导入数据里
/// 写了大词频的条目（数据不能把用户档挤下去；与 `user_boost` 重算同一条不变式）。
#[test]
fn learned_words_still_beat_import_layer() {
    let mut e = Engine::with_dictionaries(
        Box::new(dict(&[("mou", "谋", 40_000)])),
        None,
        no_symbols(),
        true, // learner_enabled
    );
    e.set_extra_dicts(vec![(
        "大词频表".into(),
        // 数据写 1e7（超过任何合理静态词频档）—— 加成+夹紧之后仍不得越过用户档。
        dict(&[("mou", "冇", 10_000_000)]),
    )]);
    type_keys(&mut e, "mou");
    let before = e.candidates(8);
    let at = before
        .iter()
        .position(|c| c.text == "谋")
        .expect("谋 在候选表里");
    assert_eq!(e.select(at), "谋");

    type_keys(&mut e, "mou");
    let after = e.candidates(8);
    assert_eq!(
        after[0].text, "谋",
        "学过一次后，无论导入数据写多大词频，用户档仍在最前：{after:?}"
    );
}

/// 2026-10-17 用户裁决「如果响应时间超过 50ms 就优先使用自带词库」：
/// 预算用尽 ⇒ **本轮只出内置结果**（导入层部分整体丢弃），下次查询重新再试
/// （不粘滞）。用 ZERO 预算做**确定性**判据 —— 不靠机器快慢。
#[test]
fn import_scan_over_budget_falls_back_to_builtin_only() {
    let base = &[("mou", "谋", 40_000)];
    let mut e = engine(base);
    e.set_extra_dicts(vec![("方言表".into(), dict(&[("mou", "冇", 50)]))]);

    // 正常预算：导入条目在场（前置，证明探针有效）。
    type_keys(&mut e, "mou");
    assert!(
        e.candidates(8).iter().any(|c| c.text == "冇"),
        "前置：正常预算下导入条目在场"
    );

    // 零预算：必触发兜底 —— 结果与「无导入层」逐条同分同序。
    e.set_import_scan_budget(Some(std::time::Duration::ZERO));
    e.clear();
    let mut plain = engine(base);
    type_keys(&mut plain, "mou");
    type_keys(&mut e, "mou");
    let fell_back = e.candidates(8);
    let baseline = plain.candidates(8);
    assert!(
        fell_back.iter().all(|c| c.text != "冇"),
        "超预算 ⇒ 本轮不出导入层结果：{fell_back:?}"
    );
    assert_eq!(
        fell_back
            .iter()
            .map(|c| (&c.text, c.score))
            .collect::<Vec<_>>(),
        baseline
            .iter()
            .map(|c| (&c.text, c.score))
            .collect::<Vec<_>>(),
        "兜底结果必须与纯内置逐条同分同序"
    );

    // 不粘滞：预算恢复后下一轮立即重新参与。
    e.set_import_scan_budget(None);
    e.clear();
    type_keys(&mut e, "mou");
    assert!(
        e.candidates(8).iter().any(|c| c.text == "冇"),
        "不粘滞：预算恢复后导入层立即回到候选里"
    );
}

/// 9/14 键的导入路径也必须共享同一条预算（2026-10-18 性能审计：旧实现只给
/// `extra_index` 加截止，`scheme_entries` 全表扫没有检查点）。9 键输入 "64" = ni。
#[test]
fn nine_key_scheme_honours_import_budget() {
    let base = &[("ni", "你", 10_000)];
    let mut e = engine(base);
    e.set_scheme(engine_core::scheme::Scheme::Nine);
    e.set_extra_dicts(vec![("方言表".into(), dict(&[("ni", "尼", 50)]))]);
    type_keys(&mut e, "64");
    assert!(
        e.candidates(8).iter().any(|c| c.text == "尼"),
        "前置：正常预算下 9 键候选能看到导入条目"
    );
    e.set_import_scan_budget(Some(std::time::Duration::ZERO));
    e.clear();
    type_keys(&mut e, "64");
    assert!(
        e.candidates(8).iter().all(|c| c.text != "尼"),
        "9 键 scheme 路径超预算也必须整层丢弃导入"
    );
}
