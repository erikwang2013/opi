// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **排序层开关** [`RankOptions`] —— 2026-10-14 从 `candidates.rs` 拆出（加
//! `segment_fallback` 后那里顶破 500 行门禁；拆分先例与 `candidates_scheme.rs` /
//! `engine_dicts.rs` 同套）。**语义一字未动**，只是搬了个家。
//!
//! 为什么是它单独成文件：它是本层唯一的**入参配置结构**（纯数据 + 一个 `Default`，
//! 不依赖 `rank_and_pick_with` 里的任何东西），与「怎么排」互不牵连 ——
//! `candidates.rs` 保留算法与阈值，这里只保留开关。
//!
//! 对外路径不变：`candidates.rs` 里 `pub use` 回来，调用方仍写
//! `candidates::RankOptions` 或 `engine_core::candidates::RankOptions`。

/// 排序层的**开关**（模糊音/纠错/简拼/分词回退；见 `Engine` 的同名字段）。
///
/// 默认 = **全开**：这是本层的既有行为，也是 [`crate::candidates::rank_and_pick`]
/// 的语义（测试与旧调用方一行不动）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankOptions {
    /// 模糊音启用集（逐对位掩码，位序见 `fuzzy.rs::FUZZY_PAIRS`；`FUZZY_ALL` = 旧「开」）。
    /// 整串变体 + 逐音节兜底共用它，表见 `fuzzy.rs`。
    pub fuzzy: crate::fuzzy::FuzzyMask,
    /// 拼音纠错开关（2026-10-13 用户裁决「混合参与排序」；变体见 `correction.rs`）。
    /// 与模糊音**同一条管线、同一档**（非精确、追加式），只是变体生成方式不同。
    pub correction: bool,
    /// 简拼（声母缩写 `nh → 你好`，展开见 `jianpin.rs`）。
    pub jianpin: bool,
    /// **简体优先**（2026-10-13 用户裁决「如果是混合就优先显示简体」）：截断前把含
    /// 繁体专属字的候选稳定分区到后面，见 `trad_chars::demote_traditional`。
    /// 默认 `false` = 本层既有行为（老调用方一行不动）；`Engine` 在**简体模式**下显式开。
    pub prefer_simplified: bool,
    /// **分词回退 + 逐音节汤**（`rank_and_pick_with` 中段那两档）的开关。
    /// 默认 `true` = 本层既有行为。**五笔关掉它**（2026-10-14，M4 3b）：那两档的判据
    /// 建立在**拼音音节**上（`segment` / `boundary_aligned`），拿音节规则去猜五笔
    /// 码串是「似是而非」—— 候选只该来自「码是缓冲的前缀」这一条；且它们若漏进
    /// 码串，候选层给出的会是**更短前缀**的候选项，而收尾层对五笔整段消耗
    /// （`engine_segment.rs`），两层的「匹配了多少」对不上（见 `engine_wubi.rs` 头）。
    pub segment_fallback: bool,
    /// **候选三档**（2026-10-14 用户裁决「优先给符合拼音的候选字，然后是词组，最后是
    /// 联想的」；实现见 `candidates_tiers.rs`）。默认 `true` = 拼音类输入的产品行为；
    /// **码类方案（笔画 / 五笔）显式关** —— 缓冲是编码不是拼音，拿「单字/词组/联想」去切
    /// 码串是似是而非（与 `segment_fallback` 在五笔上的裁决同一条理由）。
    /// ⚠️ 双拼那条路**跨解析合并之后**才分档，故它给每次解析传 `false`、在合并处自己调
    /// `candidates_tiers::apply`（见 `engine_shuangpin.rs`）。
    pub tiers: bool,
}

impl Default for RankOptions {
    fn default() -> Self {
        Self {
            fuzzy: crate::fuzzy::FUZZY_ALL,
            correction: true,
            jianpin: true,
            prefer_simplified: false,
            segment_fallback: true,
            tiers: true,
        }
    }
}
