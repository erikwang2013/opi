// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 表情**浏览**表（`data/raw/emoji.tsv`，面板「表情」页）的产物门禁。
//!
//! **数据前提（2026-10-18 变更）**：此前是本仓库从 UTS#51 `emoji-test.txt` 取的
//! 完全限定全量（3963 条，含旗帜/ZWJ 家庭/键帽等）；现改为 **OpenMoji 常用子集**
//! （687 条，9 组，含植物，不含旗帜/肤色变体/ZWJ 组合）。理由：浏览页要的是常用
//! 表情，全量里大量组合序列在字体覆盖与格宽上参差；筛选规则与源 pin 见
//! `scripts/gen_emoji_kaomoji.py`。**门禁不因换源而放松**，仍钉四类：
//!
//! 1. 条数与格式（重复/空白/缺列）—— 防静默少收、防产物损坏；
//! 2. 顺序与分组 —— 分组名/条数、连续覆盖（面板点分类 = `scrollToItem(start)`）；
//! 3. 常用覆盖 —— 每个分类都有代表，用户点名的常用表情必须在；
//! 4. 排除项 —— 旗帜/肤色变体/ZWJ 组合不能混回来（换源时最容易被带回来的东西）。

use engine_core::symbols::SymbolEngine;
use std::collections::HashSet;

/// OpenMoji 常用子集实测条数（2026-10-18，源 SHA-256 pin 下确定性产物）。
/// ⚠️ 与生成器的 400–700 自检是两道坎：那条防筛选失控，这条防**静默少收**。
const EMOJI_COUNT: usize = 687;

/// 面板分类与每类条数（生成器 `EMOJI_ORDER` + 子类上限的实测结果）。
/// 条数一起钉住：只钉总条数时，组间静默挪动（例如植物被并回动物）不会红。
const EXPECTED_GROUPS: &[(&str, usize)] = &[
    ("笑脸", 129),
    ("人物", 75),
    ("动物", 71),
    ("植物", 29),
    ("食物", 83),
    ("旅行", 89),
    ("活动", 42),
    ("物品", 92),
    ("符号", 77),
];

#[test]
fn emoji_browse_table_is_complete() {
    let list = SymbolEngine::builtin_emoji_texts();
    assert_eq!(
        list.len(),
        EMOJI_COUNT,
        "emoji.tsv 条数变了 —— 若刚重 pin 过 OpenMoji，确认筛选结果后更新本常量"
    );
    // 唯一性：重复序列在面板里就是两格一样的东西。
    let uniq: HashSet<&&str> = list.iter().collect();
    assert_eq!(uniq.len(), list.len(), "emoji.tsv 有重复序列");
    for t in &list {
        assert!(!t.is_empty(), "emoji.tsv 有空条目");
        assert!(
            !t.contains(['\t', '\n']),
            "emoji.tsv 条目含制表符/换行: {t:?}"
        );
    }
}

#[test]
fn emoji_browse_order_is_the_panel_order() {
    // 组内按首码位升序（生成器口径：早期码位 ≈ 更早进入 Unicode 的代理指标）。
    // 首条与末条一起钉，防止「换个源、顺序全变」时分组条还看似对齐。
    let list = SymbolEngine::builtin_emoji_texts();
    assert_eq!(list.first().copied(), Some("😀"), "首条应是 grinning face");
    assert_eq!(
        list.last().copied(),
        Some("◽️"),
        "末条应是符号组最后一条（实测；组内码位序）"
    );
}

#[test]
fn emoji_browse_covers_common_emoji_by_category() {
    // 常用度没有可离线复算的权威词频，生成器用「子类上限 + 常见条目白名单」
    // 兜住这批人尽皆知的表情 —— 这里逐类钉代表，白名单被改坏时当场红。
    let list = SymbolEngine::builtin_emoji_texts();
    let has = |s: &str| list.contains(&s);
    for (text, why) in [
        ("😀", "笑脸/笑"),
        ("😂", "笑哭"),
        ("🥰", "爱心脸"),
        ("😭", "大哭"),
        ("😡", "生气"),
        ("👍️", "点赞（VS16 形）"),
        ("🙏", "合掌"),
        ("🐶", "狗"),
        ("🐘", "象"),
        ("🦒", "长颈鹿"),
        ("🌹", "花"),
        ("🍎", "水果"),
        ("🍕", "食物"),
        ("🚗", "汽车"),
        ("🚀", "火箭"),
        ("⚽️", "球类（VS16 形）"),
        ("🎉", "庆祝"),
        ("📱", "手机"),
        ("💡", "灯泡"),
        ("❤️", "爱心（VS16 形）"),
    ] {
        assert!(has(text), "常用表情缺失: {text}（{why}）");
    }
}

#[test]
fn emoji_browse_excludes_flags_skin_tones_and_zwj() {
    // 2026-10-18 的明确排除项。换源/放宽子类上限时最容易被带回来的就是这三类。
    let list = SymbolEngine::builtin_emoji_texts();
    let has = |s: &str| list.contains(&s);
    for bad in ["🇨🇳", "🇺🇸", "👨‍👩‍👧", "🏳️‍🌈", "👍🏽", "🏻", "🦰"]
    {
        assert!(!has(bad), "不该收录: {bad}");
    }
    for t in &list {
        assert!(!t.contains('\u{200D}'), "含 ZWJ 组合序列: {t:?}");
        for c in t.chars() {
            assert!(
                !('\u{1F3FB}'..='\u{1F3FF}').contains(&c),
                "含肤色修饰符: {t:?}"
            );
            assert!(
                !('\u{1F9B0}'..='\u{1F9B3}').contains(&c),
                "含发色组件: {t:?}"
            );
            assert!(
                !('\u{1F1E6}'..='\u{1F1FF}').contains(&c),
                "含区域指示符（旗帜）: {t:?}"
            );
        }
    }
}

/// 分类条的分组（2026-10-12 加，用户裁决「中间是表情分类」）：分组必须**连续覆盖**
/// 全表（无空洞、无重叠）、名字非空、顺序 = 面板分类序 —— 面板点分类就是
/// `grid.scrollToItem(start)`，空洞会让某个分类滚到别的组里去。
#[test]
fn emoji_groups_cover_the_table_contiguously() {
    let groups = SymbolEngine::builtin_emoji_groups();
    let list = SymbolEngine::builtin_emoji_texts();
    let mut next = 0usize;
    let mut names_and_counts: Vec<(&str, usize)> = Vec::new();
    for (name, start, count) in &groups {
        assert!(!name.is_empty(), "组名不得为空");
        assert_eq!(
            *start, next,
            "组 {name} 的起始下标必须接上一组末尾（无空洞/无重叠）"
        );
        assert!(*count > 0, "组 {name} 为空");
        next += count;
        names_and_counts.push((*name, *count));
    }
    assert_eq!(next, list.len(), "分组必须覆盖全表");
    assert_eq!(
        names_and_counts.as_slice(),
        EXPECTED_GROUPS,
        "分组名或每类条数变了 —— 与生成器 EMOJI_ORDER/子类上限的实测结果对不上"
    );
}
