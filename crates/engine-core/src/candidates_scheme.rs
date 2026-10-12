// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **按键类展开一族**（9/14 键方案）—— 2026-10-12 从 `candidates.rs` 拆出：那边加
//! **联想档**后越 500 行门禁；拆分先例与 `engine_punct.rs` / `engine_readings.rs` 同套
//! （impl/函数按职责搬，语义一字未动）。
//!
//! 为什么它值得单独一个文件：这条路**表达不成前缀查询**（「64」不是任何键的前缀），
//! 与全键盘路的数据来源、算法形状都不同（见本文件内 [`rank_and_pick_scheme_with_ctx`]
//! 的长头注）。对外路径不变：`candidates.rs` 里 `pub use` 回来，调用方仍写
//! `candidates::rank_and_pick_scheme`。

use crate::candidates::{ASSOC_TOP_N, Candidate, CandidateKind, rank_score};
use crate::dictionary::{Dictionary, SCHEME_MATCH_CAP};
use crate::learner::Learner;

/// **按键类展开**（9/14 键方案，2026-10-09）：`input` 是一串按键符号，按
/// [`crate::scheme::Scheme::matches`] 的类前缀语义匹配词库键。
///
/// 为什么另起一条路而不是塞进 [`rank_and_pick_with`]：这条匹配**表达不成前缀
/// 查询**（「64」不是任何键的前缀），只能全量遍历词库逐键比对
/// （`Dictionary::visit_entries`）—— 数据来源、算法形状都不同，硬合并会把两条
/// 路都读不懂。打分/去重/截断规则与全键盘**逐字相同**（exact 加成按「键长相等」
/// 判，与 pinyin_len == 输入长度同义），保证两套键盘的候选行为一致；整串无命中时
/// 还有 26 键同一条**回退**（2026-10-10 扩到 9 键：`644269` → 你好一族）。
/// 返回 `(候选, 逐条读音)`：`readings[i]` = `cands[i]` 的词典拼音（截断后同序，
/// 两串等长）。9 键的缓冲是数字串，界面上要显示「拼音读数」时靠它 ——
/// `display_buffer` 取第一条（搜狗 9 键候选栏左侧那格同款），
/// `candidate_readings` 读同一份缓存并按读音**去重保序**（2026-10-12 用户裁决
/// 「左列去重」；逐条对应只到本函数返回值与引擎内部缓存为止，不再是出口契约）。
/// 没有候选时两条都为空。14 键同路返回（它的缓冲本来就是字母串，读数字段无副作用）。
pub fn rank_and_pick_scheme<D: Dictionary + ?Sized>(
    dict: &D,
    learner: &Learner,
    input: &str,
    limit: usize,
    boost: u64,
    scheme: crate::scheme::Scheme,
    pin: Option<&str>,
) -> (Vec<Candidate>, Vec<String>) {
    rank_and_pick_scheme_with_ctx(dict, learner, input, limit, boost, scheme, pin, "")
}

/// [`rank_and_pick_scheme`] + **联想上下文**（`ctx` = 上文词的**拼音**；2026-10-12 用户
/// 裁决「选词后接着打要能接着匹配」）。
///
/// 9/14 键的缓冲是**键位**（数字/字母组），所以联想的整串 = `digits(上文拼音) + 缓冲`
/// —— 打 `64426` 选「你好」、再打 `62`，拼接串命中键 `nihaoma` ⇒ 「你好吗」排最前。
/// 除按键类匹配外还要求**拼音真以上文开头**（`starts_with`，类匹配本身允许同键的
/// 别的字母，那是 9 键固有的歧义，不该被联想放大）。上限 [`ASSOC_TOP_N`] 条。
///
/// `ctx` 空 / 转不出键位（[`Scheme::digits_of`] 恒 `None` 的两方案）⇒ 逐字等于旧行为。
///
/// [`Scheme::digits_of`]: crate::scheme::Scheme::digits_of
#[allow(clippy::too_many_arguments)]
pub fn rank_and_pick_scheme_with_ctx<D: Dictionary + ?Sized>(
    dict: &D,
    learner: &Learner,
    input: &str,
    limit: usize,
    boost: u64,
    scheme: crate::scheme::Scheme,
    pin: Option<&str>,
    ctx: &str,
) -> (Vec<Candidate>, Vec<String>) {
    if input.is_empty() || limit == 0 {
        return (Vec::new(), Vec::new());
    }
    let exact_bonus = boost / 2;
    // 一趟扫描 = 一个输入串的整档（读音随条目一起暂存：pinyin 的生命周期出不了
    // visit_entries 的回调）。两趟（回退态）共用这一份。
    let scan = |input: &str| -> Vec<(Candidate, String)> {
        let input_len = input.len();
        // 按键类匹配走 `Dictionary::scheme_entries`（区间枚举 + 静态词频 top-N 截断）——
        // 2026-10-11 移动端实测：此前是全表逐条 `matches`，936k 条下单键 268ms（桌面
        // release），手机端每键秒级。pin 过滤（左列点一行拼音）语义在实现里同义保留。
        dict.scheme_entries(input, scheme, pin, SCHEME_MATCH_CAP)
            .into_iter()
            .map(|(e, pinyin)| {
                let exact = pinyin.len() == input_len;
                let score = rank_score(e.freq, learner.freq_of(&e.word), boost)
                    .saturating_add(if exact { exact_bonus } else { 0 });
                (
                    Candidate {
                        text: e.word,
                        kind: CandidateKind::Hanzi,
                        score,
                    },
                    pinyin,
                )
            })
            .collect()
    };
    // ── 数字空间回退（2026-10-10 扩到 9 键）：整串无命中改扫最长音节对齐键前缀的整档
    // （判定唯一实现 `scheme::scheme_fallback_len`；命中态非空 ⇒ 一行不重，重扫深度 ≤ 1）。
    let mut merged = scan(input);
    if merged.is_empty()
        && let Some(k) = crate::scheme::scheme_fallback_len(dict, input, scheme, pin)
    {
        merged = scan(&input[..k]);
    }
    merged.sort_by(|a, b| b.0.score.cmp(&a.0.score).then(a.0.text.cmp(&b.0.text)));
    let mut seen = std::collections::HashSet::new();
    merged.retain(|(c, _)| seen.insert(c.text.clone()));
    // 联想档（见 `rank_and_pick_scheme_with_ctx` 头注）：`digits(ctx) + input` 的整串
    // 命中，插最前；截断按新序（先保它们）。
    if !ctx.is_empty()
        && let Some(ctx_digits) = scheme.digits_of(ctx)
    {
        let joined = format!("{ctx_digits}{input}");
        let mut assoc: Vec<(Candidate, String)> = dict
            .scheme_entries(&joined, scheme, pin, SCHEME_MATCH_CAP)
            .into_iter()
            // 真接着上文：拼音以上文开头、且比上文长（等长 = 上文那个词自己）。
            .filter(|(_, py)| py.len() > ctx.len() && py.starts_with(ctx))
            .filter(|(e, _)| !seen.contains(&e.word))
            .map(|(e, py)| {
                let exact = py.len() == joined.len();
                let score = rank_score(e.freq, learner.freq_of(&e.word), boost)
                    .saturating_add(if exact { boost / 2 } else { 0 });
                (
                    Candidate {
                        text: e.word,
                        kind: CandidateKind::Hanzi,
                        score,
                    },
                    py,
                )
            })
            .collect();
        assoc.sort_by(|a, b| b.0.score.cmp(&a.0.score).then(a.0.text.cmp(&b.0.text)));
        let mut s2 = std::collections::HashSet::new();
        assoc.retain(|(c, _)| s2.insert(c.text.clone()));
        assoc.truncate(ASSOC_TOP_N);
        if !assoc.is_empty() {
            assoc.extend(merged);
            merged = assoc;
        }
    }
    merged.truncate(limit);
    // 读音**逐条**带出（与候选同序、同一次排名）—— 调用方按需取首条或全量；
    // 分开再算一次 = 第二次全量扫描，别在调用链上再开这条路。
    merged.into_iter().unzip()
}
