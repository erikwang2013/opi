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

/// 分词回退档的取回上限（2026-10-22 性能，30ms 总目标）。
///
/// 回退档出的是**第一个音节**的整档，宿主通常只显示当前页；而它走的是与主查询
/// 同一条 `rank_and_pick`。宿主传 `SIZE_MAX`（Linux/Windows 的「候选不设上限」）时，
/// 一次回退要排 9–10k 条（`jisuanjj` → `ji` 实测 28ms，pn 余量不够）。封到 64
/// （8 页）—— 回退档最终只出 64 条；主查询池自 2026-10-25 起固定 9216（有限窗口
/// 必须是全量前缀），排序成本随池走，别把这里的 64 当池上限。代价如实记：生僻字
/// 若排在回退 64 条之后要翻不到；那种字应直接打该音节本身（主查询精确层不截）。
pub(crate) const FALLBACK_LIMIT: usize = 64;

/// 分词回退的落点：候选出 `prefix` 的整档，选中后缓冲里留 `suffix` 接着编。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SegmentFallback {
    /// 被消耗的前缀：**音节对齐**，且已去掉分词符（词库键不含 `'`，格式约定）。
    pub prefix: String,
    /// 留在缓冲里的**原文后缀**（其间的 `'` 原样保留）。
    pub suffix: String,
}

/// 词库 `query` / `query_all` 是**前缀**语义（`InMemoryDictionary`/`MmapDictionary`
/// 都按前缀返回）；判「整串有没有命中」必须只认 `pinyin_len == key.len()` ——
/// 否则更长的词以输入为前缀时会被当成「整串命中」，把逐字确认整条挡掉
/// （2026-10-18 用户报障的根因：`zhongguo` 命中 `zhongguoren`）。
///
/// `query_prefix_top(key, 0)` 就是「exact 全量 + 前缀扩展 0 条」：语义与旧实现
/// （`query_all` 后手工过滤）逐条相同，但不物化那数万条前缀扩展（2026-10-22 性能）。
fn has_exact<D: Dictionary + ?Sized>(dict: &D, key: &str) -> bool {
    !dict.query_prefix_top(key, 0).is_empty()
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
///    3b. **不是「≥3 个完整音节 + 更长键前缀」** —— 音节边界全对齐且真有更长的键以整串
///    为前缀时，那是直接的前缀匹配（成语/歇后语的长词），不回退；`zhongguor` 这类
///    半截音节、以及**两音节**输入（2026-10-18 逐字确认的适用面）不在此列
///    （2026-10-22，理由见实现处）；
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
    // 输入整串是**≥3 个完整音节**、且词库真有以它为前缀的更长键 ⇒ 这是一次直接的前缀
    // 匹配（用户裁决原话「输入一个词组，匹配到的在最前面」），不许被第一个字的整档
    // 替换。没有它时 `zhimakaihuajiejiegao` 命中整条歇后语的前缀档会被 `zhi` 的 9813
    // 条整档顶掉，长词在前缀档截断后整个消失。
    // 两音节输入**不在**本条内：那是词的最小单位，2026-10-18 的逐字确认裁决仍适用
    // （`zhongguo` 只命中更长的 `zhongguoren` 时也要能选第一个字）；`zhongguor` 这类
    // 半截音节同样仍走回退。
    if segs.len() >= 3
        && segs.iter().all(|s| is_syllable(s))
        && !dict.query_prefix_top(input, 1).is_empty()
    {
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
    // 只问「有没有命中」：`query_all` 是前缀语义、会把该音节下近万条全部物化
    // （2026-10-22 实测 `ji` 一次 10ms，回退档的整键成本几乎全在这里）；
    // `query_prefix_top(first, 1)` 只物化 exact + 1 条，布尔结论相同。
    if !is_syllable(first) || dict.query_prefix_top(first, 1).is_empty() {
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
