// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 符号浏览（「符号大全」）的引擎侧门禁：**读真资产**（Android 部署的那一份，
//! include_str! —— engine-core 禁 IO，测试是唯一能把它读进来的地方）钉数据契约，
//! 再用引擎公开 API 钉浏览态的读侧分派与真缓冲保全（lead 裁决 2026-10-10 的条件 ①③）。
//!
//! 宿主路由（数字下钻 / 翻页 / Esc / 未处理键先退出）在各自轨的
//! `input_method_browse_tests.rs` / `logic_browse_tests.rs` 里钉 —— 本文件只钉引擎语义。
//!
//! 三处数据副本（Android 资产、fcitx5 `data/`、生成脚本的输入）的漂移门禁：
//! `crates/fcitx5-opi/src/data_dir.rs` 的 `bundled_asset_matches_android`
//! （两副本逐字节相等），生成脚本自身的确定性由脚本头注释里的命令重跑钉住。

use engine_core::Engine;
use engine_core::composer::Mode;
use engine_core::dictionary::InMemoryDictionary;
use engine_core::symbol_browse::{BROWSE_TITLE, Selection};

/// Android 部署的真资产（权威副本）。
const ASSET: &str = include_str!("../../../android/app/src/main/assets/symbol_categories.tsv");

fn engine_with(words: &[(&str, &str, u32)]) -> Engine {
    let mut d = InMemoryDictionary::new();
    for (py, w, f) in words {
        d.insert(py, w, *f);
    }
    Engine::new(
        Box::new(d),
        engine_core::symbols::SymbolEngine::builtin(),
        true,
    )
}

fn browse_engine() -> Engine {
    let mut e = engine_with(&[("ni", "你", 100)]);
    assert_eq!(e.load_symbol_categories(ASSET), 0, "真资产不含坏行");
    e
}

// ---- 真资产的数据契约（与 Android SymbolCategoriesTest 的资产断言同源） ----

#[test]
fn real_asset_has_22_contiguous_categories() {
    let (cats, skipped) = engine_core::symbol_browse::parse_categories(ASSET);
    assert_eq!(skipped, 0, "入库资产不得有坏行");
    assert_eq!(
        cats.len(),
        22,
        "「符号大全」固定 22 类目（与 Android 页一致）"
    );
    for (i, c) in cats.iter().enumerate() {
        assert_eq!(c.index, (i + 1) as u32, "序号 1..=22 连续：第 {i} 行");
        assert!(!c.label.is_empty(), "类目名不得为空：第 {i} 行");
        assert!(!c.symbols.is_empty(), "类目 {} 不得为空", c.label);
    }
    assert_eq!(cats[0].label, "常用", "首类目与 Android 页一致");
    assert_eq!(cats[21].label, "藏文", "末类目与 Android 页一致");
}

#[test]
fn real_asset_first_category_first_symbol_is_full_width_comma() {
    // 钉住「第一条 = ，」这个被多处引用的事实（Android 页、人肉验收都拿它当锚）
    let (cats, _) = engine_core::symbol_browse::parse_categories(ASSET);
    assert_eq!(cats[0].symbols[0], "，");
}

// ---- 引擎读侧分派：buffer 劫持 / 候选 / 选中 ----

#[test]
fn browse_hijacks_buffer_and_candidates_but_keeps_real_buffer() {
    let mut e = browse_engine();
    e.input_key('n');
    e.input_key('i');
    assert_eq!(e.buffer(), "ni");
    assert!(e.enter_symbol_browse());
    // 条件 ①：浏览态 buffer() 返回标题（面板靠它显示），候选 = 一级 22 类目
    assert_eq!(e.buffer(), BROWSE_TITLE);
    let l1 = e.candidates(usize::MAX);
    assert_eq!(l1.len(), 22);
    assert_eq!(l1[0].text, "1.常用");
    // 条件 ①c：浏览期间不写 composer 真缓冲（标题不进缓冲；缓冲也不被清）
    e.exit_symbol_browse();
    assert_eq!(e.buffer(), "ni", "退出后真缓冲原样（浏览期间零改动）");
}

#[test]
fn select_in_browse_drills_then_commits_symbol() {
    let mut e = browse_engine();
    assert!(e.enter_symbol_browse());
    // 点/按第 1 个类目 → 下钻：无提交文本、不进二级以外的地方
    assert_eq!(e.select(0), "", "类目不可上屏（任何路径）");
    assert_eq!(e.buffer(), "符号·常用", "二级标题带类目名");
    let l2 = e.candidates(usize::MAX);
    assert_eq!(l2[0].text, "，");
    // 选一个符号 → 上屏其文本，并回一级（不是退出）
    assert_eq!(e.select(2), "、");
    assert_eq!(e.buffer(), BROWSE_TITLE, "提交后回一级");
    assert_eq!(e.candidates(usize::MAX)[0].text, "1.常用");
    // 越界（本层没有第 22 项）→ 空串，状态不变
    assert_eq!(e.select(99), "");
    assert_eq!(e.buffer(), BROWSE_TITLE);
}

#[test]
fn category_labels_are_uncommittable_through_every_select_path() {
    // 条件 ③ 的机制证明：一级任何 index 都只可能下钻（返回空串）——类目展示串
    // （"1.常用"）没有任何返回路径；只有进二级才出可上屏的符号文本。
    let mut e = browse_engine();
    for i in 0..=22 {
        assert!(e.enter_symbol_browse());
        assert_eq!(e.select(i), "", "一级 select({i}) 不得返回类目展示串");
        e.exit_symbol_browse();
    }
    // 二级出的是符号本体的文本（第一类目第一符号 = ，）
    assert!(e.enter_symbol_browse());
    assert_eq!(e.select(0), "");
    assert_eq!(e.select(0), "，");
}

#[test]
fn unhandled_key_leaves_browse_then_types_normally() {
    // 条件 ② 的引擎侧兜底：未处理键 = 先退出浏览（恢复真缓冲）再按正常流程处理。
    let mut e = browse_engine();
    e.input_key('n');
    e.input_key('i');
    assert!(e.enter_symbol_browse());
    assert_eq!(e.input_key('h'), "", "拼音过程无提交");
    assert!(!e.is_symbol_browsing(), "未处理键先退出浏览");
    assert_eq!(e.buffer(), "nih", "退出后接着打进真缓冲");
    e.backspace(); // 回到 ni
    assert_eq!(
        e.candidates(10)[0].text,
        "你",
        "候选来自真缓冲（浏览层已退场，不是类目表）"
    );
}

#[test]
fn space_and_backspace_leave_browse_first() {
    let mut e = browse_engine();
    e.input_key('n');
    e.input_key('i');
    assert!(e.enter_symbol_browse());
    // 空格：先退出浏览，再按拼音规则提交首候选（绝不提交标题）
    let out = e.input_space();
    assert_eq!(out, "你");
    assert_eq!(e.buffer(), "");
    // 退格：同样先退出浏览，再删真缓冲的最后一个码点
    e.input_key('h');
    e.input_key('a');
    assert!(e.enter_symbol_browse());
    e.backspace();
    assert!(!e.is_symbol_browsing());
    assert_eq!(e.buffer(), "h");
}

#[test]
fn clear_exits_browse() {
    let mut e = browse_engine();
    assert!(e.enter_symbol_browse());
    e.clear();
    assert!(!e.is_symbol_browsing());
    assert_eq!(e.buffer(), "");
}

#[test]
fn esc_semantics_exit_is_idempotent() {
    let mut e = browse_engine();
    assert!(e.enter_symbol_browse());
    e.exit_symbol_browse();
    e.exit_symbol_browse(); // 幂等
    assert!(!e.is_symbol_browsing());
    assert_eq!(e.buffer(), "");
}

// ---- 装载：坏行跳过不抛、空数据禁用入口、重复装载替换 ----

#[test]
fn load_counts_bad_lines_and_disables_entry_on_empty() {
    let mut e = engine_with(&[]);
    assert!(!e.symbol_browse_available());
    assert!(!e.enter_symbol_browse(), "无数据 → 入口禁用");
    assert_eq!(e.load_symbol_categories("1\t常用\t，\n坏行\n2\tx\t\n"), 2);
    assert!(e.symbol_browse_available());
    assert!(e.enter_symbol_browse());
    assert_eq!(e.candidates(usize::MAX).len(), 1, "只有 1 条合法类目");
    // 重新装载 = 替换数据集并复位浏览态（旧下标对新数据无意义）；再进从一级开始
    assert_eq!(e.select(0), "");
    assert_eq!(e.buffer(), "符号·常用");
    e.load_symbol_categories(ASSET);
    assert!(!e.is_symbol_browsing(), "替换数据 = 浏览态一并复位");
    assert!(e.enter_symbol_browse());
    assert_eq!(e.buffer(), BROWSE_TITLE, "新数据从一级开始");
    // 空串 → 清掉数据集，入口回到禁用
    assert_eq!(e.load_symbol_categories(""), 0);
    assert!(!e.symbol_browse_available());
    assert!(!e.enter_symbol_browse());
    assert!(!e.is_symbol_browsing());
}

#[test]
fn loaded_data_survives_typing_until_enter() {
    // 回归（曾红）：装载 ≠ 正在浏览。若两者并成「一个 Option」，装载完的第一次
    // input_key 会走「未处理键先退出浏览」把数据集当浏览态清掉 —— 表现为装好数据
    // 后打第一个键、入口就永久禁用。exit 同理：丢的是浏览态，不是类目表。
    let mut e = browse_engine();
    e.input_key('n');
    e.input_key('i');
    assert!(e.symbol_browse_available(), "打字不丢数据集");
    assert!(!e.is_symbol_browsing(), "没进过浏览就不是浏览态");
    assert!(e.enter_symbol_browse());
    e.exit_symbol_browse();
    assert!(e.symbol_browse_available(), "退出不丢数据集");
    assert!(e.enter_symbol_browse(), "退出后凭原数据可再进，不必重装载");
}

#[test]
fn browse_select_does_not_disturb_mode_or_future_typing() {
    // 叠加态不碰模式：浏览期间模式原样，退出后接着打不换模式
    let mut e = browse_engine();
    e.switch_mode(Mode::Pinyin);
    assert!(e.enter_symbol_browse());
    e.select(0); // 下钻
    e.select(0); // 提交，
    assert_eq!(e.mode(), Mode::Pinyin);
    e.exit_symbol_browse();
    assert_eq!(e.mode(), Mode::Pinyin);
    e.input_key('n');
    e.input_key('i');
    assert_eq!(e.candidates(10)[0].text, "你");
}

#[test]
fn symbol_engine_query_outlets_still_work_after_move() {
    // 机械搬移校验：三个转发出口在 engine_symbol_browse.rs 里保持原语义（非空即可）
    let e = engine_with(&[]);
    let blocks = e.symbol_blocks();
    assert!(!blocks.is_empty(), "内置符号表有常用块");
    assert!(
        !e.search_symbols("dun").is_empty(),
        "dun → 、（符号索引仍通）"
    );
    assert!(
        !e.symbols_in_block(blocks[0].id).is_empty(),
        "块内符号非空（BlockId 路径仍通）"
    );
}

#[test]
fn browse_state_machine_direct_api() {
    // 状态机单测（对象级）：Selection 三态 —— 宿主不直接用，但语义是契约的一部分
    use engine_core::symbol_browse::SymbolBrowse;
    let (cats, _) = engine_core::symbol_browse::parse_categories(ASSET);
    let mut b = SymbolBrowse::new(cats);
    assert_eq!(b.select(0), Selection::Miss, "未 enter = 不响应选择");
    assert!(b.enter());
    assert_eq!(b.level2(), None);
    assert_eq!(b.select(0), Selection::Drill);
    assert_eq!(b.level2(), Some(0));
    assert!(matches!(b.select(0), Selection::Commit(_)));
    assert_eq!(b.level2(), None);
    assert_eq!(b.select(22), Selection::Miss);
}
