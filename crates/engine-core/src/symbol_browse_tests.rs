// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `symbol_browse` 的解析与两级状态机（`#[path]` 引入 symbol_browse.rs，保持各文件 <500 行）。
//! 真资产（22 类目 / 序号连续 / 类目非空）另在 `tests/symbol_browse.rs` 里钉 ——
//! 那边读的是 Android 部署的那一份，这里只用内联夹具钉**解析语义**（与 Kotlin
//! `SymbolCategories.parse` 逐条对齐；两边的对照表见各用例的注释）。

use super::*;

/// 内联夹具：两条合法行 + 三条坏行（缺列 / 序号非数字 / 无符号）。
const FIXTURE: &str = "\
1\t常用\t， 。 、
2\t英文\t! \" # $\t忽略的第四列
3\t缺列
x\t坏序号\t√
4\t空符号\t
\t空序号\t√

5\t拖尾空格\t  a b  \t
";

#[test]
fn parse_mirrors_kotlin_semantics() {
    let (cats, skipped) = parse_categories(FIXTURE);
    // 合法 = 序号 1、2、5 三行（序号 4 那行「无符号」被跳过，序号列不是行号）
    let labels: Vec<&str> = cats.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["常用", "英文", "拖尾空格"]);
    // 坏行 = 缺列 / 坏序号 / 空符号 / 空序号 —— 4 条（空行不计，与 Kotlin `isBlank → continue` 同义）
    assert_eq!(skipped, 4);
    // 序号取自**数据列**（不是行号），非法序号的行整行跳过：
    assert_eq!(cats[0].index, 1);
    assert_eq!(cats[1].index, 2);
    assert_eq!(cats[2].index, 5);
    // 第四列忽略（Kotlin 也只取 parts[0..3]）：
    assert_eq!(cats[1].symbols, ["!", "\"", "#", "$"]);
    // 符号按空格 token：`  a b  ` 前后空格与连续空格都不产生空 token（Kotlin filter{isNotEmpty} 同义）
    assert_eq!(cats[2].symbols, ["a", "b"]);
}

#[test]
fn parse_keeps_multi_char_tokens_whole() {
    // 组合序列与颜文字必须整块（不能按码点切）：o+U+0358 是 2 个码点 1 个 token
    let (cats, skipped) = parse_categories("21\t土音\to\u{0358} O\u{0358} ╮(╯▽╰)╭\n");
    assert_eq!(skipped, 0);
    assert_eq!(cats[0].symbols, ["o\u{0358}", "O\u{0358}", "╮(╯▽╰)╭"]);
}

#[test]
fn parse_tolerates_crlf_and_empty_input() {
    // Windows 换行（str::lines 剥 \r，同 Kotlin lineSequence）：
    let (cats, skipped) = parse_categories("1\t常用\t，\r\n2\t英文\t!\r\n");
    assert_eq!(cats.len(), 2);
    assert_eq!(skipped, 0);
    assert_eq!(cats[1].symbols, ["!"]);
    // 空输入：0 类目、0 坏行 —— 由调用方（load_symbol_categories）判「入口禁用」
    let (cats, skipped) = parse_categories("");
    assert!(cats.is_empty());
    assert_eq!(skipped, 0);
}

#[test]
fn inactive_state_answers_empty_until_enter() {
    // 装载 ≠ 浏览：`new` 只装数据，未 `enter` 前 select/candidates 一律空答
    // （这是「打字不吃数据集」那一族的对象级形态，引擎级见 tests/symbol_browse.rs）。
    let (cats, _) = parse_categories(FIXTURE);
    let mut b = SymbolBrowse::new(cats);
    assert!(!b.is_active());
    assert!(!b.is_empty());
    assert_eq!(b.preedit(), "", "非浏览态不给标题（此时 buffer 是真缓冲）");
    assert!(b.candidates(usize::MAX).is_empty());
    assert_eq!(b.select(0), Selection::Miss);
    assert!(b.enter());
    assert!(b.candidates(usize::MAX).len() == 3);
    b.exit();
    assert!(
        b.select(0) == Selection::Miss,
        "退出后再选 = Miss（不会误提交）"
    );
    // 退出不丢数据：不必重装载即可再进
    assert!(b.enter());
    assert_eq!(b.level2(), None, "再进 = 从一级开始");
}

#[test]
fn enter_while_active_keeps_level_and_exit_is_idempotent() {
    // 宿主热键是 toggle；进入接口在浏览态必须 no-op（重复按不把用户从二级弹回一级）
    let (cats, _) = parse_categories(FIXTURE);
    let mut b = SymbolBrowse::new(cats);
    assert!(b.enter());
    assert_eq!(b.select(0), Selection::Drill);
    assert!(b.enter(), "浏览态再进 = true 且 no-op");
    assert_eq!(b.level2(), Some(0), "层级保持");
    b.exit();
    b.exit(); // 幂等（宿主提交路径每次切走都无条件先调）
    assert!(!b.is_active());
}

#[test]
fn enter_drill_commit_cycle() {
    let (cats, _) = parse_categories(FIXTURE);
    let mut b = SymbolBrowse::new(cats);
    assert!(b.enter());
    // 一级：候选 = 序号.名称；预编辑 = 标题
    assert_eq!(b.level2(), None);
    assert_eq!(b.preedit(), BROWSE_TITLE);
    let l1 = b.candidates(usize::MAX);
    assert_eq!(
        l1[0].text, "常用",
        "类目条款式 = 纯名字（序号由宿主面板画）"
    );
    assert_eq!(l1[2].text, "拖尾空格");
    // 下钻：无提交文本，进二级
    assert_eq!(b.select(0), Selection::Drill);
    assert_eq!(b.level2(), Some(0));
    assert_eq!(b.preedit(), "符号·常用");
    // 二级：候选 = 符号
    let l2 = b.candidates(usize::MAX);
    assert_eq!(l2[0].text, "，");
    // 选中符号：提交并回一级
    assert_eq!(b.select(2), Selection::Commit("、".into()));
    assert_eq!(b.level2(), None);
    assert_eq!(b.preedit(), BROWSE_TITLE);
    // 越界：一级越界 / 二级越界都 Miss 且状态不变
    assert_eq!(b.select(9), Selection::Miss);
    assert_eq!(b.level2(), None);
}

#[test]
fn candidates_limit_truncates_without_reindex() {
    let (cats, _) = parse_categories(FIXTURE);
    let mut b = SymbolBrowse::new(cats);
    assert!(b.enter());
    // limit 是「取多少条」，不是「第几条开始」—— 分页归宿主 skip/take（与 fetched 同义）
    let top = b.candidates(2);
    assert_eq!(top.len(), 2);
    assert_eq!(top[0].text, "常用");
    assert_eq!(top[1].text, "英文");
}
