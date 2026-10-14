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
    Engine::with_dictionaries(Box::new(dict(base)), None, no_symbols(), false)
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

/// 同 (拼音, 词) 与系统库重复时取**最大**词频 —— 两个方向各一条：
/// 导入层更低频不压下去、更高频要顶上来。变异成「取最小」⇒ 本测试必红。
#[test]
fn duplicate_word_takes_max_freq() {
    let base = &[("hao", "好", 5000), ("hao", "号", 50)];
    let mut control = engine(base);
    type_keys(&mut control, "hao");
    let plain = control.candidates(8);

    let mut e = engine(base);
    // 同词重复：好 是「导入层更低」（5000 必须保留），号 是「导入层更高」（9999 必须生效）。
    e.set_extra_dicts(vec![(
        "对比表".into(),
        dict(&[("hao", "好", 300), ("hao", "号", 9999)]),
    )]);
    type_keys(&mut e, "hao");
    let merged = e.candidates(8);

    // boost 两库相同：全局最大词频 5000 vs 9999，都 < USER_BOOST/2 ⇒ 同为下限档。
    assert_eq!(
        score_of(&merged, "好"),
        score_of(&plain, "好"),
        "取最大词频 ⇒ 导入层的低频重复不得把系统库的 5000 压成 300"
    );
    assert_eq!(
        score_of(&merged, "号"),
        score_of(&plain, "号") + (9999 - 50),
        "取最大词频 ⇒ 导入层的高频重复必须顶上来"
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

/// `user_boost` 随导入层**重算**：导入一个词频高到抬 boost 的词后，
/// 「一次选词仍能压过全部静态词」的不变量必须保持。
/// 变异：`Engine::set_extra_dicts` 去掉 `recompute_user_boost()` ⇒ 本测试必红
/// （boost 留在 USER_BOOST=1e5，学习词 1×1e5 压不过 3e9 的静态词）。
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
    assert_eq!(before[0].text, "壕", "学习前：导入层的高频词就是首位");

    let at = before
        .iter()
        .position(|c| c.text == "好")
        .expect("好 在候选表里");
    assert_eq!(e.select(at), "好");

    type_keys(&mut e, "hao");
    let after = e.candidates(8);
    assert_eq!(
        after[0].text, "好",
        "导入高频词把 boost 抬到 2×3e9 后，一次选词仍要压过全部静态词"
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
