// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **按键类展开一族**（9/14 键方案）—— 2026-10-12 从 `candidates.rs` 拆出（当初为
//! 500 行门禁；拆分先例与 `engine_punct.rs` / `engine_readings.rs` 同套，语义一字未动）。
//! 同日该拆分随「上文拼接」一并被裁决撤除**行为**，但**文件拆分保留**（删了它
//! `candidates.rs` 会重新越线）。
//!
//! 为什么它值得单独一个文件：这条路**表达不成前缀查询**（「64」不是任何键的前缀），
//! 与全键盘路的数据来源、算法形状都不同（见 [`rank_and_pick_scheme`] 的长头注）。
//! 对外路径不变：`candidates.rs` 里 `pub use` 回来，调用方仍写
//! `candidates::rank_and_pick_scheme`。

use crate::candidates::{Candidate, CandidateKind, rank_score};
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
// 8 个参数（7 个 + `prefer_simplified`）踩了 clippy 的默认上限。**有意放行**：
// 这个开关必须在**本函数内部的截断之前**生效（截断后再分区 = 被截掉的简体候选回不来），
// 所以只能从参数进来；而把它收进 struct 会牵动唯一调用方之外的测试与全键盘那条路
// （`RankOptions` 已有三个开关，两条路的开关面并不相同 —— 合并要另立一次裁决）。
#[allow(clippy::too_many_arguments)]
pub fn rank_and_pick_scheme<D: Dictionary + ?Sized>(
    dict: &D,
    learner: &Learner,
    input: &str,
    limit: usize,
    boost: u64,
    scheme: crate::scheme::Scheme,
    pin: Option<&str>,
    prefer_simplified: bool,
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
    // 简体优先（见参数注）：**截断之前**分区。
    if prefer_simplified {
        // 成对分区（读音跟候选一起搬 —— 两者必须保持同序），档位线同全键盘路。
        crate::trad_chars::demote_traditional_by(
            &mut merged,
            boost / 2,
            |(c, _)| c.score,
            |(c, _)| &c.text,
        );
    }
    merged.truncate(limit);
    // 读音**逐条**带出（与候选同序、同一次排名）—— 调用方按需取首条或全量；
    // 分开再算一次 = 第二次全量扫描，别在调用链上再开这条路。
    merged.into_iter().unzip()
}
