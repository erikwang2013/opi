// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 表情**浏览**表（`data/raw/emoji.tsv`，面板「表情」页的全量）的产物门禁。
//!
//! 与 `symbol_coverage.rs` 同取向：内容归 `scripts/gen_symbols.py`，这里钉的是
//! **用户可见行为**与**静默丢数据**两类。上游由 SHA-256 pin（见生成器），
//! 所以**条数钉死是安全的**：只有人主动重 pin 才会变，变了就该在这里红一次、
//! 由改的人确认「确实是那次升级」（同 `symbol_coverage.rs` 的属性快照）。
//!
//! 来由：「表情中常用主流 emoji 不全」（2026-10-09 用户裁决）—— 换源前缺
//! 1F000–1F1FF / 1F200–1F2FF 整段（🀄🃏🅰️🈁 与全部国旗）、无任何 VS16 序形、
//! 全部 ZWJ 序列。

use engine_core::symbols::SymbolEngine;

/// `emoji-test.txt`（Version 18.0）里 fully-qualified 的条目数，实测。
/// ⚠️ 与生成器的 `len < 3900` 自检是两道坎：那条防风化，这条防**静默少收**。
const EMOJI_COUNT: usize = 3963;

#[test]
fn emoji_browse_table_is_complete() {
    let list = SymbolEngine::builtin_emoji_texts();
    assert_eq!(
        list.len(),
        EMOJI_COUNT,
        "emoji.tsv 条数变了 —— 若刚重 pin 过 emoji-test.txt，确认后更新本常量"
    );
    // 唯一性：重复序列在面板里就是两格一样的东西。
    let uniq: std::collections::HashSet<&&str> = list.iter().collect();
    assert_eq!(uniq.len(), list.len(), "emoji.tsv 有重复序列");
    for t in &list {
        assert!(!t.is_empty(), "emoji.tsv 有空条目");
        assert!(!t.contains(['\t', '\n']), "emoji.tsv 条目含空白: {t:?}");
    }
}

#[test]
fn emoji_browse_order_is_the_official_one() {
    // 文件序 = UTS#51 官方分组序（笑脸在前、旗帜在后）——面板的排序依据就是它。
    let list = SymbolEngine::builtin_emoji_texts();
    assert_eq!(list.first().copied(), Some("😀"), "首条应是 grinning face");
    assert_eq!(
        list.last().copied(),
        Some("🏴󠁧󠁢󠁷󠁬󠁳󠁿"),
        "末条应是威尔士旗（旗帜组末尾 —— 实测，不是猜的）"
    );
}

#[test]
fn emoji_browse_covers_the_2026_10_09_gaps() {
    // 换源前实测缺失、用户点名的几类，逐类钉一个代表：
    let list = SymbolEngine::builtin_emoji_texts();
    let has = |s: &str| list.contains(&s);
    assert!(has("🇨🇳"), "国旗序列（区域指示符成对）");
    assert!(has("🇺🇸"), "国旗序列（美国）");
    assert!(has("👨‍👩‍👧"), "ZWJ 家庭序列");
    assert!(has("🏳️‍🌈"), "ZWJ 彩虹旗（VS16 + ZWJ 混合）");
    assert!(has("1️⃣"), "键帽序列");
    assert!(
        has("#️⃣"),
        "键帽 #（序列以 '#' 开头 —— 加载器按注释滤过一版，少 1 条，测试抓到）"
    );
    assert!(has("🀄"), "麻将块 U+1F000–1F0FF");
    assert!(has("🃏"), "扑克（同块）");
    assert!(has("🅰️"), "带圈字母 U+1F100–1F1FF（VS16 形）");
    assert!(has("🈁"), "带圈表意文字 U+1F200–1F2FF");
    assert!(has("❤️"), "VS16 爱心（旧表只有裸 ❤，渲染成单色字形）");
    assert!(!has("❤"), "裸 ❤ 不该单独出现 —— 那是旧的单色形态");
}

/// 分类条的分组（2026-10-12 加，用户裁决「中间是表情分类」）：分组必须**连续覆盖**
/// 全表（无空洞、无重叠）、名字非空、顺序 = 官方组序 —— 面板点分类就是
/// `grid.scrollToItem(start)`，空洞会让某个分类滚到别的组里去。
#[test]
fn emoji_groups_cover_the_table_contiguously() {
    let groups = SymbolEngine::builtin_emoji_groups();
    let list = SymbolEngine::builtin_emoji_texts();
    let mut next = 0usize;
    let mut names: Vec<&str> = Vec::new();
    for (name, start, count) in &groups {
        assert!(!name.is_empty(), "组名不得为空");
        assert_eq!(
            *start, next,
            "组 {name} 的起始下标必须接上一组末尾（无空洞/无重叠）"
        );
        assert!(*count > 0, "组 {name} 为空");
        next += count;
        names.push(name);
    }
    assert_eq!(next, list.len(), "分组必须覆盖全表");
    // 官方 9 组（Component 全是 `component` 状态、被「只收完全限定」滤掉 —— 生成器
    // 的 EMOJI_GROUP_CN 仍列着它，上游改版时红的在生成期）。
    assert_eq!(
        names,
        [
            "笑脸", "人物", "动物", "食物", "旅行", "活动", "物品", "符号", "旗帜"
        ]
    );
}
