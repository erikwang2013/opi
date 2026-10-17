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

/// 词库 `query_all` 是**前缀**语义（`InMemoryDictionary`/`MmapDictionary` 的 `query`
/// 都按前缀返回）；判「整串有没有命中」必须再按 `pinyin_len == key.len()` 过一遍 ——
/// 否则更长的词以输入为前缀时会被当成「整串命中」，把逐字确认整条挡掉
/// （2026-10-18 用户报障的根因：`zhongguo` 命中 `zhongguoren`）。
fn has_exact<D: Dictionary + ?Sized>(dict: &D, key: &str) -> bool {
    dict.query_all(key)
        .iter()
        .any(|e| e.pinyin_len == key.len())
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
/// 3. **整串精确命中为空** —— 有精确命中 = 更具体的档，不回退；**前缀扩展不算**
///    （`has_exact` 只认 `pinyin_len == key.len()`）；
/// 4. **整串模糊/纠错精确命中为空**（开关关掉时这条不问）—— 整串变体是整串替换、
///    比前缀更具体：`zongguo` 有「中国」档时不许被 `zong` 的整档接管；
///    然后取**第一个音节**（用户裁决「先匹配前面的第一个字」）—— 它必须是完整音节
///    且词库有命中；整档候选 = 这个音节的所有读音的字（精确单字在前）。
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
    if has_exact(dict, input) {
        return None;
    }
    if fuzzy_enabled != 0
        && fuzzy::input_variants(input, fuzzy_enabled)
            .iter()
            .any(|v| has_exact(dict, v))
    {
        return None;
    }
    // 同类第四条：整串**纠错**精确命中同样比前缀回退更具体（2026-10-13）。
    if correction_enabled
        && correction::input_corrections(input)
            .iter()
            .any(|v| has_exact(dict, v))
    {
        return None;
    }
    // 用户裁决（2026-10-17 / 2026-10-18 再次确认）：**先匹配前面的第一个字**。
    // 只取第一个音节，不再从最长前缀往回试 —— 后者在「更长的词以整串为前缀」时
    // 会把整串又当成一次匹配（候选仍是那个长词，逐字确认永远进不去，用户
    // 2026-10-18 报障的现场）。第一个音节不是完整音节 / 词库无命中 ⇒ 不回退，
    // 交给候选层的逐音节汤或维持无匹配。
    let first = &segs[0];
    if !is_syllable(first) || dict.query_all(first).is_empty() {
        return None;
    }
    Some(SegmentFallback {
        prefix: first.clone(),
        suffix: raw_suffix(input, first.chars().count()),
    })
}

/// 缓冲原文里「前 `consumed` 个非分隔符字符」之后的剩余：
/// `("nihaob", 5)` → `"b"`、`("ni'hao", 5)` → `""`、`("ni'hao'x", 5)` → `"'x"`。
/// 前缀串是去 `'` 的拼接，故按「数够 `consumed` 个非 `'` 字符」定位 ——
/// 界上的分隔符归**前段**（它标记的正是这个边界），返回前跳过它们；
/// 后续段之间的分隔符原样保留（`("ni'hao'x", 2)` → `"hao'x"`）。
fn raw_suffix(input: &str, consumed: usize) -> String {
    let mut seen = 0usize;
    for (i, ch) in input.chars().enumerate() {
        if ch != '\'' {
            seen += 1;
        }
        if seen == consumed {
            let rest: String = input.chars().skip(i + 1).collect();
            return rest.trim_start_matches('\'').to_string();
        }
    }
    String::new()
}
