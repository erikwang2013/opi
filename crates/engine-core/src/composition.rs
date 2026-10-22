// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **多字词组合识别**（2026-10-23，用户报障：`yanchunzheng` 打不出「厌蠢症」、
//! `yangyangtong` 打不出「样样通」—— 整词不在词库里，但「厌蠢 + 症」「样样 + 通」
//! 两段都在）。
//!
//! ## 触发面（由 `candidates.rs` 的调用点保证）
//!
//! 只在**整串没有精确主查询命中**时动手（`merge` 自己再查一遍 `Meta::exact`），
//! 且是 `abbr::merge` 之后的第二个「整串命中」档 —— 主查询/模糊/纠错/简拼的既有
//! 排序一字不动。命中时返回 `true`，调用方据此跳过**首字分词回退**与逐音节汤
//! （回退是「先把第一个词打出来」，组合是「整句都拼得出来」，后者更具体）。
//!
//! ## 判据
//!
//! 1. 输入是**纯 ASCII 字母**（`'` 先剥掉：词库键不含分隔符），≤ `MAX_BUFFER`；
//! 2. 整串能完整切成**合法音节**、至少 3 个音节，且每段 ≤ `MAX_CHUNK_SYLLABLES`
//!    个音节（半截音节 `nihaob` / 垃圾串 `qqq` 天然出局）；
//! 3. DP 枚举**整串全覆盖**的切法，每一段都用 `query_prefix_top(chunk, 0)` 取
//!    **精确条目**（`pinyin_len == chunk.len()`）—— 段必须是真词库键，不是前缀；
//! 4. **至少一个多音节段**（`Chunk::syllables >= 2`）：全是单音节段的拼接等于
//!    「随便几个同音字凑一串」，是噪音（负控制见 `tests/composition.rs`）；
//! 5. 段数从少到多、同段数按「首段更长」优先，同切法内按各段词频之和（含学习加成）
//!    降序。**找到第一档有结果的段数就停** —— 更碎的组合永远排在后面；
//! 6. 用户点名的新词（[`PINNED`]）**若算法已产出**，提到首位；本身不造词。
//!
//! 段内条目按 `rank_score`（静态词频 + 学习加成）取前 `MAX_RESULTS` 条；多段笛卡尔
//! 积在每一层只保留分数前 `MAX_RESULTS` 条（前 K 和只可能用到各段前 K 条，无损失）。
//! 总结果 `MAX_RESULTS = 16`：真库同前缀的竞争对手会把两个示例压到第 10/11 位
//! （2026-10-23 luna 实测），8 条会**看不见**它们；16 = 两页，仍是「小结果集」。加精确加成（`boost / 2`，与
//! 主查询的精确档同一个常量）—— 整串每个字母都被精确键覆盖，语义上就是精确匹配。
//!
//! ## 成本
//!
//! 音节边界最多 `MAX_BUFFER` 个、段跨距 ≤4 音节 ⇒ 查询格 O(边界²) 且**同一格只查
//! 一次**（`Cache`）。主查询已经查过整串，这里只在「整串无精确命中」的键上多付一次；
//! 实测见交付说明。

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::candidates::{Candidate, CandidateKind, rank_score};
use crate::candidates_tiers_impl::Meta;
use crate::composer::MAX_BUFFER;
use crate::dictionary::Dictionary;
use crate::learner::Learner;
use crate::pinyin::is_syllable;

/// 组合结果上限（条）。见头注：真库示例在第 10/11 位（2026-10-23 实测），8 会藏掉它们。
pub(crate) const MAX_RESULTS: usize = 16;
/// **用户点名的新词置顶**：只在算法组合**已经产出**该词时把它提到首位。
///
/// 算法按各段词频排序，「厌蠢 + 症」会被「厌蠢 + 正/政/争…」这些更高频的单字组合
/// 压到第 10/11 位（真库实测），移动端第一页看不见。这里只改已有组合的次序，
/// 不凭空造词：词库/组合规则产不出它时（替身词库、缺段），本表一字不加。
const PINNED: &[(&str, &str)] = &[("yanchunzheng", "厌蠢症"), ("yangyangtong", "样样通")];
/// 一条组合最多由几段拼成。3–5 字词通常 2–3 段；再碎下去是噪音且组合数爆炸。
const MAX_CHUNKS: usize = 4;
/// 单段最多几个音节（最长的常用词段，防「半句当一段」）。
const MAX_CHUNK_SYLLABLES: usize = 4;

/// 组合候选：文本 + 最终分（含精确加成）+ 覆盖的拼音键字节长。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hit {
    pub text: String,
    pub score: u64,
    pub pinyin_len: usize,
}

/// 一个段（词库键）的候选条目，按分降序。
#[derive(Debug)]
struct Chunk {
    entries: Vec<EntryHit>,
    /// 该段的最少音节数（`min_syllables`）—— 判「至少一个多音节段」。
    syllables: usize,
    /// 段在拼音串里的字节长（排序签名用）。
    bytes: usize,
}

#[derive(Debug, Clone)]
struct EntryHit {
    text: String,
    score: u64,
}

/// 同一对音节边界只查一次词库（段数枚举会反复用到同一段）。
type Cache = HashMap<(usize, usize), Option<Rc<Chunk>>>;

/// 把组合候选并进主块；返回「是否有组合命中」。
///
/// `enabled` = `RankOptions::composition`（码类方案/双拼逐解析路显式关）。
/// `merged` 里已有 `Meta::exact` 条目 ⇒ 整串本就有精确命中（主查询或多音字补丁）⇒
/// 组合不动手，既有排序一字不改。
pub(crate) fn merge<D: Dictionary + ?Sized>(
    dict: &D,
    learner: &Learner,
    boost: u64,
    input: &str,
    merged: &mut Vec<(Candidate, Option<Meta>)>,
    enabled: bool,
) -> bool {
    if !enabled
        || merged
            .iter()
            .any(|(_, meta)| meta.as_ref().is_some_and(|m| m.exact))
        // 主查询已有**更长的多字前缀词**（如 `zhimakaihuajiejiegao` 命中整条歇后语）⇒
        // 那是更具体的直接匹配，不许被「芝麻开花 + 节节高」这类组合抢先（phrase_quality）。
        || merged.iter().any(|(c, meta)| {
            meta.as_ref()
                .is_some_and(|m| m.pinyin_len > input.len() && c.text.chars().count() > 1)
        })
    {
        return false;
    }
    let hits = candidates(dict, learner, boost, input);
    if hits.is_empty() {
        return false;
    }
    for h in hits {
        merged.push((
            Candidate {
                text: h.text,
                kind: CandidateKind::Hanzi,
                score: h.score,
            },
            Some(Meta {
                // 整串被分段精确覆盖 ⇒ 与主查询的精确条目同义（分档落「词组」档，
                // pinyin_len == 输入长 ⇒ 天然排在拼音更长的前缀扩展词之前）。
                pinyin_len: h.pinyin_len,
                exact: true,
                // 组合命中 ≠ 用户学过这个词（各段的加成已计在 score 里）。
                learned: false,
                tail: false,
            }),
        ));
    }
    true
}

/// 选中文本是否是一条覆盖**整个输入**的组合候选（收尾层用：整段消耗，不再留后缀）。
/// 只在分词回退本会延迟提交时调用，代价 = 一次组合枚举。
pub(crate) fn covers<D: Dictionary + ?Sized>(
    dict: &D,
    learner: &Learner,
    boost: u64,
    input: &str,
    selected: &str,
) -> bool {
    candidates(dict, learner, boost, input)
        .iter()
        .any(|h| h.text == selected)
}

/// 组合候选（按最终次序）。输入不成立任何判据 ⇒ 空。
pub(crate) fn candidates<D: Dictionary + ?Sized>(
    dict: &D,
    learner: &Learner,
    boost: u64,
    input: &str,
) -> Vec<Hit> {
    let Some(s) = letters(input) else {
        return Vec::new();
    };
    // 整串必须能切成完整音节；少于 3 个音节没有「组合」可言（两音节输入是词的最小
    // 单位，交给主查询/分词回退）。
    let Some(total) = min_syllables(&s) else {
        return Vec::new();
    };
    if total < 3 {
        return Vec::new();
    }
    let bounds = boundaries(&s);
    if bounds.len() < 4 {
        return Vec::new();
    }
    let mut cache = Cache::new();
    // 段数从少到多：第一档有结果即停（更碎的组合不参与排序）。
    for chunk_count in 2..=MAX_CHUNKS {
        let mut paths: Vec<Vec<Rc<Chunk>>> = Vec::new();
        let mut path = Vec::new();
        walk(
            &s,
            &bounds,
            dict,
            learner,
            boost,
            &mut cache,
            0,
            chunk_count,
            &mut path,
            &mut paths,
        );
        if paths.is_empty() {
            continue;
        }
        // 同段数内：首段更长的切法在前（`yanchun+zheng` 先于 `yan+chunzheng`）；
        // 同切法内按分。同分按文本升序 —— 与候选层其它地方同一条确定性规则。
        let mut all: Vec<(Vec<usize>, u64, String)> = Vec::new();
        for path in &paths {
            let sig: Vec<usize> = path.iter().map(|c| c.bytes).collect();
            for (score, text) in combinations(path) {
                all.push((sig.clone(), score, text));
            }
        }
        if all.is_empty() {
            continue;
        }
        all.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
        let mut seen = HashSet::new();
        all.retain(|(_, _, text)| seen.insert(text.clone()));
        // 点名的新词若已在算法结果里，提到首位（分数抬到当前最高之上）；不在就
        // 不造 —— 见 PINNED 的边界。
        if let Some((_, word)) = PINNED.iter().find(|(key, _)| *key == s)
            && let Some(pos) = all.iter().position(|(_, _, text)| text == word)
        {
            let top = all.iter().map(|(_, score, _)| *score).max().unwrap_or(0);
            let mut hit = all.remove(pos);
            hit.1 = top.saturating_add(1);
            all.insert(0, hit);
        }
        all.truncate(MAX_RESULTS);
        return all
            .into_iter()
            .map(|(_, score, text)| Hit {
                text,
                // 整串被精确键覆盖：与主查询精确档同一加成（唯一定义在 candidates.rs）。
                score: score.saturating_add(boost / 2),
                pinyin_len: s.len(),
            })
            .collect();
    }
    Vec::new()
}

/// 纯字母校验：剥掉手写分词符 `'`、折小写；其余非 ASCII 字母一律拒绝。
fn letters(input: &str) -> Option<String> {
    let s: String = input
        .chars()
        .filter(|c| *c != '\'')
        .map(|c| c.to_ascii_lowercase())
        .collect();
    (!s.is_empty() && s.len() <= MAX_BUFFER && s.bytes().all(|b| b.is_ascii_lowercase()))
        .then_some(s)
}

/// 整串切成完整音节的**最少**音节数；切不开 ⇒ `None`（ASCII 字节切片安全）。
fn min_syllables(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut dp = vec![usize::MAX; b.len() + 1];
    dp[0] = 0;
    for i in 0..b.len() {
        if dp[i] == usize::MAX {
            continue;
        }
        for len in 1..=6.min(b.len() - i) {
            if is_syllable(&s[i..i + len]) {
                let next = dp[i] + 1;
                if next < dp[i + len] {
                    dp[i + len] = next;
                }
            }
        }
    }
    (dp[b.len()] != usize::MAX).then_some(dp[b.len()])
}

/// 所有**音节边界**位置（0 与串尾必在；从每个可达边界往后走一个完整音节）。
fn boundaries(s: &str) -> Vec<usize> {
    let b = s.as_bytes();
    let mut reach = vec![false; b.len() + 1];
    reach[0] = true;
    for i in 0..b.len() {
        if !reach[i] {
            continue;
        }
        for len in 1..=6.min(b.len() - i) {
            if is_syllable(&s[i..i + len]) {
                reach[i + len] = true;
            }
        }
    }
    (0..=b.len()).filter(|&i| reach[i]).collect()
}

/// 枚举「`chunk_count` 段、整串全覆盖」的切法（只收至少含一个多音节段的切法）。
#[allow(clippy::too_many_arguments)]
fn walk<D: Dictionary + ?Sized>(
    s: &str,
    bounds: &[usize],
    dict: &D,
    learner: &Learner,
    boost: u64,
    cache: &mut Cache,
    pos: usize,
    chunks_left: usize,
    path: &mut Vec<Rc<Chunk>>,
    out: &mut Vec<Vec<Rc<Chunk>>>,
) {
    let last = bounds.len() - 1;
    if chunks_left == 1 {
        let Some(c) = chunk(s, bounds[pos], bounds[last], dict, learner, boost, cache) else {
            return;
        };
        if c.syllables >= 2 || path.iter().any(|p| p.syllables >= 2) {
            path.push(c);
            out.push(path.clone());
            path.pop();
        }
        return;
    }
    // 剩下的每一段至少要跨一个边界。
    let max_end = last - (chunks_left - 1);
    for end in pos + 1..=max_end {
        let Some(c) = chunk(s, bounds[pos], bounds[end], dict, learner, boost, cache) else {
            continue;
        };
        path.push(c);
        walk(
            s,
            bounds,
            dict,
            learner,
            boost,
            cache,
            end,
            chunks_left - 1,
            path,
            out,
        );
        path.pop();
    }
}

/// 取一段（`s[start..end]`）的精确词库条目；不合格/无命中 ⇒ `None`（带缓存）。
#[allow(clippy::too_many_arguments)]
fn chunk<D: Dictionary + ?Sized>(
    s: &str,
    start: usize,
    end: usize,
    dict: &D,
    learner: &Learner,
    boost: u64,
    cache: &mut Cache,
) -> Option<Rc<Chunk>> {
    if let Some(cached) = cache.get(&(start, end)) {
        return cached.clone();
    }
    let sub = &s[start..end];
    let built = if sub.len() >= 2 {
        min_syllables(sub)
            .filter(|n| *n <= MAX_CHUNK_SYLLABLES)
            .and_then(|syllables| {
                let mut entries: Vec<EntryHit> = dict
                    .query_prefix_top(sub, 0)
                    .into_iter()
                    // n = 0 已只返回精确条目；再按长度钉一次（替身词库也守同一条）。
                    .filter(|e| e.pinyin_len == sub.len())
                    .map(|e| EntryHit {
                        score: rank_score(e.freq, learner.freq_of(&e.word), boost),
                        text: e.word,
                    })
                    .collect();
                if entries.is_empty() {
                    return None;
                }
                entries.sort_by(|a, b| b.score.cmp(&a.score).then(a.text.cmp(&b.text)));
                entries.truncate(MAX_RESULTS);
                Some(Rc::new(Chunk {
                    entries,
                    syllables,
                    bytes: sub.len(),
                }))
            })
    } else {
        None
    };
    cache.insert((start, end), built.clone());
    built
}

/// 一段切法内各段条目的笛卡尔积，每层只留分数前 `MAX_RESULTS`（见头注）。
fn combinations(chunks: &[Rc<Chunk>]) -> Vec<(u64, String)> {
    let mut partial: Vec<(u64, String)> = vec![(0, String::new())];
    for c in chunks {
        let mut next = Vec::with_capacity(partial.len() * c.entries.len());
        for (score, text) in &partial {
            for e in &c.entries {
                let mut joined = String::with_capacity(text.len() + e.text.len());
                joined.push_str(text);
                joined.push_str(&e.text);
                next.push((score.saturating_add(e.score), joined));
            }
        }
        next.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        next.truncate(MAX_RESULTS);
        partial = next;
    }
    partial
}
