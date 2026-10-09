// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 符号浏览（「符号大全」）的宿主键路由门禁（lead 裁决 2026-10-10 的条件 ①②③）。
//!
//! 判据（契约全文在 `engine-core/src/engine_symbol_browse.rs` 头）：
//! - 浏览期间只有 `1`-`9`（激活：类目下钻 / 符号上屏）、`-`/`=`（浏览翻页）、Esc
//!   （退出并**吞掉**）归浏览；**其余键一律先退出浏览恢复真缓冲、再按正常路由处理**
//!   —— 尤其是不许让「符号」标题落进任何读缓冲的路径。
//! - 切走/切英文（W1）的顺序是**先退浏览再提交**；本文件用「先读后读」的对照把
//!   这个顺序钉成承重条件（不退出时 W1 看到的就是面板首项「1.常用」）。
//!
//! 两轨逐条同构：本文件 ↔ `crates/tsf-opi/src/logic_browse_tests.rs`（同名同序；
//! 进入/退出在 tsf 轨与 fcitx5 轨同为「宿主直接调状态方法」，键路由部分逐键同构）。

use super::*;
use crate::candidate::CandidateState;

/// 真资产（**打包的那一份**：`crates/fcitx5-opi/data/`，CMake 装到
/// `${CMAKE_INSTALL_DATADIR}/opi/`；与 Android 资产的逐字节门禁在 data_dir.rs）。
const ASSET: &str = include_str!("../data/symbol_categories.tsv");

/// 内置回退词库（ni → 你 / hao → 好）+ 真符号类目数据。
fn browse_state() -> CandidateState {
    let mut s = CandidateState::load(None).expect("内置回退词库");
    assert_eq!(s.load_symbol_categories(ASSET), 0, "真资产无坏行");
    s
}

fn typed(s: &mut CandidateState, word: &str) {
    for c in word.chars() {
        handle_key(s, c as u32, 0);
    }
}

// ---- 条件 ①：浏览态劫持 buffer/候选；数字下钻、翻页、Esc ----

#[test]
fn browse_hijacks_panel_then_digit_drills_and_commits() {
    let mut s = browse_state();
    assert!(s.enter_symbol_browse());
    assert_eq!(s.buffer(), "符号", "浏览态 buffer = 标题（面板靠它显示）");
    assert_eq!(s.candidates()[0], "1.常用", "一级第一页 = 类目 1..8");
    // 按 `1` → 下钻：无提交文本，标题带类目名
    assert_eq!(handle_key(&mut s, '1' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(s.buffer(), "符号·常用");
    assert!(s.is_symbol_browsing(), "下钻不退出");
    // 二级第一页 = 该类目符号；再按 `1` → 上屏第一个符号并**回一级**（不是退出）
    assert_eq!(s.candidates()[0], "，");
    assert_eq!(
        handle_key(&mut s, '1' as u32, 0),
        KeyAction::Input("，".to_string())
    );
    assert_eq!(s.buffer(), "符号", "提交后回一级");
    assert_eq!(s.candidates()[0], "1.常用");
    assert!(s.is_symbol_browsing());
}

#[test]
fn category_labels_are_uncommittable_through_keys() {
    // 条件 ③ 的键路由证明：一级任何数字都只可能下钻（EngineHandled，绝无 Input）
    let mut s = browse_state();
    assert!(s.enter_symbol_browse());
    for d in '1'..='8' {
        assert!(s.enter_symbol_browse()); // 幂等 no-op，保证在一级
        assert_eq!(
            handle_key(&mut s, d as u32, 0),
            KeyAction::EngineHandled,
            "一级 `{d}` = 下钻，不是提交"
        );
        s.exit_symbol_browse();
    }
    // 越界数字（本页没有第 9 项时）也**吞掉不直通**（浏览是模态）
    assert!(s.enter_symbol_browse());
    assert_eq!(handle_key(&mut s, '1' as u32, 0), KeyAction::EngineHandled); // 进二级
    for _ in 0..9 {
        handle_key(&mut s, '=' as u32, 0); // 常用 78 个符号：页 9 = 第 73..78 项（6 条）
    }
    assert_eq!(s.candidates().len(), 6, "末页只剩 6 条");
    assert_eq!(
        handle_key(&mut s, '7' as u32, 0),
        KeyAction::EngineHandled,
        "越界数字吞掉（不 PassThrough）"
    );
    assert_eq!(s.buffer(), "符号·常用", "越界不动层级");
}

#[test]
fn minus_and_equal_page_in_browse() {
    let mut s = browse_state();
    assert!(s.enter_symbol_browse());
    assert_eq!(handle_key(&mut s, '=' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(s.page(), 1);
    assert_eq!(s.candidates()[0], "9.平假", "第二页从第 9 个类目开始");
    assert_eq!(handle_key(&mut s, '-' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(s.page(), 0, "'-' 上一页");
    assert_eq!(handle_key(&mut s, '-' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(s.page(), 0, "首页钳制（不越界）");
    // 显示集合翻了，真缓冲与层级都不动
    assert_eq!(s.buffer(), "符号");
    // 抬起对称：`=` 是 EngineHandled，抬起也必须 EngineHandled（不直通）
    assert_eq!(
        handle_key(&mut s, '=' as u32, KEY_STATE_RELEASED),
        KeyAction::EngineHandled
    );
}

#[test]
fn esc_exits_and_swallows_with_release_symmetry() {
    let mut s = browse_state();
    assert!(s.enter_symbol_browse());
    assert_eq!(
        handle_key(&mut s, KEY_ESCAPE, 0),
        KeyAction::EngineHandled,
        "浏览态 Esc = 退出并吞掉"
    );
    assert!(!s.is_symbol_browsing());
    assert_eq!(
        handle_key(&mut s, KEY_ESCAPE, KEY_STATE_RELEASED),
        KeyAction::EngineHandled,
        "抬起按按下结论（不放行一个被拦的 keyup）"
    );
    // 非浏览态 Esc 仍是老行为直通（Tab 同档）
    assert_eq!(handle_key(&mut s, KEY_ESCAPE, 0), KeyAction::PassThrough);
    assert_eq!(
        handle_key(&mut s, KEY_ESCAPE, KEY_STATE_RELEASED),
        KeyAction::PassThrough
    );
}

// ---- 条件 ②：未处理键 = 先退出浏览（恢复真缓冲）再正常处理 ----

#[test]
fn unhandled_letter_exits_browse_then_types_into_real_buffer() {
    let mut s = browse_state();
    typed(&mut s, "ni");
    assert!(s.enter_symbol_browse());
    assert_eq!(handle_key(&mut s, 'h' as u32, 0), KeyAction::EngineHandled);
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
        handle_key(&mut s, KEY_SPACE, 0),
        KeyAction::Input("你".to_string())
    );
    assert_eq!(s.buffer(), "");
    assert!(!s.is_symbol_browsing());
    // 空真缓冲时的退格：退出浏览 + 直通（老行为 —— 不许吞键）
    assert!(s.enter_symbol_browse());
    assert_eq!(
        handle_key(&mut s, KEY_BACK_SPACE, 0),
        KeyAction::PassThrough
    );
    assert!(!s.is_symbol_browsing());
    // 回车：退出浏览 + 提交首候选（真词库）
    typed(&mut s, "hao");
    assert!(s.enter_symbol_browse());
    assert_eq!(
        handle_key(&mut s, KEY_RETURN, 0),
        KeyAction::Input("好".to_string())
    );
    // Delete 同退格路径（浏览态下也被「先退出」覆盖）
    assert!(s.enter_symbol_browse());
    assert_eq!(handle_key(&mut s, KEY_DELETE, 0), KeyAction::PassThrough);
    assert!(!s.is_symbol_browsing());
}

#[test]
fn page_keys_and_shift_are_literal_unhandled() {
    // lead 裁决的键集是**闭集**（1-9 / `-`/`=` / Esc）：PageDown、Shift 都算未处理
    let mut s = browse_state();
    assert!(s.enter_symbol_browse());
    assert_eq!(
        handle_key(&mut s, KEY_PAGE_DOWN, 0),
        KeyAction::EngineHandled
    );
    assert!(!s.is_symbol_browsing(), "PageDown 不在键集里 → 先退出浏览");
    assert!(s.enter_symbol_browse());
    assert_eq!(handle_key(&mut s, KEY_SHIFT_L, 0), KeyAction::EngineHandled);
    assert!(!s.is_symbol_browsing(), "裸 Shift 不在键集里 → 先退出浏览");
}

// ---- 条件 ②（W1 承重条件）：切走/切英文前必须先退浏览 ----

#[test]
fn switch_away_must_exit_first_or_it_commits_the_title() {
    // fcitx5-glue 的 W1 顺序：「先 opi_fcitx5_symbol_browse(0) → 再按 W1 提交
    // （有候选首候选、无候选原文）」。本用例把「顺序是承重的」钉成事实：
    // **不退出**时 W1 读到的缓冲/首候选就是浏览内容（会让用户上屏「符号」「1.常用」）。
    // 状态机侧的同序实现 = `switch_mode_confirmed`（见下一条用例）。
    let mut s = browse_state();
    typed(&mut s, "ni");
    assert!(s.enter_symbol_browse());
    // 负对照：不退出 → 面板首项是类目、缓冲是标题（这就是「顺序错了」的形态）
    assert_eq!(s.candidates()[0], "1.常用");
    assert_eq!(s.buffer(), "符号");
    // 正确顺序：action 0（= exit_symbol_browse）→ 同步，返回后已是真缓冲/真候选
    s.exit_symbol_browse();
    assert_eq!(s.buffer(), "ni", "同步：退出返回后 buffer 已是真缓冲");
    assert_eq!(s.candidates()[0], "你", "W1「有候选上首候选」= 真词库候选");
    // 面板点击/确认第一条 = 真候选（不是类目展示串）
    assert_eq!(s.select(0), "你");
    // 空真缓冲：W1 提交原文 = 空（而不是「符号」两个字）
    let mut e = browse_state();
    assert!(e.enter_symbol_browse());
    assert_eq!(e.buffer(), "符号");
    e.exit_symbol_browse();
    assert_eq!(e.buffer(), "", "无候选无原文：W1 提交空串");
    assert!(e.candidates().is_empty());
}

// ---- W1 的状态机实现：switch_mode_confirmed（确认 + 切换，返回待上屏文本）----

#[test]
fn switch_mode_confirmed_takes_first_candidate_or_raw_buffer() {
    // 有候选 → 首候选；无候选 → **原文**（与 toggle_symbol 的「无候选丢弃」不同支）
    let mut s = browse_state();
    typed(&mut s, "ni");
    assert_eq!(s.candidates()[0], "你", "前置：真词库候选");
    // 「先确认后切」是承重的：顺序反了，英文模式候选集为空，这里会拿到原文 "ni"
    assert_eq!(
        s.switch_mode_confirmed(Mode::English),
        "你",
        "有候选上首候选"
    );
    assert_eq!(s.buffer(), "", "确认后缓冲收干净");
    assert_eq!(s.mode(), Mode::English, "切换真的发生了");
    let mut z = browse_state();
    typed(&mut z, "qqq"); // 别拿 zzz：它是 😴 的关键字，会走「有候选」那一支
    assert!(z.candidates().is_empty(), "前置：qqq 无候选");
    assert_eq!(
        z.switch_mode_confirmed(Mode::English),
        "qqq",
        "无候选上原文（不丢弃）"
    );
    assert_eq!(z.buffer(), "");
    assert_eq!(z.mode(), Mode::English);
}

#[test]
fn switch_mode_confirmed_exits_browse_first_never_commits_the_title() {
    // 顺序承重：浏览态下 buffer()/candidates() 是面板内容 → 确认必须先退浏览
    let mut s = browse_state();
    typed(&mut s, "ni");
    assert!(s.enter_symbol_browse());
    assert_eq!(s.buffer(), "符号", "负对照：不退出就是标题");
    assert_eq!(
        s.switch_mode_confirmed(Mode::English),
        "你",
        "先退浏览 → 真缓冲的候选"
    );
    assert!(!s.is_symbol_browsing(), "确认顺带退了浏览");
    assert_eq!(s.buffer(), "");
    // 空真缓冲（进浏览前的常见形态）：确认 = 空串，绝不是「符号」/「1.常用」
    let mut e = browse_state();
    assert!(e.enter_symbol_browse());
    assert_eq!(
        e.switch_mode_confirmed(Mode::English),
        "",
        "空真缓冲：W1 提交空串"
    );
    assert!(!e.is_symbol_browsing());
    assert_eq!(e.mode(), Mode::English);
}

// ---- C 入口面（opi_fcitx5_symbol_browse / opi_fcitx5_load_symbols）----

#[test]
fn c_entry_symbol_browse_actions_and_loader() {
    // 碰 SINGLETON 的用例一律用 serial_install（crate 级唯一测试锁）
    let _g = crate::serial_install();
    // 数据未装：进入被拒、切换无事发生（入口禁用的表达 = 返回 0）
    assert_eq!(unsafe { crate::opi_fcitx5_symbol_browse(1) }, 0);
    assert_eq!(unsafe { crate::opi_fcitx5_symbol_browse(2) }, 0);
    // 装载真资产（走「宿主 locate 路径、Rust 读文件」那条缝）
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/symbol_categories.tsv");
    assert_eq!(
        unsafe { crate::opi_fcitx5_load_symbols(path.as_ptr(), path.len()) },
        0,
        "真资产无坏行"
    );
    // action 2 切换：进（1）→ 退（0）；action 0 幂等
    assert_eq!(unsafe { crate::opi_fcitx5_symbol_browse(2) }, 1);
    assert_eq!(
        crate::with_state(|s| s.buffer()).unwrap(),
        "符号",
        "进入后 buffer = 标题"
    );
    assert_eq!(unsafe { crate::opi_fcitx5_symbol_browse(2) }, 0);
    assert_eq!(unsafe { crate::opi_fcitx5_symbol_browse(0) }, 0);
    assert_eq!(unsafe { crate::opi_fcitx5_symbol_browse(0) }, 0, "退出幂等");
    // action 1 进入 + 未知 action 空操作（返回当前态，不崩）
    assert_eq!(unsafe { crate::opi_fcitx5_symbol_browse(1) }, 1);
    assert_eq!(unsafe { crate::opi_fcitx5_symbol_browse(9) }, 1);
    assert_eq!(unsafe { crate::opi_fcitx5_symbol_browse(0) }, 0);
    // 读侧失败：路径缺失 / 空路径 → -1（入口保持禁用，旧数据不丢）
    let bad = "/nonexistent/symbol_categories.tsv";
    assert_eq!(
        unsafe { crate::opi_fcitx5_load_symbols(bad.as_ptr(), bad.len()) },
        -1
    );
    assert_eq!(
        unsafe { crate::opi_fcitx5_load_symbols(std::ptr::null(), 0) },
        -1
    );
    assert!(crate::with_state(|s| s.symbol_browse_available()).unwrap());
    // 读到但零合法类目 → -2（空文件）
    let dir = std::env::temp_dir().join(format!("opi-sym-cabi-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let empty = dir.join("empty.tsv");
    std::fs::write(&empty, "").unwrap();
    let p = empty.to_string_lossy().into_owned();
    assert_eq!(
        unsafe { crate::opi_fcitx5_load_symbols(p.as_ptr(), p.len()) },
        -2
    );
    assert!(
        !crate::with_state(|s| s.symbol_browse_available()).unwrap(),
        "零合法类目 → 入口禁用"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
