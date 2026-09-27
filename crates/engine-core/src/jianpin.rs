// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 简拼（声母缩写）：`zg` → `zhongguo`、`nh` → `nihao`，与全拼**映射到同一批候选**。
//!
//! **常开、无开关**（与 fuzzy.rs 同源理由）：fcitx5（`Configurable=False`）与 TSF（无配置面）
//! 两端没有配置面，做成开关只会让两端无法关闭或恒开。故本模块无状态、`rank_and_pick`
//! 签名不动。
//!
//! **触发条件 = 纯缩写**：`segment` 切出的每一段都是单字母，且**都不是完整音节**。
//! 后者排除 `a`/`e`/`o` —— 它们是真音节（打 `a` 要的是「啊」），不是 `ai/an/ang` 的缩写。
//! 另要求 ≥ 2 段：单字母输入不展开，否则最贵的一次击键（扫该字母下的全部音节）
//! 会落在每个词的第一键上。混合输入（`nih` = 「ni」+「h」）也不展开 —— 那是另一套语义。
//! `'` 不挡：`n'h` 是用户手写的切分，展开它正是那个切分想表达的缩写。
//!
//! **展开在字符串层面**：单字母 → 它以开头的合法音节（由 `SYLLABLES` 过滤得到，**不新增表、
//! 不改 `segment`**）。那张表被 410 条计数断言、贪婪切分断言、全音节全前缀的二分单调性
//! 穷举锁着（pinyin.rs / tests/adversarial_input.rs），扩表会连带推翻它们。
//!
//! **每位置取哪几个音节**：按「该音节**自身键**下最优候选的最终分」降序，自身键证据整体
//! 优先于继承来的证据（`best_score` 的两个尺度）—— 与全拼路径同一个排序键，学过的词因此
//! 也能把它的音节抬上来。查不出任何条目的音节直接排除：它们只会白占一个网格位、多一次查库。
//!
//! **上限**：k 个位置 × 每位置 cap 个音节是 cap^k 的笛卡尔积，而每个展开键都是一次词库
//! 前缀查询 —— 这条路在**每次击键**上，必须封顶（与 `fuzzy::MAX_VARIANTS` 同源理由）。
//! 落实方式与那里不同：fuzzy 是「枚举到 16 个就停」，这里是**按位置数反推每位置取几个**
//! （见 [`per_position_cap`]），于是枚举的永远是**完整网格**。「前 16 个」式的截断会系统性
//! 偏向靠前的音节：打 `zg` 只会试 `za`+…，`zhongguo` 连试都试不到。
//!
//! 上限取 512（2 个位置 → 每位置 22；3 → 8；4 → 4；5 → 3；6~9 → 2；≥10 → 1），依据是真库实测：
//!
//! - **覆盖率**（真库前 100 个**真**双音节词，判据 = 两个音节都在各自字母的自键榜前 cap）：
//!   cap4 = 41/100、cap8 = 73/100、cap16 = 99/100。任务的头号用例 `zg` 需要 z 的第 5 个音节
//!   `zhong`（cap4 时到不了），`bj` 需要 b 的第 9 个 `bei`（cap8 也到不了）；3 个字母的
//!   `nhm` →「你好吗」需要 m 的第 7 个音节 `ma`，正是 cap≥7 那一档（2 个字母的上限反推不出）。
//! - **成本**：一次完整构建（逐音节扫描 + 连接键查询 + 候选构造 + 排序）实测
//!   cap4 ≈ 2.9~3.5ms、cap8 ≈ 3.1~3.4ms、cap16 ≈ 3.3~3.5ms、cap22 ≈ 3.6~4.2ms（区间是两轮
//!   探针的跨度 —— 同机 11 个并发分身，同一输入轮间差 ±30%，只有量级与趋势可信）。
//!   大头是**逐音节扫描**（37 个 z- 音节、19 个 g- 音节各一次 `query_all`）且与 cap 无关；
//!   cap 的边际（连接键查询 ~1.5µs/个 + 候选构造）在 64→418 键上是 +0.2~1ms。
//!   既然边际远小于固定成本、而覆盖率的拐点在 16 附近，上限就该按**覆盖率**取。
//!
//! 这些数字来自临时探针（已删），不是门禁 —— 真库下目标词可能超 `FETCH_LIMIT`，那种断言是
//! 假绿。门禁只管「上限存在且组合数不越界」（`tests/jianpin_ranking.rs`）。
//!
//! ⚠️ 简拼档的可见性靠**精确档为空**：`zg`/`nh` 这类纯缩写在词库里（拼音键都是音节序列）
//! 查不出任何东西，故简拼档直接成为主结果。混合输入下它只能追加在精确档之后、可能落在
//! `FETCH_LIMIT` 之外 —— 那不是本档的问题（分档规则与模糊同源），别据此改。

use crate::candidates::rank_score;
use crate::dictionary::Dictionary;
use crate::learner::Learner;
use crate::pinyin::{SYLLABLES, is_syllable, segment};

/// 简拼展开键的条数上限（= 每次击键的词库前缀查询次数上限）。
pub const MAX_EXPANSIONS: usize = 512;

/// 位置数 → 每位置取几个音节：满足 `cap^positions <= MAX_EXPANSIONS` 的**最大** cap。
/// 2 → 22（484）；3 → 8（512）；4 → 4（256）；5 → 3（243）；6~9 → 2；≥10 → 1。
///
/// `pub` 是为了让引擎级门禁能算出「这次该查多少个键」（网格 = `∏ min(cap, 该字母音指数)`），
/// 而不是把 cap 再抄一遍 —— 那些**数值**由 `jianpin_tests.rs::per_position_cap_*` 钉住。
pub fn per_position_cap(positions: usize) -> usize {
    let mut cap = MAX_EXPANSIONS;
    while cap > 1 && !grid_fits(cap, positions) {
        cap -= 1;
    }
    cap
}

/// `cap^positions <= MAX_EXPANSIONS`。逐步乘、越界即停 —— 直接写 `cap.pow(positions)`
/// 在 8 个位置上就溢出了。
///
/// ⚠️ 这里**不能**截断指数（曾写 `positions.min(5)`，理由是「≥5 个位置时 cap 必为 1」——
/// 那只在 `MAX_EXPANSIONS = 16` 时成立）。上限 512 时 2 个位置的 cap 是 22，截断指数会让
/// 7/8 个位置算出 cap=3 ⇒ 网格 2187/6561，**上限静默失效**。门禁那条
/// `per_position_cap_*` 当初跟着实现一起截断指数，所以是假绿。
fn grid_fits(cap: usize, positions: usize) -> bool {
    let mut acc: usize = 1;
    for _ in 0..positions {
        match acc.checked_mul(cap) {
            Some(v) if v <= MAX_EXPANSIONS => acc = v,
            _ => return false,
        }
    }
    true
}

/// 简拼整串展开（**不含输入自身**）：每位置一个音节的笛卡尔积，最左音节变化最快
/// （顺序确定，同 `fuzzy::input_variants`）。非纯缩写输入返回空。
///
/// 返回的串可直接拿去查库；调用方负责把它们当**非精确**档处理（见 candidates.rs）。
pub fn expansions<D: Dictionary + ?Sized>(
    dict: &D,
    learner: &Learner,
    boost: u64,
    input: &str,
) -> Vec<String> {
    let syls = segment(input);
    if syls.len() < 2
        || !syls
            .iter()
            .all(|s| s.chars().count() == 1 && !is_syllable(s))
    {
        return Vec::new();
    }
    let cap = per_position_cap(syls.len());
    // 同一个字母只扫一次：扫描是这条路的主要成本（真库 37 个 z- 音节 ≈ 2~3 ms），而
    // 「等等/谢谢/妈妈/天天」这类**重复音节**的缩写很常见 —— 不合并就是把同一组白扫两遍。
    // （不同字母仍各扫一次，这是机制本身的成本，见模块头的实测数字。）
    let mut uniq: Vec<(&str, Vec<&'static str>)> = Vec::new();
    let mut per: Vec<Vec<&'static str>> = Vec::with_capacity(syls.len());
    for s in &syls {
        match uniq.iter().position(|(l, _)| *l == s.as_str()) {
            Some(i) => per.push(uniq[i].1.clone()),
            None => {
                let list = top_syllables(dict, learner, boost, s, cap);
                uniq.push((s.as_str(), list.clone()));
                per.push(list);
            }
        }
    }
    // 任一位置没有候选（该字母在词库里没有任何音节）⇒ 不展开：凭空拼出的串只会白查一次库。
    if per.iter().any(Vec::is_empty) {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut idx = vec![0usize; per.len()];
    loop {
        out.push(per.iter().zip(&idx).map(|(v, i)| v[*i]).collect::<String>());
        // 里程表进位（最左最快），进位溢出即穷尽。网格是完整的（`per_position_cap` 保证
        // `cap^k <= MAX_EXPANSIONS`），故这里不截断、也不计数。
        let mut k = 0;
        loop {
            if k == per.len() {
                return out;
            }
            idx[k] += 1;
            if idx[k] < per[k].len() {
                break;
            }
            idx[k] = 0;
            k += 1;
        }
    }
}

/// 该字母开头的音节里按 [`best_score`] 取前 `cap` 个（降序，同分按音节升序 ——
/// 两个词库实现必须给出同一份候选，见 `opid_robustness.rs`）。
fn top_syllables<D: Dictionary + ?Sized>(
    dict: &D,
    learner: &Learner,
    boost: u64,
    letter: &str,
    cap: usize,
) -> Vec<&'static str> {
    let mut scored: Vec<((u8, u64), &'static str)> = SYLLABLES
        .iter()
        .filter(|s| s.starts_with(letter))
        .filter_map(|s| best_score(dict, learner, boost, s).map(|score| (score, *s)))
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
    scored.truncate(cap);
    scored.into_iter().map(|(_, s)| s).collect()
}

/// 该音节下最优候选的最终分（含学习权重），返回 `(是否自身键, 分数)`。
///
/// **自身键的证据优先**，两个尺度必须分开排：查询是前缀语义，短音节会白拿长音节的词 ——
/// `za` 继承 `zai`/`zhan`（真库实测同样 3999750886），`ha` 继承 `hao`，`gu` 继承 `guo`。
/// 不分开的话这些「幽灵音节」按继承来的高分插进榜，把真正的音节挤掉（`zhong` 被挤出靠前的
/// 档 ⇒ `zg` 到不了「中国」）。
///
/// ⚠️ **别在这里写幽灵的个数**（2026-09-28 订正）：原注释写「真库实测：`z` 的 16 个入榜音节里
/// 幽灵占 4 个」，实测不复现 —— 幽灵个数随 `cap` 变（榜越长越容易进幽灵），且当时那一档也不是
/// 4；「16」说的是 `cap=16` 的**榜长**，不是音节的个数，原文把它当成了「16 个音节」。机制本身
/// （分开尺度挡住幽灵）已实测为真、变异有牙，别因为数字不对去动它 —— 要钉就钉「自身键优先」
/// 这条行为（`jianpin_tests.rs::top_syllables_rank_own_evidence_before_inherited`，同一句
/// 数字在它的文档里也有一份）。
///
/// 用 `query_all` 而不是 `query(syl, 1)`：后者截的是**静态**词频前 1，学过的词在加 boost
/// 之前就被丢掉了（与 candidates.rs 逐音节回退那条是同一条不变式，那里栽过）。
fn best_score<D: Dictionary + ?Sized>(
    dict: &D,
    learner: &Learner,
    boost: u64,
    syllable: &str,
) -> Option<(u8, u64)> {
    let mut best: Option<(u8, u64)> = None;
    // 前缀查询下 `pinyin_len == syllable.len()` ⟺ 键恰好就是这个音节。
    for e in dict.query_all(syllable) {
        let cand = (
            u8::from(e.pinyin_len == syllable.len()),
            rank_score(e.freq, learner.freq_of(&e.word), boost),
        );
        if best.is_none_or(|b| cand > b) {
            best = Some(cand);
        }
    }
    best
}

// 单测独立成文件（`#[path]` 引入）以保持本文件 <500 行，与 candidates.rs / learner.rs 同惯例。
#[cfg(test)]
#[path = "jianpin_tests.rs"]
mod tests;
