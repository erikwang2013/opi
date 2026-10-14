// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **分词回退一族**（2026-10-14 从 `candidates.rs` 拆出 —— 那边加候选三档后 558 行，
//! 撞 500 行门禁；拆分先例与 `candidates_scheme.rs` / `candidates_options.rs` /
//! `candidates_tiers.rs` 同套，**语义一字未动**）。
//!
//! 为什么它单独成文件：这段是「缓冲打不出整串时，该拿哪一段当候选」的**唯一判定**，
//! 候选层（`rank_and_pick_meta` 的回退档）与收尾层（`Engine::commit_buffer_after_select`）
//! 共用它 —— 它不依赖候选装配的其余部分，反过来也一样。
//!
//! 对外路径不变：`candidates.rs` 里 `pub(crate) use` 回来，调用方仍写
//! `candidates::segment_fallback`。

use crate::correction;
use crate::dictionary::Dictionary;
use crate::fuzzy;
use crate::pinyin::{is_syllable, segment};

/// 分词回退的落点：候选出 `prefix` 的整档，选中后缓冲里留 `suffix` 接着编。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SegmentFallback {
    /// 被消耗的前缀：**音节对齐**，且已去掉分词符（词库键不含 `'`，格式约定）。
    pub prefix: String,
    /// 留在缓冲里的**原文后缀**（其间的 `'` 原样保留）。
    pub suffix: String,
}

/// 分词回退的**唯一判定**（2026-10-10 用户裁决「输入拼音无匹配时要分词匹配」）：
/// 给定缓冲，返回「候选该出哪一段、选完缓冲剩什么」。
///
/// 两个调用点共用它 —— 候选层（[`rank_and_pick_with`] 的回退档）与收尾层
/// （`Engine::commit_buffer_after_select`）；**纯函数，故不存状态**：收尾层不需要
/// 「先取候选、再选中」的次序假设（缓冲自证，与 `scheme_readings` 的 `(缓冲, 值)`
/// 绑存是同一条纪律）。
///
/// 判据（全成立才回退；**先算不查库的两条**，收尾层每次选中都要过一次它）：
/// 1. 缓冲不止一个字符 —— 空/单字符维持既有「无匹配」；
/// 2. 切分出来**至少两段** —— 单音节没有「分词」可言（边界：单音节无匹配维持原样）；
/// 3. **整串直接命中为空** —— 有命中 = 更具体的档，不回退；
/// 4. **整串模糊命中为空**（`fuzzy_enabled` 关掉时这条不问）—— 整串变体是整串替换、
///    比前缀更具体：`zongguo` 有「中国」档时不许被 `zong` 的整档接管；
///    然后从**最长**的音节对齐前缀往短试，第一个「每段都是完整音节 + 词库有命中」的胜出。
///
/// 「完整音节」是**门槛**（用户裁决的边界）：`qqq` / `zxcvb` 这类打错串切出来的段不是
/// 完整音节 ⇒ 不回退 ⇒ 维持无候选（「打英文」的逃生口靠它，
/// 钉子见 tests/punctuation.rs 与 tests/adversarial_input.rs）。
///
/// ⚠️ 门槛只管「段是不是音节」：首段**恰好**是完整音节的英文串会回退（`hello` → 前缀
/// `he`，留 `llo`）。这是本规则的已知形态、不是漏网 —— 这类串在旧行为里同样有候选
/// （逐音节汤也命中 `he`），而逃生口本就只对**无候选**的串开放（无候选时空格提交原文）。
///
/// 前缀串取**各段拼接**（去掉 `'`）：词库键不含分隔符，`ni'hao` 的整串直接查询本是空的，
/// 拼起来查才是用户的本意（`ni'hao` → 你好一族）。
pub(crate) fn segment_fallback<D: Dictionary + ?Sized>(
    dict: &D,
    input: &str,
    fuzzy_enabled: crate::fuzzy::FuzzyMask,
    correction_enabled: bool,
) -> Option<SegmentFallback> {
    if input.chars().count() <= 1 {
        return None;
    }
    let segs = segment(input);
    if segs.len() < 2 {
        return None;
    }
    if !dict.query_all(input).is_empty() {
        return None;
    }
    if fuzzy_enabled != 0
        && fuzzy::input_variants(input, fuzzy_enabled)
            .iter()
            .any(|v| !dict.query_all(v).is_empty())
    {
        return None;
    }
    // 同类第四条：整串**纠错**命中同样比前缀回退更具体（2026-10-13）。
    if correction_enabled
        && correction::input_corrections(input)
            .iter()
            .any(|v| !dict.query_all(v).is_empty())
    {
        return None;
    }
    // 各段在拼接串里的累计字符数：`prefix_len[k-1]` = 前 k 段拼起来的长度，
    // 用来在**原文**里定位后缀从哪开始。
    let mut prefix_len = Vec::with_capacity(segs.len());
    let mut total = 0usize;
    for s in &segs {
        total += s.chars().count();
        prefix_len.push(total);
    }
    for k in (1..=segs.len()).rev() {
        if !segs[..k].iter().all(|s| is_syllable(s)) {
            continue;
        }
        let prefix: String = segs[..k].concat();
        if !dict.query_all(&prefix).is_empty() {
            return Some(SegmentFallback {
                suffix: raw_suffix(input, prefix_len[k - 1]),
                prefix,
            });
        }
    }
    None
}

/// 缓冲原文里「前 `consumed` 个非分隔符字符」之后的剩余（其间的 `'` 原样保留）：
/// `("nihaob", 5)` → `"b"`、`("ni'hao", 5)` → `""`、`("ni'hao'x", 5)` → `"'x"`。
/// 前缀串是去 `'` 的拼接，故按「数够 `consumed` 个非 `'` 字符」定位 ——
/// 分隔符归**前段**（它标记的正是这个边界）。
fn raw_suffix(input: &str, consumed: usize) -> String {
    let mut seen = 0usize;
    for (i, ch) in input.chars().enumerate() {
        if ch != '\'' {
            seen += 1;
        }
        if seen == consumed {
            return input.chars().skip(i + 1).collect();
        }
    }
    String::new()
}
