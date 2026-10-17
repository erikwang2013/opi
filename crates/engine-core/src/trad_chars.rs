// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「繁体专属字」表（`data/raw/trad_chars.tsv`，`scripts/gen_trad_chars.py` 生成）+
//! **简体优先**的候选分区。
//!
//! 用户裁决（2026-10-13）：「所有端切换简体就用简体数据，切换繁体就用繁体数据，
//! 默认简体，如果是混合就优先显示简体」。模式→词典的路由本来就有
//! （`Engine::active_dict`：Pinyin → luna、Traditional → trad），这一层管的是
//! **数据里混了另一种字形时谁排前面**。
//!
//! 为什么运行期还要兜底：数据侧本该干净 —— `gen_luna_dict.py` / `gen_extra_phrases.py`
//! 都用 `hanzi_freq.trad_only` **拒收**含繁体专形的条目（简体库不得收繁体字形），
//! 但那份守卫只覆盖 pinyin_simp 那一路；2026-10-11 词库整合（rime-ice 等来源）之后
//! 实测简体库候选里又出现 國/學/車/電（探针见提交信息）。数据修好之前，这一层保证
//! **四端**看到的次序一致：简体在前、繁体在后。
//!
//! 判据沿用仓里那一份（不在 GB2312 且 `kSimplifiedVariant` 指向 GB2312 字），别另造
//! —— 更松的「变体与自身不同」会把 著/覆/乾/藉/瑙/阪 误判成繁体（实测 35 条）。

use crate::candidates::Candidate;

/// 表按码位升序（生成器保证），两列 `繁体字\t简体字` —— 简体列当前不消费，
/// 留着是为了将来做「繁→简」转换时不必再生成一遍（生成器里一次写出来的）。
const TABLE: &str = include_str!("../../../data/raw/trad_chars.tsv");

/// 该字是不是**繁体专属**（有对应的简体字形，且自己不在 GB2312 里）。
///
/// 首次调用时把表解析成升序码位数组（表 ~2.4k 条，二分查；不引依赖、不加锁）。
pub fn is_trad_only(ch: char) -> bool {
    use std::sync::OnceLock;
    static CODES: OnceLock<Vec<u32>> = OnceLock::new();
    let codes = CODES.get_or_init(|| {
        let mut v: Vec<u32> = TABLE
            .lines()
            .filter_map(|l| l.split('\t').next())
            .filter_map(|c| c.chars().next())
            .map(|c| c as u32)
            .collect();
        v.sort_unstable(); // 生成器已升序；这里再排一次，改生成器也不会让查表失效
        v
    });
    codes.binary_search(&(ch as u32)).is_ok()
}

/// 文本里有没有繁体专属字（词组只要一个字命中就算）。
pub fn contains_trad_only(text: &str) -> bool {
    text.chars().any(is_trad_only)
}

/// **简体优先**（`prefer_simplified`）：在**同一档位内**把含繁体专属字的候选**稳定地**排后。
///
/// 「同一档位」用 `exact_floor` 划（调用方传 `boost / 2` = 引擎的精确加成量）：
/// **`score > floor` = 精确命中那一档**（它吃到 +boost/2），`score <= floor` = 前缀扩展 /
/// 模糊 / 简拼 / 逐音节那些档（**≤ max_freq**，见 `candidates.rs` 的不变量
/// 「任何候选 ≤ max_freq ≤ boost/2 = exact_bonus」）。本函数只在这个次序**内部**再按简繁排。
///
/// ⚠️ 边界取 `<=`（不是 `<`）：**模糊档里会出现恰好等于 floor 的**——那条不变量的等号就是
/// 「该候选的静态词频 = 全库最大词频」的情形（`fuzzy_ranking` 夹具里「你」正是如此：
/// freq = max_freq = boost/2，naive 的 `<` 会把它当精确档，然后把精确的「裏」压下去 ⇒ 那条门禁红）。
/// 代价如实记：freq = 0 的**精确**命中其 score 也等于 floor（0 + boost/2），会被归进下一档
/// —— 它本来就是分数最低的那条，只是「繁体且 freq=0」时可能再被简体模糊命中越过去。
///
/// ⚠️ **不分档就分区会踩既有门禁**（2026-10-13 实测三条红）：`fuzzy_ranking` 钉着
/// 「变体命中不得冒充精确位」—— 打 `li` 时把精确的「裏」（繁体专形）压到模糊命中
/// 「你」（简体）之下，那条当场红。用户裁决是**简体优先**，不是**简体压过一切**。
pub fn demote_traditional_by<T>(
    items: &mut [T],
    exact_floor: u64,
    score_of: impl Fn(&T) -> u64,
    text_of: impl Fn(&T) -> &str,
) {
    if items.len() < 2 {
        return;
    }
    // 稳定排序 + 元组键：只在（档位, 简繁）上分组，组内保持**原名次**（= 既有分数序）。
    // cached_key：`contains_trad_only` 是扫字符 + 二分表的活，按项算一次即可
    // （sort_by_key 每次比较重算，O(n log n) 次；2026-10-18 性能审计）。
    items.sort_by_cached_key(|it| (score_of(it) <= exact_floor, contains_trad_only(text_of(it))));
}

/// [`demote_traditional_by`] 的候选版（符号页那类单层调用）。
pub fn demote_traditional(cands: &mut [Candidate], exact_floor: u64) {
    demote_traditional_by(cands, exact_floor, |c| c.score, |c| &c.text);
}
