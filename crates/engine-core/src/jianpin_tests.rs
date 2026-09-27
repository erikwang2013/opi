// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `jianpin.rs` 的单元测试（`#[path]` 引入，见该文件尾部）：单测独立成文件以保持
//! 源文件 <500 行，与 `candidates_tests.rs` / `router_tests.rs` 同惯例。
//! 引擎级门禁（缩写能到、只追加、上限）在 `tests/jianpin_ranking.rs`。

use super::*;
use crate::candidates::USER_BOOST;
use crate::dictionary::InMemoryDictionary;

/// 每位置音节数由位置数反推，且 `cap^k` **真的**不越界。
/// 断言的是性质本身（逐步乘到越界为止），不是 `cap.pow(k.min(5))` —— 后者跟着实现一起
/// 截断指数，实现把指数截断时它照样绿（上限 16 那版就是如此，见 `grid_fits`）。
#[test]
fn per_position_cap_never_breaks_the_ceiling() {
    assert_eq!(
        per_position_cap(2),
        22,
        "22^2 = 484 ≤ 512（23^2 = 529 越界）"
    );
    assert_eq!(per_position_cap(3), 8, "8^3 = 512");
    assert_eq!(per_position_cap(4), 4, "4^4 = 256（5^4 = 625 越界）");
    assert_eq!(per_position_cap(5), 3, "3^5 = 243");
    assert_eq!(per_position_cap(6), 2, "2^6 = 64");
    for k in 2..=40 {
        let cap = per_position_cap(k);
        assert!(cap >= 1, "k={k}");
        let mut acc: usize = 1;
        for _ in 0..k {
            acc = acc
                .checked_mul(cap)
                .unwrap_or_else(|| panic!("k={k} cap={cap}: 网格溢出 usize"));
        }
        assert!(
            acc <= MAX_EXPANSIONS,
            "k={k} cap={cap}: 网格 {acc} 突破上限 {MAX_EXPANSIONS}"
        );
    }
    assert_eq!(per_position_cap(9), 2, "2^9 = 512，恰好铺满");
    assert_eq!(per_position_cap(10), 1, "2^10 = 1024 越界");
    assert_eq!(per_position_cap(64), 1);
}

/// 排的是**该音节自身键**的证据，不是「该前缀下最大词频」：查询是前缀语义，短音节会白拿
/// 长音节的词。真库实测有这种「幽灵音节」（`za`←`zai`、`zha`←`zhan`…，判据 = 该音节**最高分
/// 条目**的 `pinyin_len ≠ 音节长度`），幽灵按继承来的高分插榜会把真音节挤下去
/// （`zhong` 被挤出靠前的档 ⇒ `zg` 到不了「中国」）。
///
/// ⚠️ **别在这里写幽灵的个数**（2026-09-28 订正）：原文写「真库实测 `z` 的前 16 个音节里 4 个
/// 是这种幽灵」，实测不复现 —— 个数随 `cap` 变（榜越长越容易进幽灵），且「16」是 `cap=16` 的
/// **榜长**、不是音节的个数。机制本身实测为真，别因为数字不对去动它
/// （同一句的订正说明另见 `jianpin.rs::best_score`）。
#[test]
fn top_syllables_rank_own_evidence_before_inherited() {
    let mut d = InMemoryDictionary::new();
    d.insert("na", "那", 900);
    d.insert("neng", "能", 700); // 「ne」不是自身键，只是前缀
    d.insert("nihao", "你好", 100); // 「ni」只有继承来的证据
    let l = Learner::new(false);
    // 幽灵「ne」从 neng 继承 700 ⇒ 与 neng 同分；不分开尺度时同分按音节升序
    // ⇒ 「ne」在前，「neng」被挤出 cap=2 的榜（真库里发生的就是这件事）
    assert_eq!(
        top_syllables(&d, &l, USER_BOOST, "n", 2),
        vec!["na", "neng"],
        "幽灵音节「ne」（只从前缀 neng 继承证据）挤掉了真音节「neng」"
    );
    // 空音节（nai/nan/… 一条都没有）不占位；继承来的证据仍排在所有自身键之后。
    // 「ne」与「nen」**都是** `neng` 的前缀 ⇒ 两个幽灵各带 700 跟在 neng 后面（同分按音节
    // 升序）；「ni」只从 `nihao` 继承到 100。
    assert_eq!(
        top_syllables(&d, &l, USER_BOOST, "n", 8),
        vec!["na", "neng", "ne", "nen", "ni"],
        "自身键（na/neng）必须整体排在继承键（ne/nen/ni）之前"
    );

    // 同分按音节升序（两个词库实现必须给出同一份候选）：ga 与 ge 同为 900
    let mut tie = InMemoryDictionary::new();
    tie.insert("ge", "个", 900);
    tie.insert("ga", "嘎", 900);
    assert_eq!(
        top_syllables(&tie, &l, USER_BOOST, "g", 8),
        vec!["ga", "ge"]
    );
    assert_eq!(
        top_syllables(&tie, &l, USER_BOOST, "q", 8),
        Vec::<&str>::new()
    );
}

/// 排序键是**含学习权重的最终分**。用 `query(syl, 1)`（静态前 1）就会在加 boost 之前
/// 把学过的词丢掉 —— 与 candidates.rs 逐音节回退那条同一条不变式。
#[test]
fn syllable_ranking_uses_learned_weights() {
    let mut d = InMemoryDictionary::new();
    d.insert("na", "那", 50_000);
    d.insert("ni", "你", 1); // 静态垫底
    let mut l = Learner::new(true);
    l.record_selection("你");
    assert_eq!(
        top_syllables(&d, &l, USER_BOOST, "n", 1),
        vec!["ni"],
        "学过的「你」该把音节 ni 抬到第一"
    );
    assert_eq!(
        top_syllables(&d, &Learner::new(false), USER_BOOST, "n", 1),
        vec!["na"],
        "学习关闭时静态词频说了算"
    );
}

/// `n`/`h` 两个字母各有 2 个音节（未铺满 cap=8 ⇒ 网格 2×2）。
fn sparse_dict() -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    for (py, w, f) in [
        ("ni", "你", 3_999_128_899u32),
        ("na", "那", 3_995_000_000),
        ("hao", "好", 3_997_635_583),
        ("he", "和", 3_992_000_000),
        ("nihao", "你好", 47_594),
        ("nahe", "哪和", 3),
    ] {
        d.insert(py, w, f);
    }
    d
}

/// 展开键的确定顺序：里程表进位（最左音节变化最快，同 `fuzzy::input_variants`），
/// 且每个键恰好出现一次。
///
/// h 组是 3 个音节（hao/he 是自身键，`ha` 从前缀 `hao` 继承到「好」的词）⇒ 网格 2×3，
/// 后两个键是 `niha`/`naha`。**别按「3 个音节不对」去修** —— 那正是继承证据该有的样子，
/// 门禁管的是「自身键优先」而非「幽灵不进榜」（见 `top_syllables_rank_own_evidence_*`）。
#[test]
fn expansions_are_deterministic_and_unique() {
    let d = sparse_dict();
    let l = Learner::new(false);
    let got = expansions(&d, &l, USER_BOOST, "nh");
    assert_eq!(
        got,
        vec!["nihao", "nahao", "nihe", "nahe", "niha", "naha"],
        "里程表进位应最左最快（ni 配遍 h 组，再轮到 na）：{got:?}"
    );
    let mut uniq = got.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(uniq.len(), got.len(), "展开键重复：{got:?}");
    // 展开键一律是 ≥2 个合法音节的拼接（查库串不得是凭空拼的字母）
    for v in &got {
        assert!(!is_syllable(v), "{v} 竟是单个音节");
        let parts = segment(v);
        assert_eq!(parts.len(), 2, "{v} 不是两个音节的拼接：{parts:?}");
        assert!(parts.iter().all(|p| is_syllable(p)), "{v} 含非法音节");
    }
}

/// `'` 不挡简拼：`n'h` 是用户手写的切分，展开它正是那个切分想表达的缩写。
/// （fuzzy.rs 相反 —— 那里 `'` 表达「别动我的切分」，简拼是**缩写**不是替换，不冲突。）
#[test]
fn apostrophe_is_a_valid_abbreviation_split() {
    let d = sparse_dict();
    let l = Learner::new(false);
    let got = expansions(&d, &l, USER_BOOST, "n'h");
    assert_eq!(
        got,
        expansions(&d, &l, USER_BOOST, "nh"),
        "`'` 不该改变展开"
    );
    assert!(!got.is_empty());
}
