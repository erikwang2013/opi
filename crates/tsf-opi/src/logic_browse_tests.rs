// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 符号浏览（「符号大全」）的宿主键路由门禁（lead 裁决 2026-10-10 的条件 ①②③）。
//!
//! 判据（契约全文在 `engine-core/src/engine_symbol_browse.rs` 头）：
//! - 浏览期间只有 `1`-`9`（激活：类目下钻 / 符号上屏）、`-`/`=`（浏览翻页）、Esc
//!   （退出并**吞掉**）归浏览；**其余键一律先退出浏览恢复真缓冲、再按正常路由处理**
//!   —— 尤其是不许让「符号」标题落进任何读缓冲的路径。
//! - 切走（W1）的顺序是**先退浏览再提交**；本文件用「先读后读」的对照把
//!   这个顺序钉成承重条件（不退出时读到的就是面板首项「1.常用」）。
//!
//! 两轨逐条同构：本文件 ↔ `crates/fcitx5-opi/src/input_method_browse_tests.rs`
//! （同名同序；进入/退出两轨同为「宿主直接调状态方法」，键路由逐键同构 —— 结论别名：
//! `Consumed`≡`EngineHandled`、`Commit`≡`Input`、`Unhandled`≡`PassThrough`）。
//! fcitx5 轨文件末尾另有一条 `c_entry_symbol_browse_actions_and_loader`（C 出口面）：
//! Windows 侧本回合无 C 出口、无装载接线（2026-10-10 裁决 —— 符号页由桌面 Compose
//! 自渲染、点选走 `commit_text` 直插），故本文件没有对应用例。

use super::*;

/// 真资产（与桌面打包的那一份 `crates/fcitx5-opi/data/` 同字节；后者与 Android
/// 资产的逐字节门禁在 `fcitx5-opi/src/data_dir.rs` 的 `bundled_asset_matches_android`）。
/// Windows 侧本回合无装载接线（见 logic.rs 那块注释），本文件用真数据钉状态机。
const ASSET: &str = include_str!("../../fcitx5-opi/data/symbol_categories.tsv");

/// 内置回退词库（ni → 你 / hao → 好）+ 真符号类目数据。
fn browse_state() -> TsfLogic {
    let mut s = TsfLogic::load(None).expect("内置回退词库");
    assert_eq!(s.load_symbol_categories(ASSET), 0, "真资产无坏行");
    s
}

fn typed(s: &mut TsfLogic, word: &str) {
    for c in word.chars() {
        s.input_key(c as u32, 0);
    }
}

// ---- 条件 ①：浏览态劫持 buffer/候选；数字下钻、翻页、Esc ----

#[test]
fn browse_hijacks_panel_then_digit_drills_and_commits() {
    let mut s = browse_state();
    assert!(s.enter_symbol_browse());
    assert_eq!(s.buffer(), "符号", "浏览态 buffer = 标题（面板靠它显示）");
    assert_eq!(s.candidates()[0], "1.常用", "一级第一页 = 类目 1..8");
    // 按 `1` → 下钻：无提交文本，标题带类目名（CompositionChanged = 面板要刷新）
    assert_eq!(s.input_key('1' as u32, 0), KeyOutcome::CompositionChanged);
    assert_eq!(s.buffer(), "符号·常用");
    assert!(s.is_symbol_browsing(), "下钻不退出");
    // 二级第一页 = 该类目符号；再按 `1` → 上屏第一个符号并**回一级**（不是退出）
    assert_eq!(s.candidates()[0], "，");
    assert_eq!(
        s.input_key('1' as u32, 0),
        KeyOutcome::Commit("，".to_string())
    );
    assert_eq!(s.buffer(), "符号", "提交后回一级");
    assert_eq!(s.candidates()[0], "1.常用");
    assert!(s.is_symbol_browsing());
}

#[test]
fn category_labels_are_uncommittable_through_keys() {
    // 条件 ③ 的键路由证明：一级任何数字都只可能下钻（CompositionChanged，绝无 Commit）
    let mut s = browse_state();
    assert!(s.enter_symbol_browse());
    for d in '1'..='8' {
        assert!(s.enter_symbol_browse()); // 幂等 no-op，保证在一级
        assert_eq!(
            s.input_key(d as u32, 0),
            KeyOutcome::CompositionChanged,
            "一级 `{d}` = 下钻，不是提交"
        );
        s.exit_symbol_browse();
    }
    // 越界数字（本页没有第 9 项时）也**吞掉不直通**（浏览是模态）
    assert!(s.enter_symbol_browse());
    assert_eq!(s.input_key('1' as u32, 0), KeyOutcome::CompositionChanged); // 进二级
    for _ in 0..9 {
        s.input_key('=' as u32, 0); // 常用 78 个符号：页 9 = 第 73..78 项（6 条）
    }
    assert_eq!(s.candidates().len(), 6, "末页只剩 6 条");
    assert_eq!(
        s.input_key('7' as u32, 0),
        KeyOutcome::CompositionChanged,
        "越界数字吞掉（不 Unhandled）"
    );
    assert_eq!(s.buffer(), "符号·常用", "越界不动层级");
}

#[test]
fn minus_and_equal_page_in_browse() {
    let mut s = browse_state();
    assert!(s.enter_symbol_browse());
    assert_eq!(s.input_key('=' as u32, 0), KeyOutcome::CompositionChanged);
    assert_eq!(s.page(), 1);
    assert_eq!(s.candidates()[0], "9.平假", "第二页从第 9 个类目开始");
    assert_eq!(s.input_key('-' as u32, 0), KeyOutcome::CompositionChanged);
    assert_eq!(s.page(), 0, "'-' 上一页");
    assert_eq!(s.input_key('-' as u32, 0), KeyOutcome::CompositionChanged);
    assert_eq!(s.page(), 0, "首页钳制（不越界）");
    // 显示集合翻了，真缓冲与层级都不动
    assert_eq!(s.buffer(), "符号");
    // 抬起对称：`=` 按下被消费，抬起也必须被消费（不直通）
    assert_eq!(
        s.input_key('=' as u32, KEY_STATE_RELEASED),
        KeyOutcome::Consumed
    );
}

#[test]
fn esc_exits_and_swallows_with_release_symmetry() {
    let mut s = browse_state();
    assert!(s.enter_symbol_browse());
    assert_eq!(
        s.input_key(KEY_ESCAPE, 0),
        KeyOutcome::Consumed,
        "浏览态 Esc = 退出并吞掉"
    );
    assert!(!s.is_symbol_browsing());
    assert_eq!(
        s.input_key(KEY_ESCAPE, KEY_STATE_RELEASED),
        KeyOutcome::Consumed,
        "抬起按按下结论（不放行一个被拦的 keyup）"
    );
    // 非浏览态 Esc 仍是老行为放行（Tab 同档）
    assert_eq!(s.input_key(KEY_ESCAPE, 0), KeyOutcome::Unhandled);
    assert_eq!(
        s.input_key(KEY_ESCAPE, KEY_STATE_RELEASED),
        KeyOutcome::Unhandled
    );
}

// ---- 条件 ②：未处理键 = 先退出浏览（恢复真缓冲）再正常处理 ----

#[test]
fn unhandled_letter_exits_browse_then_types_into_real_buffer() {
    let mut s = browse_state();
    typed(&mut s, "ni");
    assert!(s.enter_symbol_browse());
    assert_eq!(s.input_key('h' as u32, 0), KeyOutcome::CompositionChanged);
    assert!(!s.is_symbol_browsing(), "未处理键先退出浏览");
    assert_eq!(s.buffer(), "nih", "退出后接着打进真缓冲");
    assert_eq!(s.candidates()[0], "你", "候选来自真词库（不是类目表）");
}

#[test]
fn space_enter_backspace_exit_first_and_never_see_the_title() {
    let mut s = browse_state();
    typed(&mut s, "ni");
    assert!(s.enter_symbol_browse());
    // 空格：退出浏览 + 按拼音规则提交首候选（绝不提交标题）
    assert_eq!(
        s.input_key(KEY_SPACE, 0),
        KeyOutcome::Commit("你".to_string())
    );
    assert_eq!(s.buffer(), "");
    assert!(!s.is_symbol_browsing());
    // 空真缓冲时的退格：退出浏览 + 放行（老行为 —— 不许吞键）
    assert!(s.enter_symbol_browse());
    assert_eq!(s.input_key(KEY_BACK_SPACE, 0), KeyOutcome::Unhandled);
    assert!(!s.is_symbol_browsing());
    // 回车：退出浏览 + 提交首候选（真词库）
    typed(&mut s, "hao");
    assert!(s.enter_symbol_browse());
    assert_eq!(
        s.input_key(KEY_RETURN, 0),
        KeyOutcome::Commit("好".to_string())
    );
    // Delete 同退格路径（浏览态下也被「先退出」覆盖）
    assert!(s.enter_symbol_browse());
    assert_eq!(s.input_key(KEY_DELETE, 0), KeyOutcome::Unhandled);
    assert!(!s.is_symbol_browsing());
}

#[test]
fn page_keys_and_shift_are_literal_unhandled() {
    // lead 裁决的键集是**闭集**（1-9 / `-`/`=` / Esc）：PageDown、Shift 都算未处理
    let mut s = browse_state();
    assert!(s.enter_symbol_browse());
    assert_eq!(
        s.input_key(KEY_PAGE_DOWN, 0),
        KeyOutcome::CompositionChanged
    );
    assert!(!s.is_symbol_browsing(), "PageDown 不在键集里 → 先退出浏览");
    assert!(s.enter_symbol_browse());
    assert_eq!(s.input_key(KEY_SHIFT, 0), KeyOutcome::Consumed);
    assert!(!s.is_symbol_browsing(), "裸 Shift 不在键集里 → 先退出浏览");
}

// ---- 条件 ②（W1 承重条件）：切走前必须先退浏览 ----

#[test]
fn switch_away_must_exit_first_or_it_commits_the_title() {
    // fcitx5 轨的 W1 顺序在 C 侧（glue 的提交路径先调 opi_fcitx5_symbol_browse(0)）；
    // Windows 轨没有对应的「切走提交」点（tsf.rs 的 Deactivate 只清
    // 语言栏/线程管理器/client_id，不提交缓冲）—— 但「顺序是承重的」两轨同源，
    // 故在状态机层钉住：**不退出**时读到的是浏览内容（会让调用方上屏「符号」
    // 「1.常用」）。
    let mut s = browse_state();
    typed(&mut s, "ni");
    assert!(s.enter_symbol_browse());
    // 负对照：不退出 → 面板首项是类目、缓冲是标题（这就是「顺序错了」的形态）
    assert_eq!(s.candidates()[0], "1.常用");
    assert_eq!(s.buffer(), "符号");
    // 正确顺序：先 exit_symbol_browse() → 返回后已是真缓冲/真候选
    s.exit_symbol_browse();
    assert_eq!(s.buffer(), "ni", "同步：退出返回后 buffer 已是真缓冲");
    assert_eq!(
        s.candidates()[0],
        "你",
        "切走「有候选上首候选」= 真词库候选"
    );
    // 面板点击/确认第一条 = 真候选（不是类目展示串）
    assert_eq!(s.select(0), "你");
    // 空真缓冲：提交原文 = 空（而不是「符号」两个字）
    let mut e = browse_state();
    assert!(e.enter_symbol_browse());
    assert_eq!(e.buffer(), "符号");
    e.exit_symbol_browse();
    assert_eq!(e.buffer(), "", "无候选无原文：提交空串");
    assert!(e.candidates().is_empty());
}
