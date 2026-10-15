// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **抓取次数**的计数夹具（2026-10-10 从 `router_invariants.rs` 拆出 —— 加 9 键回退
//! 记账后那边越 500 行门禁；与 `scheme_readings.rs` 从 `scheme_keys.rs` 拆出同一套先例）。
//!
//! 两条腿分开数，判据同一条：**一次动作只抓一次候选表**。
//! - **全键盘**：候选由 `query`（经 `query_all`）给出 —— 数字选词不得把整表排两遍；
//! - **9/14 键**：候选由 `visit_entries` 全量扫描给出（按键类展开，**零 query**）——
//!   命中态 1 扫、回退态 3 扫（主扫 + 判定扫 + 前缀重扫），点击只多一次收尾判定
//!   （同一个纯函数的重算，见 engine_segment.rs）。

use engine_core::Engine;
use engine_core::composer::Mode;
use engine_core::dictionary::InMemoryDictionary;
use engine_core::router::{KeyAction, KeyRouter};
use engine_core::symbols::SymbolEngine;

/// 可计数的词库包装。两条腿分开记：`n` = `query` 次数（`query_all` 走 trait 默认实现
/// → 落到 `query`；全键盘路径每次恰好 1 次，走分词回退时会多记，本用例的 "hao" 不触发）、
/// `v` = `visit_entries` 次数（按键类展开，9/14 键 —— 必须**转发**：trait 默认是空实现，
/// 不转发则 9 键候选在夹具下恒空。这条腿零 query，记账见下一条用例）。
struct Counting {
    inner: InMemoryDictionary,
    n: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    v: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl engine_core::dictionary::Dictionary for Counting {
    fn query(&self, pinyin: &str, limit: usize) -> Vec<engine_core::trie::Entry> {
        self.n.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.inner.query(pinyin, limit)
    }
    fn visit_entries(&self, f: &mut dyn FnMut(&str, &str, u32)) {
        self.v.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.inner.visit_entries(f)
    }
    fn len(&self) -> usize {
        self.inner.len()
    }
    fn max_freq(&self) -> u64 {
        self.inner.max_freq()
    }
}

/// 数字选词旧路径抓两次候选表：`candidates()` 一次，`select()` 内 `engine.select`
/// 又一次 —— 每次选词白付一次全量排序。抓取次数是计数式指标，比时间稳
/// （同一台机器上还有别的编译在跑）。改回两次即红。
///
/// 口径说明（2026-10-10 扩 9 键回退后复核）：本条数的是 `query`，走的是全键盘路径；
/// 9/14 键的按键类展开走 `visit_entries`（零 `query`）且 Nine 下数字是输入键、
/// 不走 `digit_select`（router_digit.rs）—— 新回退腿**不改变本条的计数口径**，
/// 它在另一条腿上的账见下一条用例。
#[test]
fn digit_select_fetches_candidate_table_once() {
    use std::sync::atomic::Ordering;
    let n = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let v = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)); // 全键盘不碰 visit_entries
    let mut d = InMemoryDictionary::new();
    for i in 0..20 {
        d.insert("hao", &format!("词{i:03}"), (20 - i) as u32);
    }
    let mut r = KeyRouter::new(Engine::new(
        Box::new(Counting {
            inner: d,
            n: n.clone(),
            v,
        }),
        SymbolEngine::new(Vec::new(), Vec::new()),
        true,
    ));
    r.switch_mode(Mode::Pinyin);
    for c in "hao".chars() {
        r.key_event(c as u32, 0);
    }
    // 基准：读一次当前页候选表要查几次词库（= 一次全量抓取）
    n.store(0, Ordering::Relaxed);
    let _ = r.candidates();
    let one_fetch = n.load(Ordering::Relaxed);
    assert!(one_fetch > 0, "基准抓取没查到词库，用例失效");
    n.store(0, Ordering::Relaxed);
    assert_eq!(
        r.key_event('1' as u32, 0),
        KeyAction::Input("词000".to_string())
    );
    assert_eq!(
        n.load(Ordering::Relaxed),
        one_fetch,
        "数字选词应只抓一次候选表（两次 = 整表排两遍）"
    );
}

/// 9 键回退腿的记账（2026-10-10 扩 9 键；与上一条同一件事的另一条腿）：本夹具走
/// `Dictionary` 的**默认**按键类实现（全表 `visit_entries`，生产实现是区间枚举，
/// 见 loader）—— 主档**零 query**；命中态 1 扫。回退态（2026-10-11 改）：判定拆成
/// 「存在性 + 见证者」两步，故一次抓取 = 主扫(1) + 存在性(1) + 见证者 l 探测
/// （本夹具的第一个 l 即中，1）+ 前缀重扫(1) = **4 扫**；点击 = 一次抓取 + 一次
/// 收尾判定（存在性 + 见证者 = 2 扫），判定是同一个纯函数的重算
/// （engine_segment.rs）—— 不多抓这条不变。
/// ⚠️ **2026-10-15 改判（宽松兜底档）**：`q` 从 **0 变 1** —— 尾巴 `9` 的首字母 `w`
/// 走一次前缀查询（`query_prefix_top` 默认实现经 `query_all` 落回 `query`）。统计口径
/// 「零 query」只对**严格档**成立；本档每次抓取固定 ≤ 1 次（前段 1，后段同字母复用，
/// 见 `candidates_scheme::tail_guess_hits`）。
#[test]
fn nine_key_fallback_accounting_is_bounded_scans_and_zero_queries() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let q = std::sync::Arc::new(AtomicUsize::new(0));
    let v = std::sync::Arc::new(AtomicUsize::new(0));
    let mut d = InMemoryDictionary::new();
    d.insert("nihao", "你好", 9000);
    d.insert("nihao", "你好啊", 8000);
    d.insert("ni", "你", 7000);
    let mut r = KeyRouter::new(Engine::new(
        Box::new(Counting {
            inner: d,
            n: q.clone(),
            v: v.clone(),
        }),
        SymbolEngine::new(Vec::new(), Vec::new()),
        true,
    ));
    r.engine_mut().set_scheme(engine_core::Scheme::Nine);
    for c in "64426".chars() {
        r.key_event(c as u32, 0);
    }
    v.store(0, Ordering::Relaxed);
    q.store(0, Ordering::Relaxed);
    assert_eq!(r.candidates().first().map(String::as_str), Some("你好"));
    assert_eq!(
        (v.load(Ordering::Relaxed), q.load(Ordering::Relaxed)),
        (1, 0),
        "命中态：主扫一趟、零 query"
    );
    r.key_event('9' as u32, 0); // 打超到下一音节首键 ⇒ 回退态
    v.store(0, Ordering::Relaxed);
    let _ = r.candidates();
    let one_fetch = v.load(Ordering::Relaxed);
    assert_eq!(
        (one_fetch, q.load(Ordering::Relaxed)),
        (4, 1),
        "回退态一次抓取 = 主扫 + 存在性 + 见证者首探 + 前缀重扫 + 尾字母 1 次（宽松兜底档）"
    );
    v.store(0, Ordering::Relaxed);
    assert_eq!(r.select(0), "你好");
    assert_eq!(
        v.load(Ordering::Relaxed),
        one_fetch + 2,
        "点击 = 一次抓取 + 一次收尾判定（存在性 + 见证者，不多抓）"
    );
    assert_eq!(r.buffer(), "9", "回退选中只消耗前缀的键（两侧 k 同值）");
}
