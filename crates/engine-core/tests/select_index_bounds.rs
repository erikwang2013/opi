// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **选词下标的边界**：两个入口（页内 / 全局）各自的越界判据，以及它们与候选表的口径。
//!
//! 从 `router_invariants.rs` 拆出来（本仓 500 行门禁，与 `punctuation_switches.rs` 从
//! `punctuation.rs` 拆出同一条），两个用例原本就与那门的 `digit_*` 用例并列：
//!
//! - `select_beyond_page_must_not_commit_hidden_candidate` —— **页内**入口
//!   （`KeyRouter::select`，C ABI `opi_select_page` → `Api::select_page` 一行转发）；
//! - `engine_select_index_space_agrees_with_unlimited_list` —— **全局**入口
//!   （`Engine::select`，JNI / `opi_select` 走它）与 `candidates(FETCH_LIMIT)` 必须同表。
//!
//! 两条都是**回归用例**：越界判据曾在两个入口上分叉（一个按页内、一个按全量），
//! 而同一个 `select_from` 又是数字选词、回车提交、点击候选三条路的公共下游 ——
//! 判据只此一份，才有「一个入口改对、三个入口同时改对」。
//!
//! 辅助函数与 `router_invariants.rs` 同名同义（集成测试是各自独立的 crate，没有共享
//! 模块目录，故这里重抄三个小助手；改语义时**两处一起改**）。

use engine_core::Engine;
use engine_core::composer::Mode;
use engine_core::dictionary::InMemoryDictionary;
use engine_core::keys::KEY_PAGE_DOWN;
use engine_core::router::{FETCH_LIMIT, KeyRouter, PAGE_SIZE};
use engine_core::symbols::SymbolEngine;

/// n 条 "hao" 词条（词频严格递减 → 顺序确定），符号表用空表。
fn router_with(n: usize) -> KeyRouter {
    let mut d = InMemoryDictionary::new();
    for i in 0..n {
        d.insert("hao", &format!("词{i:03}"), (n - i) as u32);
    }
    let mut r = KeyRouter::new(Engine::new(
        Box::new(d),
        SymbolEngine::new(Vec::new(), Vec::new()),
        true,
    ));
    r.switch_mode(Mode::Pinyin);
    r
}

fn type_hao(r: &mut KeyRouter) {
    for c in ['h', 'a', 'o'] {
        r.key_event(c as u32, 0);
    }
}

/// 可达候选 = `engine.candidates(FETCH_LIMIT)`（分页的唯一数据源）。
fn reachable(r: &KeyRouter) -> Vec<String> {
    r.engine()
        .candidates(FETCH_LIMIT)
        .into_iter()
        .map(|c| c.text)
        .collect()
}

/// 回归（本轮缺陷）：**页内**下标入口（`KeyRouter::select`）的越界判据曾是**全量**
/// 候选列表 —— 只按 `.get(page * PAGE_SIZE + index)` 落空才给空串，于是页内越界
/// （第 0 页只有 8 项时点第 9 项，k=8）被换算成**次页**下标，提交了用户屏幕上看不见的候选。
/// 审计实测（真库 luna.opid）：`nh` 第 0 页 `select(8)` 提交「年后」（第 1 页首条）。
///
/// 入口与 `router_invariants.rs::digit_beyond_page_must_not_commit_hidden_candidate`
/// 不同、不变式是同一条：数字键走 `digit_select`，**点击候选**走本函数 —— C ABI
/// `opi_select_page` → `Api::select_page`（一行转发 `self.router.select(index)`）→
/// `KeyRouter::select`。修法在两者共用的 `select_from` 里（判据提到页内），两个入口因此
/// 自动一致；本用例钉的是第二个入口，此前只有第一个入口有门禁。
#[test]
fn select_beyond_page_must_not_commit_hidden_candidate() {
    // 页内正常项不受影响（先钉住不误伤）
    let mut r = router_with(20);
    type_hao(&mut r);
    assert_eq!(r.select(0), "词000", "第 0 页首位");
    let mut r = router_with(20);
    type_hao(&mut r);
    assert_eq!(
        r.select(PAGE_SIZE - 1),
        "词007",
        "第 0 页末位（页内最大合法下标）"
    );

    // 页内越界 k=8：全量列表里第 8 项**存在**（就是旧实现提交的那一条）
    let mut r = router_with(20);
    type_hao(&mut r);
    assert_eq!(r.page(), 0);
    assert_eq!(r.candidates().len(), PAGE_SIZE, "前置：第 0 页是满页 8 条");
    let full = reachable(&r);
    assert_eq!(
        full[PAGE_SIZE], "词008",
        "前置：全量第 8 项确实存在（页内越界与全量越界因此必须分开判）"
    );
    assert_eq!(
        r.select(PAGE_SIZE),
        "",
        "页内没有第 9 项 → 空串；实际提交了 {:?}（次页首条，用户看不见）",
        full[PAGE_SIZE]
    );
    assert_eq!(r.buffer(), "hao", "越界不得提交 ⇒ 缓冲不变");
    assert_eq!(r.page(), 0, "越界不得改页码");

    // 第 1 页同样：k=8 → 全局 16（第 2 页首条）也存在，必须空串
    r.key_event(KEY_PAGE_DOWN, 0);
    assert_eq!(r.page(), 1);
    assert_eq!(r.select(PAGE_SIZE), "", "第 1 页的页内越界同样必须空串");
    assert_eq!(r.buffer(), "hao");
    // 而第 1 页的页内合法项照旧
    assert_eq!(r.select(0), "词008", "第 1 页首位 = 全量第 8 项");
    let mut r = router_with(20);
    type_hao(&mut r);
    r.key_event(KEY_PAGE_DOWN, 0);
    assert_eq!(r.select(PAGE_SIZE - 1), "词015", "第 1 页末位");

    // 末页只有 4 项（20 = 8+8+4）：k=3 合法、k=4 页内越界（此处全量也越界）
    let mut r = router_with(20);
    type_hao(&mut r);
    r.key_event(KEY_PAGE_DOWN, 0);
    r.key_event(KEY_PAGE_DOWN, 0);
    assert_eq!(r.page(), 2);
    assert_eq!(r.candidates().len(), 4, "前置：末页不满");
    assert_eq!(r.select(3), "词019", "末页最后一项");
    let mut r = router_with(20);
    type_hao(&mut r);
    r.key_event(KEY_PAGE_DOWN, 0);
    r.key_event(KEY_PAGE_DOWN, 0);
    assert_eq!(r.select(4), "", "末页第 5 项（页内越界）");
    assert_eq!(r.buffer(), "hao");
}

/// `Engine::select(index)`（全局下标：JNI / `opi_select` 走它）与
/// `engine.candidates(usize::MAX)`（分页的唯一数据源 [`reachable`]）**必须是同一份候选表**。
///
/// 缺陷：`Engine::select` 内部用 `candidates(512)` 抓表 ⇒ 产生一条**静默失败的下标带**。
/// 审计实测（真库，'y'）：`candidates(usize::MAX)` 8017 项、`candidates(512)` 512 项且
/// 为前者前缀，下标 0/7/511 两路同候选，而下标 **512/513/600/8000/8016 全部分歧**：
/// `full.get(i) = Some` 而 `Engine::select(i) = ""` —— 不提交、不学习，调用方无从分辨
/// 「没有这一项」与「我内部只取了 512 项」（**契约级失败，不是提交错候选**）。
///
/// 修法选了「512 与 `FETCH_LIMIT` 统一」：512 不省任何成本（`rank_and_pick` 无论 limit
/// 多大都全量收集 + 排序，512 只是扔掉已算好的尾巴），却造出一个下标带，
/// 且 `Engine::select` 的返回值只有一个字符串 —— 没有地方让调用方识别这个失败。
#[test]
fn engine_select_index_space_agrees_with_unlimited_list() {
    let n = 600;
    let mut probe = router_with(n);
    type_hao(&mut probe);
    let full = reachable(&probe);
    assert!(
        full.len() > 512,
        "前置：候选必须多过旧的 512 截断才有分歧带（实得 {}）",
        full.len()
    );
    assert_eq!(full.len(), n, "前置：全量抓取拿到全部 {n} 条");
    for i in [0usize, 7, 511, 512, 513, 599] {
        let mut r = router_with(n);
        type_hao(&mut r);
        assert_eq!(
            r.engine_mut().select(i),
            full[i],
            "下标 {i}：Engine::select 与全量候选表分歧（512 截断的实现在 512.. 返回空串）"
        );
    }
    // 真·越界仍必须是空串（下标空间放开不等于无条件接受）
    let mut r = router_with(n);
    type_hao(&mut r);
    assert_eq!(r.engine_mut().select(n), "", "第 {n} 项不存在 → 空串");
    assert_eq!(r.engine_mut().select(usize::MAX), "", "极大下标 → 空串");
}
