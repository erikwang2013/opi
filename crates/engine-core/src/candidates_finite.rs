// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **单键有限窗口的精确前缀**（2026-10-25 首击尖峰）。
//!
//! 移动端 `candidates(8/64)` 显示候选，选中时 `Engine::select` 用 `candidates(MAX)` 的
//! 全局下标提交。因此有限窗口必须是全量列表的**逐条前缀**，否则会「看到百度、上屏不饿」。
//! 单纯把前缀池从 9216 缩到 2048 或按分数预筛都会破坏这条（词按 `pinyin_len` 先排、
//! 单字按长度分组且每组封顶 6 —— 都不是分数序），所以这里不缩池，而是**只物化首位
//! 那一小段**：
//!
//! 1. 取与 MAX 完全相同的池：`query_prefix_top(input, PREFIX_MATCH_CAP)`；
//! 2. 在池上按全管线同一条规则算「每个文本的最佳行」（最高分；同分保留池序）；
//! 3. 按 `tiers::apply` 的档位顺序，只挑出填满 `limit` 所需的分组（整组保留）；
//! 4. 档位内用与全管线相同的键排序：(得分是否 ≤ floor, 是否含繁体, 分数降序, 文本升序)。
//!
//! 只在**单字符输入**启用：这是首击尖峰所在；多字符输入直接走全管线（池也固定 9216，
//! 因此同样满足前缀契约）。单字符下 `abbr`（要求主块为空）、`composition`（至少 3 个
//! 音节）都不参与主块，`polyphone` 表也没有单字母键 —— 这三点由本函数的入口校验兜底。
//! 符号/模糊/纠错/简拼都追加在主块之后；主块已够 `limit` 时它们不可见，故可安全省掉。

use std::collections::HashMap;

use crate::candidates::{Candidate, CandidateKind, rank_score};
use crate::dictionary::{Dictionary, PREFIX_MATCH_CAP};
use crate::learner::Learner;

/// 池中的一行（借用词条文本，避免为 9216 条各分配一次 Candidate）。
struct Row<'a> {
    text: &'a str,
    score: u64,
    pinyin_len: usize,
    exact: bool,
    learned: bool,
}

/// 单字符、有限 limit 的精确前缀；条件不成立或主块填不满一屏时返回 `None`
/// （调用方回落到全管线，结果相同，只是慢）。
pub(crate) fn single_key_prefix<D: Dictionary + ?Sized>(
    dict: &D,
    learner: &Learner,
    input: &str,
    limit: usize,
    boost: u64,
    prefer_simplified: bool,
) -> Option<Vec<Candidate>> {
    if limit == 0
        || limit == usize::MAX
        || input.chars().count() != 1
        // 单字母没有补读音条目（表按合法音节建；单字母只有 a/e/o，表里也没有）。
        || !crate::polyphone::hits(input).is_empty()
        // 兜底：将来组合识别若对单键产出，交给全管线。
        || !crate::composition::candidates(dict, learner, boost, input).is_empty()
    {
        return None;
    }
    let pool = dict.query_prefix_top(input, PREFIX_MATCH_CAP);
    if pool.is_empty() {
        return None;
    }
    let exact_bonus = boost / 2;
    let rows: Vec<Row<'_>> = pool
        .iter()
        .map(|e| {
            let user = learner.freq_of(&e.word);
            let exact = e.pinyin_len == input.len();
            Row {
                text: &e.word,
                score: rank_score(e.freq, user, boost).saturating_add(if exact {
                    exact_bonus
                } else {
                    0
                }),
                pinyin_len: e.pinyin_len,
                exact,
                learned: user > 0,
            }
        })
        .collect();

    // 全管线先按 (分数降序, 文本升序) 排序再去重：每个文本保留最高分那一行；
    // 同分保留池序（这里的 `>=` 即「先出现者赢」）。
    let mut best: HashMap<&str, usize> = HashMap::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        match best.get(row.text) {
            Some(&j) if rows[j].score >= row.score => {}
            _ => {
                best.insert(row.text, i);
            }
        }
    }
    let trad: Vec<bool> = if prefer_simplified {
        rows.iter()
            .map(|r| crate::trad_chars::contains_trad_only(r.text))
            .collect()
    } else {
        vec![false; rows.len()]
    };
    let ci = |a: usize, b: usize| {
        let (ra, rb) = (&rows[a], &rows[b]);
        let key = |r: &Row<'_>, i: usize| {
            if prefer_simplified {
                (r.score <= exact_bonus, trad[i])
            } else {
                (false, false)
            }
        };
        key(ra, a)
            .cmp(&key(rb, b))
            .then_with(|| rb.score.cmp(&ra.score))
            .then_with(|| ra.text.cmp(rb.text))
    };

    let (mut learned, mut chars, mut words, mut singles) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for i in best.into_values() {
        let r = &rows[i];
        if r.learned {
            learned.push(i);
        } else if r.text.chars().count() == 1 {
            if r.exact {
                chars.push(i);
            } else {
                singles.push(i);
            }
        } else {
            words.push(i);
        }
    }
    let mut out: Vec<usize> = Vec::with_capacity(limit);
    learned.sort_by(|&a, &b| ci(a, b));
    for i in learned {
        out.push(i);
        if out.len() == limit {
            return Some(materialize(&rows, &out));
        }
    }
    chars.sort_by(|&a, &b| ci(a, b));
    for i in chars {
        out.push(i);
        if out.len() == limit {
            return Some(materialize(&rows, &out));
        }
    }
    // 词组档：先按拼音长度分组，组内按同一分档键；整组取到填满为止。
    words.sort_by(|&a, &b| {
        rows[a]
            .pinyin_len
            .cmp(&rows[b].pinyin_len)
            .then_with(|| ci(a, b))
    });
    for &i in &words {
        out.push(i);
        if out.len() == limit {
            return Some(materialize(&rows, &out));
        }
    }
    // 联想档：进入顺序 = 全局分档序；每长度前 6 条进「联想」，其余沉底。
    singles.sort_by(|&a, &b| ci(a, b));
    let mut per_len: HashMap<usize, usize> = HashMap::new();
    let (mut assoc, mut sunk) = (Vec::new(), Vec::new());
    for i in singles {
        let n = per_len.entry(rows[i].pinyin_len).or_insert(0);
        if *n < 6 {
            *n += 1;
            assoc.push(i);
        } else {
            sunk.push(i);
        }
    }
    // 联想档按拼音长度升序；同一长度内保持上面的全局分档序（稳定排序）。
    assoc.sort_by_key(|&i| rows[i].pinyin_len);
    for i in assoc.into_iter().chain(sunk) {
        out.push(i);
        if out.len() == limit {
            return Some(materialize(&rows, &out));
        }
    }
    // 主块不够一屏：符号/模糊等追加档可能进屏，交回全管线（结果相同）。
    None
}

fn materialize(rows: &[Row<'_>], idx: &[usize]) -> Vec<Candidate> {
    idx.iter()
        .map(|&i| Candidate {
            text: rows[i].text.to_string(),
            kind: CandidateKind::Hanzi,
            score: rows[i].score,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::InMemoryDictionary;

    fn dict(words: usize) -> InMemoryDictionary {
        let mut d = InMemoryDictionary::new();
        for i in 0..words {
            d.insert("ba", &format!("词{i:02}"), 10_000 - i as u32);
        }
        d
    }

    #[test]
    fn fills_exactly_on_the_last_word() {
        // limit 恰好等于词数：最后一组最后一条推入后就应返回 Some（曾在循环顶部
        // 检查长度，填满后没有下一轮 ⇒ 误判为「填不满」并退回全管线）。
        let fast = single_key_prefix(&dict(8), &Learner::new(true), "b", 8, 100_000, true);
        assert_eq!(fast.map(|c| c.len()), Some(8));
    }

    #[test]
    fn returns_none_when_the_main_block_cannot_fill_the_page() {
        // 只有 3 条、limit 8：符号/追加档可能补位，必须交回全管线。
        assert!(single_key_prefix(&dict(3), &Learner::new(true), "b", 8, 100_000, true).is_none());
    }
}
