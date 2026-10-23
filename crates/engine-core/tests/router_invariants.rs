// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 候选分页与会话状态机的不变量（`KeyRouter`，Apple 两平台的键路由）。
//!
//! 既有 `router_tests.rs` 是**手写的逐条语义用例**（20 条候选 → 3 页），覆盖不到：
//! - 候选数**恰为 PAGE_SIZE 整数倍**（8/16/64）：末页是不是满页、翻页会不会停在空页；
//! - 0 条候选时翻页/选词；
//! - 页码与缓冲的强一致：**任何改缓冲的路径都必须把页码归零**
//!   （`reset_page_if_buffer_changed` 只在 buffer 与快照不同时才归零 —— 谁的快照没刷新，谁就漏归零）；
//! - 「页内下标 → 全局下标」的换算在**每一页每一项**上都对齐（差一位即选错词）。
//!
//! 断言都要求改坏实现就红（见 memory: green-tests-only-prove-what-is-asserted）。
//! 分页与两轨同构（`PAGE_SIZE`/`FETCH_LIMIT`），本文件不另立语义。

use engine_core::Engine;
use engine_core::composer::Mode;
use engine_core::dictionary::InMemoryDictionary;
use engine_core::keys::{
    KEY_BACK_SPACE, KEY_DELETE, KEY_PAGE_DOWN, KEY_PAGE_UP, KEY_RETURN, KEY_SHIFT, KEY_SPACE,
};
use engine_core::router::{FETCH_LIMIT, KeyAction, KeyRouter, PAGE_SIZE, ShiftState};
use engine_core::symbols::SymbolEngine;
use proptest::prelude::*;

/// n 条 "qwx" 词条（词频严格递减 → 顺序确定），符号表用空表。
fn router_with(n: usize) -> KeyRouter {
    let mut d = InMemoryDictionary::new();
    for i in 0..n {
        d.insert("qwx", &format!("词{i:03}"), (n - i) as u32);
    }
    let mut r = KeyRouter::new(Engine::new(
        Box::new(d),
        SymbolEngine::new(Vec::new(), Vec::new()),
        true,
    ));
    r.switch_mode(Mode::Pinyin);
    r
}

fn type_key(r: &mut KeyRouter) {
    for c in ['q', 'w', 'x'] {
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

/// 从首页起逐页取候选（翻到底为止）。
fn all_pages(r: &mut KeyRouter) -> Vec<Vec<String>> {
    let mut pages = Vec::new();
    loop {
        pages.push(r.candidates());
        let before = r.page();
        r.key_event(KEY_PAGE_DOWN, 0);
        if r.page() == before {
            break;
        }
    }
    pages
}

#[test]
fn page_count_is_ceiling_and_last_page_matches_remainder() {
    for n in [0usize, 1, 7, 8, 9, 15, 16, 17, 23, 24, 63, 64, 65, 100, 200] {
        let mut r = router_with(n);
        type_key(&mut r);
        let want = reachable(&r);
        // 曾经写作 `n.min(FETCH_LIMIT)` —— FETCH_LIMIT 不再截断后 clippy 判定该 min
        // 是 no-op（`-D warnings` 下必须去掉），去掉正好把断言**收紧**成「一条不漏」。
        assert_eq!(want.len(), n, "n={n}: 可达候选数必须是全部 n 条");
        assert_eq!(
            r.page_count(),
            want.len().div_ceil(PAGE_SIZE),
            "n={n}: 总页数"
        );

        let pages = all_pages(&mut r);
        assert_eq!(pages.len(), r.page_count().max(1), "n={n}: 实际页数");
        for (p, page) in pages.iter().enumerate() {
            if want.is_empty() {
                assert!(page.is_empty(), "n={n}: 无候选时第 {p} 页竟有条目");
                continue;
            }
            assert!(!page.is_empty(), "n={n}: 第 {p} 页为空（翻页停在了空页上）");
            assert!(page.len() <= PAGE_SIZE, "n={n}: 第 {p} 页超长");
        }
        // 末页条数：整数倍时必须**满页**（差一位的表现是「末页只剩一个」，翻页像失灵）
        if let Some(last) = pages.last()
            && !want.is_empty()
        {
            let remain = want.len() % PAGE_SIZE;
            let expect = if remain == 0 { PAGE_SIZE } else { remain };
            assert_eq!(last.len(), expect, "n={n}: 末页条数");
        }
    }
}

/// 逐页取候选，拼起来必须**恰好等于**可达列表（不重不漏不乱序）。
#[test]
fn pages_partition_reachable_candidates_in_order() {
    for n in [0usize, 1, 8, 9, 20, 64] {
        let mut r = router_with(n);
        type_key(&mut r);
        let want = reachable(&r);
        let got: Vec<String> = all_pages(&mut r).into_iter().flatten().collect();
        assert_eq!(got, want, "n={n}: 分页与全量列表不一致");
    }
}

/// 天花板回归（2026-09-28）：**可达集合 = 引擎排出的全部候选**，不是某个固定条数。
///
/// ⚠️ 期望值**故意不引用 `FETCH_LIMIT`**。上面用例的 `want` 取自同一个常量 ——
/// 常量被调小时 `want` 与 `page_count` 一起缩，截断对它们完全不可见
/// （`FETCH_LIMIT = 64` 时全部用例仍绿，包括 n=100）。这里的 300 是**独立**期望值：
/// 越过任何「64 时代」的上限，截断一回来即红。
#[test]
fn paging_reaches_every_candidate_the_engine_ranks() {
    const N: usize = 300; // 独立期望值，不随 FETCH_LIMIT 缩放
    let mut r = router_with(N);
    type_key(&mut r);
    assert_eq!(
        r.page_count(),
        N.div_ceil(PAGE_SIZE),
        "总页数必须覆盖全部候选"
    );
    let got: Vec<String> = all_pages(&mut r).into_iter().flatten().collect();
    assert_eq!(got.len(), N, "翻到底必须能取到全部 {N} 条");
    assert_eq!(
        got,
        reachable(&r),
        "分页结果 = 引擎的候选列表（不重不漏不乱序）"
    );
}

#[test]
fn page_down_clamps_and_page_up_floors_at_zero() {
    let mut r = router_with(20);
    type_key(&mut r);
    for _ in 0..50 {
        r.key_event(KEY_PAGE_DOWN, 0);
    }
    assert_eq!(r.page(), 2, "翻到底应钳制在末页（20 条 = 3 页）");
    assert_eq!(r.page(), r.page_count() - 1);
    assert!(!r.candidates().is_empty(), "末页（余 4 条）不得为空");
    for _ in 0..50 {
        r.key_event(KEY_PAGE_UP, 0);
    }
    assert_eq!(r.page(), 0, "翻到顶应钳制在首页");
    assert!(!r.candidates().is_empty(), "首页不得为空");
}

#[test]
fn empty_candidate_set_survives_paging_and_digits() {
    let mut r = router_with(0);
    type_key(&mut r); // "qwx" 无词条 → 0 候选
    assert_eq!(r.page_count(), 0);
    assert!(r.candidates().is_empty());
    assert_eq!(r.key_event(KEY_PAGE_DOWN, 0), KeyAction::EngineHandled);
    assert_eq!(r.page(), 0, "无候选时页码必须为 0");
    assert_eq!(r.key_event(KEY_PAGE_UP, 0), KeyAction::EngineHandled);
    assert_eq!(r.page(), 0);
    assert_eq!(r.candidates().len(), 0);
    // 选词越界：不得消费、不得动缓冲
    assert_eq!(r.key_event('1' as u32, 0), KeyAction::PassThrough);
    assert_eq!(r.buffer(), "qwx");
}

/// 页内下标 → 全局下标的换算，在**每一页每一项**上都要对齐。
/// 差一位的表现是「按 3 选到第 2 个」——总数对、顺序错，最不容易被发现。
#[test]
fn digit_select_matches_page_listing_every_index() {
    let mut r = router_with(20);
    type_key(&mut r);
    let pages = all_pages(&mut r);
    assert_eq!(pages.len(), 3);
    for (p, page) in pages.iter().enumerate() {
        for (i, want) in page.iter().enumerate() {
            let mut r = router_with(20);
            type_key(&mut r);
            for _ in 0..p {
                r.key_event(KEY_PAGE_DOWN, 0);
            }
            assert_eq!(r.candidates(), *page, "第 {p} 页内容不一致");
            let digit = char::from_digit((i + 1) as u32, 10).expect("页内下标 1..=8");
            assert_eq!(
                r.key_event(digit as u32, 0),
                KeyAction::Input(want.clone()),
                "第 {p} 页第 {i} 项（键 {digit}）选错"
            );
            assert_eq!(r.buffer(), "", "选中后必须清缓冲");
            assert_eq!(r.page(), 0, "选中后页码必须归零");
        }
        // 页内越界（该项不存在）→ 直通，且不得改缓冲/页码
        let mut r = router_with(20);
        type_key(&mut r);
        for _ in 0..p {
            r.key_event(KEY_PAGE_DOWN, 0);
        }
        if page.len() < PAGE_SIZE {
            let digit = char::from_digit((page.len() + 1) as u32, 10).unwrap();
            assert_eq!(r.key_event(digit as u32, 0), KeyAction::PassThrough);
            assert_eq!(r.buffer(), "qwx");
            assert_eq!(r.page(), p);
        }
    }
}

/// 回车提交**当前页**首位（`handle_enter` → `select(0)`），翻页后不得仍提交首页那个词。
#[test]
fn enter_commits_current_page_first_candidate() {
    let mut r = router_with(20);
    type_key(&mut r);
    assert_eq!(
        r.key_event(KEY_RETURN, 0),
        KeyAction::Input("词000".into()),
        "首页回车 = 首页首位"
    );
    let mut r = router_with(20);
    type_key(&mut r);
    r.key_event(KEY_PAGE_DOWN, 0);
    assert_eq!(
        r.key_event(KEY_RETURN, 0),
        KeyAction::Input("词008".into()),
        "第二页回车 = 第二页首位（页内换算漏了 page 就会提交首页那个）"
    );
    assert_eq!(r.page(), 0, "提交后页码归零");
}

/// 空格提交**当前页**首位 —— 用户报障「桌面端翻页后直接确认始终是第一页的 1」。
/// 空格收尾曾无条件走 `Engine::input_space()`（全局第 0 条），页码被静默丢掉。
#[test]
fn space_commits_current_page_first_candidate() {
    let mut r = router_with(20);
    type_key(&mut r);
    assert_eq!(
        r.key_event(KEY_SPACE, 0),
        KeyAction::Input("词000".into()),
        "首页空格 = 首页首位"
    );
    for (pages_down, want) in [(1usize, "词008"), (2, "词016")] {
        let mut r = router_with(20);
        type_key(&mut r);
        for _ in 0..pages_down {
            r.key_event(KEY_PAGE_DOWN, 0);
        }
        assert_eq!(
            r.key_event(KEY_SPACE, 0),
            KeyAction::Input(want.into()),
            "第 {} 页空格 = 当前页首位（漏 page 就会提交首页那个）",
            pages_down + 1
        );
        assert_eq!(r.page(), 0, "提交后页码归零");
    }
    // 标点收尾同一条路（`input_punct` → 空格收尾）：翻页后打逗号也要提当前页首位。
    let mut r = router_with(20);
    type_key(&mut r);
    r.key_event(KEY_PAGE_DOWN, 0);
    assert_eq!(
        r.key_event(',' as u32, 0),
        KeyAction::Input("词008，".into()),
        "第 2 页打 ',' = 当前页首位 + 标点"
    );
}

/// 回归（原缺陷）：数字选词的「越界」判据曾是**全量候选列表**，不是**当前页**。
///
/// 最小复现：输入 `qwx`（20 条候选 → 第 0 页显示 `词000..词007`），按 `9`。
/// 期望：页内没有第 9 项 → 直通，交客户端输入数字 `9`（与末页的既有行为一致，
/// 见 `digit_out_of_range_passes_through`：那里 '9' 在末页确实是直通的）。
/// 实际：`digit_select` 只看 `select(8)` 的返回值空不空，而 `select` 的「越界」
/// 是**全量列表**的越界（`global = page * PAGE_SIZE + index` 落到次页），
/// 于是提交了 `词008` —— 用户屏幕上根本没有这一条。
///
/// 影响面：非末页按 `9`（页内只有 8 项时唯一的越界数字）会静默上屏一个看不见的词；
/// 分页越靠前越容易撞上。同一形态在三端逐行同构（行号会随并发改动漂移，按符号找）：
/// `crates/engine-core/src/router.rs`、`crates/fcitx5-opi/src/input_method.rs`、
/// `crates/tsf-opi/src/logic_input_method.rs` 三处各自的 `fn digit_select`。
/// 修法：`digit_select` 在调用 `select` 之前先按**本页**候选数判越界
/// （`select` 自身的判据仍是全量的，见该函数的文档）。
#[test]
fn digit_beyond_page_must_not_commit_hidden_candidate() {
    let mut r = router_with(20);
    type_key(&mut r);
    assert_eq!(r.page(), 0);
    assert_eq!(r.page_count(), 3);
    assert_eq!(r.candidates().len(), PAGE_SIZE, "前置：第 0 页是满页 8 条");
    assert_eq!(
        r.key_event('9' as u32, 0),
        KeyAction::PassThrough,
        "页内没有第 9 项，'9' 必须直通；实际提交了 {:?}（次页首条）",
        KeyAction::Input("词008".into())
    );
    assert_eq!(r.buffer(), "qwx", "'9' 直通时不得动缓冲");
}

// 第二个选词入口（`KeyRouter::select`，C ABI `opi_select_page`）与 `Engine::select` 的
// 下标边界用例在这一批拆分中移到了 tests/select_index_bounds.rs（本文件撞 500 行门禁）——
// 与上面的 `digit_beyond_page_must_not_commit_hidden_candidate` 是同一条不变式。

// ---------- 页码与缓冲的强一致 ----------

/// 任何**改变缓冲**的按键路径都必须把页码归零。判据取自「缓冲是否变了」这个外部
/// 可观测量，不看实现里的快照 —— 快照漏刷新的实现会在这里露馅。
#[test]
fn page_tracks_buffer_changes() {
    let cases: Vec<(&str, Vec<u32>)> = vec![
        ("字母入缓冲", vec!['n' as u32]),
        ("退格", vec![KEY_BACK_SPACE]),
        ("Delete", vec![KEY_DELETE]),
        ("空格提交", vec![0x20]),
        ("回车提交", vec![KEY_RETURN]),
        ("数字选词", vec!['1' as u32]),
        ("翻页（不改缓冲，页码应保持）", vec![KEY_PAGE_DOWN]),
    ];
    for (what, keys) in cases {
        let mut r = router_with(20);
        type_key(&mut r);
        r.key_event(KEY_PAGE_DOWN, 0);
        assert_eq!(r.page(), 1, "{what}: 前置翻页失败");
        let before = r.buffer();
        for k in &keys {
            r.key_event(*k, 0);
        }
        if r.buffer() != before {
            assert_eq!(
                r.page(),
                0,
                "{what}: 缓冲已变（{before:?} → {:?}）页码未归零",
                r.buffer()
            );
        }
        assert!(r.page() < r.page_count().max(1), "{what}: 页码越界");
    }

    // 切模式清缓冲 ⇒ 页码归零
    let mut r = router_with(20);
    type_key(&mut r);
    r.key_event(KEY_PAGE_DOWN, 0);
    assert_eq!(r.page(), 1);
    r.switch_mode(Mode::English);
    assert_eq!(r.buffer(), "");
    assert_eq!(r.page(), 0, "切模式清了缓冲，页码必须归零");
}

/// ⇧ 不改缓冲，也不改候选集 —— 页码既不得越界，也不得凭空跳页。
#[test]
fn shift_keeps_page_in_range() {
    let mut r = router_with(20);
    type_key(&mut r);
    r.key_event(KEY_PAGE_DOWN, 0);
    assert_eq!(r.page(), 1);
    r.shift_tap();
    assert_eq!(r.shift_state(), ShiftState::Single);
    assert_eq!(r.page(), 1, "⇧ 不改变候选集时页码不动");
    assert!(r.page() < r.page_count());
}

/// 随机按键序列下的分页不变量（组合爆炸的死角靠这个扫）：
/// 页码恒在 `[0, page_count)`、页内恒 ≤ PAGE_SIZE、缓冲变则页码必归零、
/// 每次提交的文本必须**来自按键前那一页**或显然是直传结果。
#[test]
fn paging_invariants_under_random_keys() {
    let key_pool = vec![
        'h' as u32,
        'a' as u32,
        'o' as u32,
        'n' as u32,
        '1' as u32,
        '8' as u32,
        '9' as u32,
        0x20, // 空格
        KEY_BACK_SPACE,
        KEY_DELETE,
        KEY_RETURN,
        KEY_PAGE_UP,
        KEY_PAGE_DOWN,
        KEY_SHIFT,
    ];
    let keys = prop::sample::select(key_pool);
    proptest!(|(keys in prop::collection::vec(keys, 0..30))| {
        let mut r = router_with(20);
        for k in &keys {
            let buffer_before = r.buffer();
            let pending_before = r.pending_text();
            let page_before = r.page();
            let cands_before = r.candidates();
            let global_first = reachable(&r).first().cloned();
            let action = r.key_event(*k, 0);
            prop_assert!(r.page() < r.page_count().max(1), "页码越界: {}", r.page());
            prop_assert!(r.candidates().len() <= PAGE_SIZE);
            if r.buffer() != buffer_before {
                prop_assert_eq!(r.page(), 0, "缓冲变了页码没归零（键 {:#x}）", k);
            }
            match action {
                KeyAction::Input(t) => {
                    prop_assert!(!t.is_empty(), "提交了空串");
                    // 空格提交的是**全量列表首位**（engine.input_space 的语义，
                    // 与两轨一致），其余提交必须来自按键前那一页
                    let hit = t == " "
                        || Some(&t) == global_first.as_ref()
                        || cands_before.contains(&t)
                        || (cands_before.is_empty() && t == buffer_before)
                        // 逐字态空格收尾（2026-10-17）：片段 + 原文的复合提交
                        // 逐字态（2026-10-17）：片段与某个候选的**复合提交** ——
                        // 选词（片段+该候选）或空格收尾（片段+原文/首候选）。
                        || (!pending_before.is_empty()
                            && (t == format!("{pending_before}{buffer_before}")
                                // 选词：片段 + 某候选；空格：片段 + **全量首位**
                                || cands_before
                                    .iter()
                                    .any(|c| t == format!("{pending_before}{c}"))
                                || global_first
                                    .as_ref()
                                    .is_some_and(|g| t == format!("{pending_before}{g}"))));
                    // 这里曾有一条「数字越页」豁免，随 `digit_beyond_page_must_not_commit_hidden_candidate`
                    // 一起修掉了：页内越界的数字现在直通，走不到本分支。
                    prop_assert!(hit,
                        "提交的 {:?} 不在按键前那一页 {:?}（首页 {:?} 页码 {}/{}）",
                        t, cands_before, global_first, page_before, r.page_count());
                }
                KeyAction::PassThrough | KeyAction::EngineHandled => {}
            }
        }
    });
}
