// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

use crate::composer::Mode;
use crate::dictionary::Dictionary;
use crate::fuzzy;
use crate::learner::Learner;
use crate::pinyin::segment;
use crate::symbols::SymbolEngine;
use crate::trie::Entry;

/// 用户词频权重下限：一次选词 ≈ 10 万次静态词频，保证学习迅速生效。
/// 实际权重在 Engine::new 按词典最大词频动态缩放（max_freq × 2），
/// 固定值在 luna 百万级词频下失配：选一次"我"(10万) 仍输给"倭"(50万)。
pub const USER_BOOST: u64 = 100_000;
/// 默认候选栏容量。
pub const DEFAULT_TOP_N: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateKind {
    Hanzi,
    English,
    Emoji,
    Symbol,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub text: String,
    pub kind: CandidateKind,
    pub score: u64,
}

/// 排序分：静态词频 + 用户词频 × boost。
pub fn rank_score(static_freq: u32, user_freq: u32, boost: u64) -> u64 {
    (static_freq as u64).saturating_add((user_freq as u64).saturating_mul(boost))
}

/// 合并词典候选与符号候选，排序、去重、截断。
pub fn rank_and_pick<D: Dictionary + ?Sized>(
    dict: &D,
    symbols: &SymbolEngine,
    learner: &Learner,
    input: &str,
    mode: Mode,
    limit: usize,
    boost: u64,
) -> Vec<Candidate> {
    // 空输入守卫必须**先于**模式分派：`ranking_invariants.rs` 与 `adversarial_input.rs`
    // 都对「含 Symbol 的 5 模式 + 空输入」断言无候选（`search("")` 会返回全部 583 条）。
    if input.is_empty() {
        return Vec::new();
    }
    // 符号模式（用户裁决）：候选**只出符号**，不与词库合并。合并版本里符号永远赢不了 ——
    // dun 组汉字带精确加成（score ≈ 3.9e9 + exact_bonus），符号未学习态 score 0，
    // 被 DEFAULT_TOP_N 与 FETCH_LIMIT 双重截掉，模式开关等于没生效。
    // 顺序即 `symbols.search` 的关键字序（不重排：这个模式的语义是「只剩符号」）。
    if mode == Mode::Symbol {
        return symbols
            .search(input)
            .into_iter()
            .take(limit)
            .map(|s| Candidate {
                text: s.text.clone(),
                kind: if s.emoji {
                    CandidateKind::Emoji
                } else {
                    CandidateKind::Symbol
                },
                score: rank_score(0, learner.freq_of(&s.text), boost),
            })
            .collect();
    }
    if !matches!(mode, Mode::Pinyin | Mode::Traditional) {
        return Vec::new();
    }
    let input_len = input.len();
    // 精确等长匹配加成 = boost/2。boost = max(USER_BOOST, 词典最大静态词频×2)
    // （见 Engine::with_dictionaries），故 boost/2 ≥ 最大静态词频 —— 任何精确匹配（哪怕
    // freq=0）都排在任何前缀扩展之前：输入 xian 不再由 xiang 的「想」占首位。
    // 而一次选词的最小收益是 1×boost > boost/2，学过的前缀扩展词仍能反超（缘起 #3 不受损）。
    let exact_bonus = boost / 2;
    let hanzi = |e: Entry, exact: bool| Candidate {
        text: e.word.clone(),
        kind: CandidateKind::Hanzi,
        score: rank_score(e.freq, learner.freq_of(&e.word), boost).saturating_add(if exact {
            exact_bonus
        } else {
            0
        }),
    };
    let mut merged: Vec<Candidate> = dict
        .query_all(input)
        .into_iter()
        .map(|e| {
            let exact = e.pinyin_len == input_len;
            hanzi(e, exact)
        })
        .collect();
    // 多音节整串无命中时按音节逐段补候选（segment 此前是死代码）：
    // nihao → [你][好] 逐字可选；单字母音节跳过避免噪音。
    // 每音节仅取 top 3：luna 繁体词库下全量并入会把生僻字顶进 top-8。
    //
    // 这个 top 3 必须按**含 learner boost 的最终分**排，不能下推给
    // `dict.query(&syl, 3)` —— 后者截的是静态词频前 3，学过的常用字会在加 boost
    // 之前就被丢掉（"haoxiao" 里「好」在 hao 组排第 6，输入再多次也进不来）。
    // 与下方 85-86 行主路径是同一条不变式，此处曾违反。
    // ── 模糊音：整串变体（常开；表与变体生成见 fuzzy.rs）──────────────────────
    // 先算，因为它决定逐音节回退里还要不要再做模糊（见下）：**更具体的匹配优先** ——
    // 与上面「整串命中就不逐音节」是同一条分层规则。
    //
    // **变体命中一律不算精确匹配**（`exact = false`）：`e.pinyin_len == input_len` 这条
    // 判定对变体查询同样成立 —— `n↔l` / `f↔h` 是保长替换（ni 打回 li，键长都是 2），
    // 变体的**前缀扩展**更会因键长巧合命中（输入 `shu` 的变体 `su` 打回键 `suo`）。
    // 误判的代价是吃满 exact_bonus，与真精确匹配同档硬碰词频（真实词频：你 ni
    // 3999128899 > 裏 li 3998755570，只差 0.009%），模糊档内部也不再按真实词频排。
    // 加成必须为 **0**（非精确同档）：任何候选 ≤ max_freq ≤ boost/2 = exact_bonus 是
    // 「精确 ≥ 前缀扩展」那条不变量的结构性保证，模糊档因此按真实词频排。
    // ⚠️ 此处曾写「给半额会让 `ranking_invariants.rs` 的 proptest 随机红」——**实测不复现**
    // （半额版连跑 10 次 10/10 绿：重叠文本被上面的 retain 先去重，逐音节路径又没有精确
    // 候选可违）。结论（0 加成）不变，但**那条机制不许再当理由引用**。
    let mut fuzzy: Vec<Candidate> = fuzzy::input_variants(input)
        .iter()
        .flat_map(|v| dict.query_all(v))
        .map(|e| hanzi(e, false))
        .collect();
    let wide_hits = !fuzzy.is_empty();
    if merged.is_empty() && input.chars().count() > 1 {
        for syl in segment(input) {
            if syl.chars().count() < 2 {
                continue;
            }
            let mut syl_cands: Vec<Candidate> = dict
                .query_all(&syl)
                .into_iter()
                .map(|e| {
                    // 精确性按**音节**判（该路径下"输入"就是音节），与主路径同一条规则。
                    let exact = e.pinyin_len == syl.len();
                    hanzi(e, exact)
                })
                .collect();
            syl_cands.sort_by(|a, b| b.score.cmp(&a.score).then(a.text.cmp(&b.text)));
            merged.extend(syl_cands.into_iter().take(3));
            // 逐音节模糊：**只在整串变体一无所获时**才用。两库都有连写词组（新 luna 简体
            // 词组、trad），整串变体会带回「中国/中國」，那条更具体；再叠逐音节的「中/种/種」
            // 会把「中国」挤出 8 槽（实测 trad 7→10）。
            //
            // 整段回退（含上面那半段**不加模糊**的逐音节精确查询）非有不可 —— 这是**实测的
            // 前置条件**，不是推断：新 luna 打 `zongguo` 时 `query_all("zongguo")` 是 **0 行**
            // ⇒ `merged` 为空 ⇒ 回退成立；该输入 8 槽里的 国/國/过/总/總/宗 **六条只能**由
            // 逐音节子查询给出（整串变体 `zhongguo` 的 81 行里没有它们）。`sengseng` 同理：
            // 僧/䒏/鬙 来自逐音节，深深/神圣/生生… 来自整串变体，同一栏里两条机制并存。
            // ⚠️ 曾在此写过「luna 词组键空格分隔、21759 条全带空格」——旧 luna 的事实，
            // 2026-09-27 换简体词组源后已失效。
            //
            // ⚠️ **本块的可见性已被简繁两库的连写词组压得很低**（2026-09-27 实测，别据此删）：
            // - trad 仍承重：`nihao` 的 8 槽第 7/8 位「裏/裡」只能来自 `ni→li` 这条变体查询
            //   （`mingtian`→民、`zuotian`→桌/着、`yihou`→否/紑 同理）。
            // - 新 luna 上，**47 个常用词的实测里它一个候选都没贡献** —— 常用词现在多有条目键
            //   （`mingtian` 直接命中），整串变体也就非空 ⇒ `wide_hits` 拦下本块。它并未失效：
            //   64×64 音节对的穷举里仍有 1285 个输入以它为**唯一**模糊来源（如 `ninv`→
            //   `nin→lin/ling`、`nifei`→`fei→hei`），只是那些多不是词。
            // 每音节 ≤3 次额外查询且同样被 take(3) 截住，无笛卡尔积。
            if !wide_hits {
                let mut syl_fuzzy: Vec<Candidate> = fuzzy::variants(&syl)
                    .iter()
                    .filter(|v| *v != &syl)
                    .flat_map(|v| dict.query_all(v))
                    .map(|e| hanzi(e, false))
                    .collect();
                syl_fuzzy.sort_by(|a, b| b.score.cmp(&a.score).then(a.text.cmp(&b.text)));
                merged.extend(syl_fuzzy.into_iter().take(3));
            }
        }
    }
    fuzzy.sort_by(|a, b| b.score.cmp(&a.score).then(a.text.cmp(&b.text)));

    // 单字符时仅并入 emoji：符号英文关键字前缀（如 comma→顿号）泄漏进拼音候选是噪音
    // ——真机 "c" 键唯一候选曾是顿号。emoji 保留作趣味反馈。过滤由 search 内部完成：
    // 单键 183 条命中只克隆 16 条 emoji，其余不再被造出来再丢掉。
    let sym_hits = if input.chars().count() == 1 {
        symbols.search_emoji(input)
    } else {
        symbols.search(input)
    };
    for s in sym_hits {
        // 符号不带拼音，故不参与「精确/扩展」分层（其分数本就只有学习权重）。
        merged.push(Candidate {
            text: s.text.clone(),
            kind: if s.emoji {
                CandidateKind::Emoji
            } else {
                CandidateKind::Symbol
            },
            score: rank_score(0, learner.freq_of(&s.text), boost),
        });
    }
    // 不能下推 limit 到词库：学过的低静态词可反超截断线外的词，
    // 全量收集 + 排序是唯一正确方案（select 用有限 limit 只限 FFI 载荷）。
    // 故走 query_all：词库不必先按词频排一遍（下面这一排就是权威序）。
    merged.sort_by(|a, b| b.score.cmp(&a.score).then(a.text.cmp(&b.text)));
    let mut seen = std::collections::HashSet::new();
    merged.retain(|c| seen.insert(c.text.clone()));
    // 分档（用户裁决）：精确/前缀候选**非空** ⇒ 模糊结果**追加在最后**，既有候选的集合
    // 与顺序一字不变（= 开模糊后的列表以未开模糊的列表为前缀）；**为空** ⇒ 模糊结果就是
    // 主结果（打 zongguo 想打「中国」不该退到列表末尾）。两分支同一份代码：先追加再截断，
    // 故截断永远先砍模糊结果，精确档不受影响。
    fuzzy.retain(|c| seen.insert(c.text.clone()));
    if merged.is_empty() {
        merged = fuzzy;
    } else {
        merged.extend(fuzzy);
    }
    merged.truncate(limit);
    merged
}

// 单测独立成文件（`#[path]` 引入）以保持本文件 <500 行，与 router_tests.rs 同惯例。
#[cfg(test)]
#[path = "candidates_tests.rs"]
mod tests;
