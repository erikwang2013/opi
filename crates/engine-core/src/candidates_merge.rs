// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 主块的排序/去重与追加档合并（2026-11-02 性能审计；`candidates.rs` 已在 500 行
//! 门禁上，故与 `candidates_tiers.rs` 同一套拆法）。
//!
//! 审计实测的两笔白付：主查询 `merged` 可达 9216 条（`PREFIX_MATCH_CAP`），旧实现
//! ①排序时反复搬 56 字节的 `(Candidate, Option<Meta>)`；②去重为每条候选克隆一次
//! `String` 进 `HashSet`。这里排序只动「分数字 + 文本引用 + 源下标」键数组，排完按
//! 置换把元组各搬一次；去重集合借用文本，不克隆。

use std::collections::HashSet;

use crate::candidates::Candidate;

/// 按 (分数降序, 文本升序) 排序后按文本去重（首次出现者留）。
///
/// 次序与旧的 `merged.sort_unstable_by(...)` + `HashSet<String>` 去重逐条相同：键数组
/// 与元组数组等长且每个位置的键值一致，`sort_unstable_by` 的比较序列只由位置上的键
/// 决定 ⇒ 两个数组得到同一置换；去重仍按排序后的首个出现者保留。
pub(crate) fn sort_and_dedupe<T>(
    items: &mut Vec<T>,
    score: impl Fn(&T) -> u64,
    text: impl for<'a> Fn(&'a T) -> &'a str,
) {
    let mut keys: Vec<(u64, &str, usize)> = items
        .iter()
        .enumerate()
        .map(|(i, it)| (score(it), text(it), i))
        .collect();
    keys.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
    // dest[源位置] = 目标位置（键数组排序结果的逆置换）。
    let mut dest = vec![0usize; items.len()];
    for (pos, key) in keys.iter().enumerate() {
        dest[key.2] = pos;
    }
    drop(keys);
    // 沿环置换：每个元素最多被换一次就落到最终位置，元组不做整趟排序搬运。
    for i in 0..items.len() {
        while dest[i] != i {
            let j = dest[i];
            items.swap(i, j);
            dest.swap(i, j);
        }
    }
    // 借用文本去重（不克隆）：keep 掩码先算完，再一次性搬走幸存者。
    let keep: Vec<bool> = {
        let mut seen = HashSet::with_capacity(items.len());
        items.iter().map(|it| seen.insert(text(it))).collect()
    };
    let mut next = 0;
    items.retain(|_| {
        let keep = keep[next];
        next += 1;
        keep
    });
}

/// 模糊/简拼/纠错按序追加：与主块（借用集合）+ 已保留的追加档（小的持有集合）
/// 去重，首次出现者留；主块为空时该档直接成为主结果。
///
/// 与旧实现的次序/语义逐条相同：三档按 `fuzzy → jianpin → corr` 的次序判重与合并，
/// 主块非空时只追加（截断先砍追加档），主块为空时第一档接管。
pub(crate) fn append_tiers(main: &mut Vec<Candidate>, mut tiers: [&mut Vec<Candidate>; 3]) {
    {
        // 主块借用，不克隆 9k 个 String；追加档的存活项少，才用持有集合。
        let main_seen: HashSet<&str> = main.iter().map(|c| c.text.as_str()).collect();
        let mut added: HashSet<String> = HashSet::new();
        for tier in &mut tiers {
            tier.retain(|c| !main_seen.contains(c.text.as_str()) && added.insert(c.text.clone()));
        }
    }
    for tier in tiers {
        if main.is_empty() {
            *main = std::mem::take(tier);
        } else {
            main.append(tier);
        }
    }
}
