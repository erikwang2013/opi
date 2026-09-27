// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `Mode::Symbol` 的键路由与提交收尾（B3 把符号模式从空壳补成真模式）。
//!
//! B3 之前：路由把它与 Number 一起直通（`PassThrough` 一臂），composer 对它的任何键
//! 都 `Ignored`，缓冲恒空，四端都拿不到符号候选。补完后的契约：
//! - 可见 ASCII 进缓冲当**关键字**（dun → 、、comma → ，），由 `SymbolEngine` 关键字
//!   索引搜候选；空格、控制符、非 ASCII、特殊键照旧直通；
//! - 提交一个符号后自动回拼音（用户裁决 2026-09-27：符号模式不跨键保持）。
//!
//! 本文件走**公开 API**（`KeyRouter` + `Engine`）：两轨的 `select_from` 是 `pub(crate)`，
//! 各自的符号模式用例落在 crates/fcitx5-opi/src/input_method_symbol_tests.rs 与
//! crates/tsf-opi/src/logic_symbol_tests.rs（同名同序）；本层是平台中立的那一份。
//! 放在 tests/ 而非 src/router_symbol_tests.rs：router.rs 已在 490 行的 500 行上限
//! 边缘，再挂一个 `#[path]` 测试模块就顶破（项目硬规矩）。

use engine_core::candidates::{Candidate, CandidateKind, DEFAULT_TOP_N};
use engine_core::dictionary::InMemoryDictionary;
use engine_core::keys::{KEY_ESCAPE, KEY_SPACE, KEY_STATE_RELEASED, KEY_TAB, KEY_UP};
use engine_core::router::{KeyAction, KeyRouter};
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Mode};

/// **空**词库 + 内置符号表：候选只可能来自符号关键字索引 —— 词库侧不干扰。
/// 大部分用例钉的是路由与提交收尾（候选表由用例自己给定）；`dun`/`ballot` 两条
/// 走真实索引，顺带覆盖 `candidates.rs` 的模式闸门（Symbol 只出符号、不合词库）。
fn symbol_state() -> KeyRouter {
    let mut r = KeyRouter::new(Engine::new(
        Box::new(InMemoryDictionary::new()),
        SymbolEngine::builtin(),
        true,
    ));
    r.switch_mode(Mode::Symbol);
    r
}

fn symbol_candidate() -> Candidate {
    Candidate {
        text: "、".into(),
        kind: CandidateKind::Symbol,
        score: 0,
    }
}

// ---- 可打印键进缓冲当关键字 ----
#[test]
fn symbol_mode_consumes_printable_keywords() {
    let mut r = symbol_state();
    for c in ['d', 'u', 'n'] {
        assert_eq!(r.key_event(c as u32, 0), KeyAction::EngineHandled);
    }
    assert_eq!(r.buffer(), "dun");
}

// ---- 验收 #8：打 dun → 候选第一条就是 、 ----
// 闸门在 `candidates.rs`（Symbol 模式只出符号、不与词库合并，用户裁决 2026-09-27）；
// 本层用空词库，钉的是「路由把关键字送进引擎 → 引擎搜出符号 → 空格上屏并回拼音」整条链。
#[test]
fn symbol_mode_search_yields_symbol_candidates() {
    let mut r = symbol_state();
    for c in ['d', 'u', 'n'] {
        r.key_event(c as u32, 0);
    }
    let cands = r.engine().candidates(DEFAULT_TOP_N);
    assert_eq!(
        cands[0].text, "、",
        "dun 的首候选（data/raw/symbols.tsv:2）"
    );
    assert_eq!(cands[0].kind, CandidateKind::Symbol);
    assert_eq!(
        r.key_event(KEY_SPACE, 0),
        KeyAction::Input("、".into()),
        "空格 = 选首候选"
    );
    assert_eq!(r.mode(), Mode::Pinyin, "选了符号就回拼音");
}

// ---- 数字在符号模式下当**选词键**（裁决 2026-09-27，与拼音一致），不进关键字缓冲 ----
// 空缓冲那一半**与 candidates.rs 的模式闸门无关**（空缓冲必无候选，闸门开了也一样）；
// 有候选那一半（下面 `symbol_mode_digit_selects_candidate`）要闸门开了才写得出 ——
// 与验收 #8 同一条理由，闸门开之前写只能是假绿。
// 代价（如实记）：`symbols.tsv` 里 15 条符号带**数字别名**关键字（骰子一 ⚀ "1"、
// 回收 ♳ "1"…），这条路径因此没了；实测这 15 条**全部另有字母关键字**
// （touzi/dice、recycling/type/plastics、restricted/entry），故无一符号变得打不出来。
#[test]
fn symbol_mode_digit_does_not_enter_keyword_buffer() {
    let mut r = symbol_state();
    assert_eq!(
        r.key_event('1' as u32, 0),
        KeyAction::PassThrough,
        "空缓冲无候选：数字交应用，不吞"
    );
    assert_eq!(r.buffer(), "", "数字不进关键字缓冲");
}

// ---- 数字选词的另一半（有候选时）：页内 '1'..='9' 提交对应候选 ----
// `data/raw/symbols.tsv` 的 ballot 组 12 条（☐ ☑ ☒ ✗ ✘ ⮽ 🗳 🗴 …），本页取前 8 条；
// 关键字索引按 (小写关键字, 条目序) 排序，故同关键字下的页内序稳定 = 文件序。
//
// 越界探针取 `'9'` 而不是 `'4'`：探针的**前提**是「本页没这么多项」，而这个数会随
// 表长漂（2026-09-28 扩充符号表时 ballot 从 3 条涨到 12 条，`'4'` 就落进范围内了 ——
// 断言当场变红，红的不是不变式而是前提）。`PAGE_SIZE = 8` 是 `router.rs` 的常量，
// 故 `'9'` 对任何页宽 ≤ 8 的表都**恒**越界，探针不会因数据变动而失去意义。
#[test]
fn symbol_mode_digit_selects_candidate() {
    let mut r = symbol_state();
    for c in "ballot".chars() {
        r.key_event(c as u32, 0);
    }
    assert_eq!(r.engine().candidates(DEFAULT_TOP_N)[1].text, "☑");
    // 越界（本页 8 项，按 '9'）：交应用，且**不进**关键字缓冲 —— 数字不是关键字
    assert_eq!(r.key_event('9' as u32, 0), KeyAction::PassThrough);
    assert_eq!(r.buffer(), "ballot", "越界数字不得落进关键字缓冲");
    // 页内 '2' → 第二个候选
    assert_eq!(r.key_event('2' as u32, 0), KeyAction::Input("☑".into()));
    assert_eq!(r.mode(), Mode::Pinyin, "选了符号就回拼音");
}

// ---- 非可打印键照旧直通 ----
#[test]
fn symbol_mode_keeps_non_printable_passthrough() {
    let mut r = symbol_state();
    for c in ['d', 'u', 'n'] {
        r.key_event(c as u32, 0);
    }
    assert_eq!(r.key_event(KEY_ESCAPE, 0), KeyAction::PassThrough);
    assert_eq!(r.key_event(KEY_TAB, 0), KeyAction::PassThrough);
    // 本层独有的特殊键（两轨的方向键由各自 C++/C2 侧处理，不在路由表里）
    assert_eq!(r.key_event(KEY_UP, 0), KeyAction::PassThrough);
    // 非 ASCII：可打印但不是关键字
    assert_eq!(r.key_event('，' as u32, 0), KeyAction::PassThrough);
    assert_eq!(r.key_event(0x4e2d, 0), KeyAction::PassThrough); // 中
    assert_eq!(r.buffer(), "dun", "非可打印键不得改动关键字缓冲");
}

// ---- 空格：有候选选首候选，无候选**什么都不提交** ----
// 与拼音模式有意不同：拼音无候选时提交缓冲原文是「打英文」的逃生口，
// 而符号模式缓冲里是**关键字**（`dunx`），它不是要上屏的文本 ——
// 把拉丁原文塞进文档是这个模式最不该有的行为。缓冲留着不清：
// 用户看得见自己打的字，退格改一个字母就能重来（清了等于静默吞键）。
#[test]
fn symbol_mode_space_commits_nothing_without_candidates() {
    let mut r = symbol_state();
    // "qqq"：内置符号表无此关键字前缀（见 entering_symbol_drops_unmatched_buffer）
    for c in ['q', 'q', 'q'] {
        r.key_event(c as u32, 0);
    }
    assert_eq!(
        r.key_event(KEY_SPACE, 0),
        KeyAction::EngineHandled,
        "无候选：不上屏，也不吞掉缓冲"
    );
    assert_eq!(r.buffer(), "qqq", "缓冲须原样留着");
}

// ---- 抬起与按下同判（符号模式的可打印键已从直通改为消费） ----
#[test]
fn symbol_mode_release_matches_press() {
    let mut r = symbol_state();
    assert_eq!(r.key_event('d' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(
        r.key_event('d' as u32, KEY_STATE_RELEASED),
        KeyAction::EngineHandled,
        "按下消费、抬起直通 → 客户端收到无 keydown 的 keyup"
    );
    assert_eq!(r.buffer(), "d", "抬起事件不得二次入引擎（否则关键字翻倍）");
}

// ---- 提交符号后回拼音（用户裁决：符号模式不跨键保持） ----
#[test]
fn committing_symbol_returns_to_pinyin() {
    let mut r = symbol_state();
    for c in ['d', 'u', 'n'] {
        r.key_event(c as u32, 0);
    }
    // `KeyRouter::select_from` 是 pub(crate)（集成测试够不着），经 `engine_mut` 走同一段
    // 收尾；`engine_mut` 的文档要求绕过路由改引擎后手动归零页码。
    let out = r.engine_mut().select_from(&[symbol_candidate()], 0);
    r.reset_page_if_buffer_changed();
    assert_eq!(out, "、");
    assert_eq!(r.mode(), Mode::Pinyin, "符号是一次性的：提交后回拼音");
    assert_eq!(r.buffer(), "");
    assert_eq!(r.page(), 0, "换模式清了缓冲，页码须归零");
}

// ---- 触发键进出符号模式：不丢未提交的拼音 ----
// 用户裁决 2026-09-27：进符号模式前先把 pending 处理掉 —— 照抄 Android
// `ImeState.commitPendingBuffer()`（ImeState.kt:180-189，`openSymbol`/`openNumber`
// 开面板前都先调它）；退出只用「再按一次触发键」（Esc 是产品变更：两轨都有
// 「缓冲非空时 Esc 直通且不吞缓冲」的断言）。
// **键位未定**（计划 B5 标注「实现时二选一实测」，TSF 侧还要动 vk.rs 的映射表），
// 故本层只落语义、不绑键；触发键接线由路由层/前端做。
fn pending_engine() -> Engine {
    let mut dict = InMemoryDictionary::new();
    dict.insert("ni", "你", 100);
    Engine::new(Box::new(dict), SymbolEngine::builtin(), true)
}

#[test]
fn entering_symbol_commits_pending_pinyin() {
    let mut e = pending_engine();
    for c in "ni".chars() {
        e.input_key(c);
    }
    assert_eq!(
        e.toggle_symbol(),
        "你",
        "有候选：首候选上屏，不随缓冲一起丢"
    );
    assert_eq!(e.mode(), Mode::Symbol);
    assert_eq!(e.buffer(), "");
}

#[test]
fn entering_symbol_drops_unmatched_buffer() {
    let mut e = pending_engine();
    // "qqq"：内置符号表里没有任何关键字以它开头（`data/raw/symbols.tsv` 的关键字前缀
    // 无 qqq），词库也没有 —— 这就是 Android 说的「乱码缓冲」。
    // 别拿 "zzz" 当乱码：它是 😴 的关键字，会走「有候选」那一支。
    for c in "qqq".chars() {
        e.input_key(c);
    }
    assert_eq!(e.toggle_symbol(), "", "无候选的乱码缓冲：清掉，**不上屏**");
    assert_eq!(e.mode(), Mode::Symbol);
    assert_eq!(e.buffer(), "");
}

#[test]
fn toggling_symbol_again_returns_to_pinyin() {
    let mut e = pending_engine();
    assert_eq!(e.toggle_symbol(), "");
    assert_eq!(e.mode(), Mode::Symbol);
    assert_eq!(e.toggle_symbol(), "", "空缓冲：无文本可交，不留残留");
    assert_eq!(e.mode(), Mode::Pinyin, "再按一次触发键 = 退出（不用 Esc）");
}

#[test]
fn committing_in_other_modes_keeps_mode() {
    let mut r = symbol_state();
    r.switch_mode(Mode::Traditional);
    let hanzi = Candidate {
        text: "好".into(),
        kind: CandidateKind::Hanzi,
        score: 0,
    };
    assert_eq!(r.engine_mut().select_from(&[hanzi], 0), "好");
    assert_eq!(r.mode(), Mode::Traditional, "回拼音只针对 Symbol 模式");
}
