// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 候选上限门禁（两轨同源，配对：`candidate_limits_tests.rs` ↔ `logic_limits_tests.rs`）。
//!
//! ## 为什么要有这一条
//!
//! `FETCH_LIMIT` 曾在**三处各存一份**（`engine-core/src/router.rs` 与两轨），三者之间
//! 没有任何绑定：v1.3.0 只把 `router.rs` 那份抬成 `usize::MAX`，两轨仍留在 64，**没有
//! 一条门禁发现得了**。后果是产品级截断：真词库下 `y` 前缀一次命中**八千余条**，本轨用户
//! 翻到第 64 条（≈0.8%）就没有下一页了，而 64 与 65 名的静态词频只差 0.01%（无断崖，用户
//! 只会觉得"后面没有词了"）。根因不是那个 64，是**三份常量之间零绑定**。
//! ⚠️ 「八千余」别换成精确数字：它同时取决于词库与符号表，两者本会话内都被并发再生过 ——
//! 同一个 `y` 实测到 8006（空符号表，恰与 `router.rs` 注释同数）/ 8017 / 8028（内建符号表，
//! 本轮相隔几分钟的两次读数）。**它不该当判据**，见 `router_invariants.rs` 那类教训。
//!
//! ## 两道门禁、两种锚 —— 都不从被检查的那个常量派生
//!
//! 1. **定义处 `const _` 断言**（`candidate.rs` 常量正下方）：拿本文件的常量去比
//!    `engine_core::router` 的同名常量 —— 期望值来自**另一份源码**，不是本文件的字面量，
//!    也不是同一个常量。编译期求值：漂回 64 是 **E0080 编译失败**，不是"某条测试恰好
//!    没跑到"。为什么放定义处：见 `vk.rs` 同款断言的注释（常量就在上面几行，改的人没法
//!    不看见它）。
//! 2. **本文件的 300 / 8**：上面那条**挡不住"三份一起漂回 64"**（两边同源 ⇒ 一起缩 ⇒
//!    照绿；`opi-ffi/tests/keycode_contract.rs` 已把这条列为已知的缺口形状并写明补法）。
//!    故这里再钉一条**独立字面量**：越过 64 时代的任何上限，截断一回来即红。
//!
//! ⚠️ 反面教材（本仓同类门禁栽过的那次）：`engine-core/tests/router_invariants.rs` 里
//! 老用例的期望值取自 `engine.candidates(FETCH_LIMIT)` —— 与被断言的 `page_count()` 从
//! 同一个常量派生，`FETCH_LIMIT = 64` 时**全部用例照绿**（含 n=100）。该文件 120-126 行
//! 已自行订正并记下这次教训，本文件不重犯：300 与 8 都是**独立字面量**，谁都不随常量缩放。
//!
//! ## 覆盖边界（如实记，本门禁**不**保证的）
//!
//! * 只钉 `FETCH_LIMIT` / `PAGE_SIZE` 这两个常量，与"翻页能不能翻到底"。
//! * **Android 的 `fetchLimit = 64` 不在射程内**：那是 JNI 通路，够不到引擎侧分页出口，
//!   属结构性不同的另一条路（另一个 owner）。
//! * macOS/iOS 的 `InputController.swift` 同理不在本 crate。
//! * `page_count()` 的实现形状（`div_ceil`）两轨是否同构，由人比对，本文件只说结果。
//!
//! ## 键路径说明
//!
//! 缓冲经 `s.engine.input_key`（**engine-core 的 API**）置入，不走本轨的键路由 ——
//! 本门禁测的是分页天花板，键路径由既有的候选测试与键路由测试覆盖。顺带的好处是这行
//! 两轨逐字相同，配对 diff 不必为此再多一条归一化规则。

use super::*;
use engine_core::dictionary::InMemoryDictionary;
use engine_core::symbols::SymbolEngine;

/// `n` 条 "hao" 词条（词频严格递减 → 顺序确定），符号表取**空表**：
/// 候选集恰为 `n` 条，符号/emoji 一条都不混进来（与 `router_invariants.rs` 同法）。
fn state_n(n: usize) -> TsfLogic {
    let mut d = InMemoryDictionary::new();
    for i in 0..n {
        d.insert("hao", &format!("词{i:03}"), (n - i) as u32);
    }
    let symbols = SymbolEngine::new(Vec::new(), Vec::new());
    let mut s = TsfLogic {
        engine: Engine::new(Box::new(d), symbols, true),
        page: 0,
        buffer_snapshot: String::new(),
        shift_state: ShiftState::Off,
        last_printable: None,
    };
    s.refresh_snapshot();
    s
}

/// 翻页必须覆盖引擎排出的**全部**候选，不是某个固定条数。
#[test]
fn paging_reaches_past_any_64_era_ceiling() {
    const N: usize = 300; // 独立字面量：越过 64 时代的任何上限
    const PER_PAGE: usize = 8; // 独立字面量：Android 候选栏 + `router.rs` 注释双重记载
    let mut s = state_n(N);
    for c in ['h', 'a', 'o'] {
        s.engine.input_key(c);
    }
    assert_eq!(
        s.page_count(),
        N.div_ceil(PER_PAGE),
        "总页数必须覆盖全部 {N} 条（截断一回来即红）"
    );
    let mut got: Vec<String> = Vec::new();
    for p in 0..s.page_count() {
        s.set_page(p);
        got.extend(s.candidates());
    }
    let last = format!("词{:03}", N - 1);
    assert_eq!(got.len(), N, "逐页翻到底必须能取到全部 {N} 条");
    assert_eq!(got.first().map(String::as_str), Some("词000"), "首页首条");
    assert_eq!(got.last().map(String::as_str), Some(&*last), "末页末条");
}
