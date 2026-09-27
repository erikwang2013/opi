// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `Mode::Symbol` 的键路由（`#[path]` 引入 input_method.rs，保持各文件 <500 行）。
//!
//! B3：符号模式此前是空壳 —— 路由把它与 Number 一起直通（`PassThrough` 一臂），
//! composer 对它的任何键都 `Ignored`，缓冲恒空，面板拿不到符号候选。补成真模式后：
//! 可见 ASCII 进缓冲当**关键字**（dun → 、、comma → ，），由 `SymbolEngine` 关键字索引
//! 搜候选；其余（空格、控制符、非 ASCII、特殊键、Ctrl/Alt 组合）照旧直通。
//! 提交符号后自动回拼音（用户裁决 2026-09-27：符号模式不跨键保持）。
//!
//! 三轨逐条同构：`crates/engine-core/tests/symbol_mode.rs` 与
//! `crates/tsf-opi/src/logic_symbol_tests.rs`（同名同序）。

use super::*;
use crate::candidate::CandidateState;
use engine_core::candidates::{Candidate, CandidateKind};
use engine_core::dictionary::InMemoryDictionary;

/// **空**词库 + 内置符号表：候选只可能来自符号关键字索引 —— 词库侧不干扰。
/// 大部分用例钉的是路由与提交收尾（候选表由用例自己给定）；`dun`/`ballot` 两条
/// 走真实索引，顺带覆盖 `candidates.rs` 的模式闸门（Symbol 只出符号、不合词库）。
fn symbol_state() -> CandidateState {
    let mut s = CandidateState {
        engine: engine_core::Engine::new(
            Box::new(InMemoryDictionary::new()),
            engine_core::symbols::SymbolEngine::builtin(),
            true,
        ),
        page: 0,
        buffer_snapshot: String::new(),
        shift_state: ShiftState::Off,
        last_printable: None,
    };
    s.switch_mode(Mode::Symbol);
    s
}

// ---- 可打印键进缓冲当关键字 ----
#[test]
fn symbol_mode_consumes_printable_keywords() {
    let mut s = symbol_state();
    for c in ['d', 'u', 'n'] {
        assert_eq!(handle_key(&mut s, c as u32, 0), KeyAction::EngineHandled);
    }
    assert_eq!(s.buffer(), "dun");
}

// ---- 验收 #8：打 dun → 候选第一条就是 、 ----
// 闸门在 `candidates.rs`（Symbol 只出符号、不合词库，用户裁决 2026-09-27）；
// 本层钉的是「路由送关键字进引擎 → 搜出符号 → 空格上屏并回拼音」整条链。
#[test]
fn symbol_mode_search_yields_symbol_candidates() {
    let mut s = symbol_state();
    for c in ['d', 'u', 'n'] {
        handle_key(&mut s, c as u32, 0);
    }
    assert_eq!(
        s.candidates()[0],
        "、",
        "dun 的首候选（data/raw/symbols.tsv:2）"
    );
    assert_eq!(
        handle_key(&mut s, KEY_SPACE, 0),
        KeyAction::Input("、".into())
    );
    assert_eq!(s.mode(), Mode::Pinyin, "选了符号就回拼音");
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
    let mut s = symbol_state();
    assert_eq!(
        handle_key(&mut s, '1' as u32, 0),
        KeyAction::PassThrough,
        "空缓冲无候选：数字交客户端，不吞"
    );
    assert_eq!(s.buffer(), "", "数字不进关键字缓冲");
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
    let mut s = symbol_state();
    for c in "ballot".chars() {
        handle_key(&mut s, c as u32, 0);
    }
    assert_eq!(s.candidates()[1], "☑");
    // 越界（本页 8 项，按 '9'）：交客户端，且**不进**关键字缓冲 —— 数字不是关键字
    assert_eq!(handle_key(&mut s, '9' as u32, 0), KeyAction::PassThrough);
    assert_eq!(s.buffer(), "ballot", "越界数字不得落进关键字缓冲");
    // 页内 '2' → 第二个候选
    assert_eq!(
        handle_key(&mut s, '2' as u32, 0),
        KeyAction::Input("☑".into())
    );
    assert_eq!(s.mode(), Mode::Pinyin, "选了符号就回拼音");
}

// ---- 非可打印键照旧直通 ----
#[test]
fn symbol_mode_keeps_non_printable_passthrough() {
    let mut s = symbol_state();
    for c in ['d', 'u', 'n'] {
        handle_key(&mut s, c as u32, 0);
    }
    assert_eq!(handle_key(&mut s, KEY_ESCAPE, 0), KeyAction::PassThrough);
    assert_eq!(handle_key(&mut s, KEY_TAB, 0), KeyAction::PassThrough);
    // 非 ASCII：可打印但不是关键字（fcitx5 侧键值是 xkb keysym，与码点同段）
    assert_eq!(handle_key(&mut s, '，' as u32, 0), KeyAction::PassThrough);
    assert_eq!(handle_key(&mut s, 0x4e2d, 0), KeyAction::PassThrough); // 中
    assert_eq!(s.buffer(), "dun", "非可打印键不得改动关键字缓冲");
    assert_eq!(s.page(), 0);
}

// ---- 空格：有候选选首候选，无候选**什么都不提交** ----
// 与拼音模式有意不同：拼音无候选时提交缓冲原文是「打英文」的逃生口，
// 而符号模式缓冲里是**关键字**（`dunx`），它不是要上屏的文本 ——
// 把拉丁原文塞进文档是这个模式最不该有的行为。缓冲留着不清：
// 用户看得见自己打的字，退格改一个字母就能重来（清了等于静默吞键）。
#[test]
fn symbol_mode_space_commits_nothing_without_candidates() {
    let mut s = symbol_state();
    // "qqq"：内置符号表无关键字以它开头，词库为空 → 必无候选。
    // 别拿 "zzz" 当乱码：它是 😴 的关键字，会走「有候选」那一支。
    for c in ['q', 'q', 'q'] {
        handle_key(&mut s, c as u32, 0);
    }
    assert_eq!(
        handle_key(&mut s, KEY_SPACE, 0),
        KeyAction::EngineHandled,
        "无候选：不上屏，也不吞掉缓冲"
    );
    assert_eq!(s.buffer(), "qqq", "缓冲须原样留着");
}

// ---- 抬起与按下同判（符号模式的可打印键已从直通改为消费） ----
#[test]
fn symbol_mode_release_matches_press() {
    let mut s = symbol_state();
    assert_eq!(handle_key(&mut s, 'd' as u32, 0), KeyAction::EngineHandled);
    assert_eq!(
        handle_key(&mut s, 'd' as u32, KEY_STATE_RELEASED),
        KeyAction::EngineHandled,
        "按下消费、抬起直通 → 客户端收到无 keydown 的 keyup"
    );
    assert_eq!(s.buffer(), "d", "抬起事件不得二次入引擎（否则关键字翻倍）");
}

// ---- 提交符号后回拼音（用户裁决：符号模式不跨键保持） ----
// 候选表由调用方给定：本层验的是「提交的收尾」，不依赖符号索引/词库是否命中。
#[test]
fn committing_symbol_returns_to_pinyin() {
    let mut s = symbol_state();
    for c in ['d', 'u', 'n'] {
        handle_key(&mut s, c as u32, 0);
    }
    let symbol = Candidate {
        text: "、".into(),
        kind: CandidateKind::Symbol,
        score: 0,
    };
    assert_eq!(s.select_from(&[symbol], 0), "、");
    assert_eq!(s.mode(), Mode::Pinyin, "符号是一次性的：提交后回拼音");
    assert_eq!(s.buffer(), "");
    assert_eq!(s.page(), 0, "换模式清了缓冲，页码须归零");
}

#[test]
fn committing_in_other_modes_keeps_mode() {
    let mut s = symbol_state();
    s.switch_mode(Mode::Traditional);
    let hanzi = Candidate {
        text: "好".into(),
        kind: CandidateKind::Hanzi,
        score: 0,
    };
    assert_eq!(s.select_from(&[hanzi], 0), "好");
    assert_eq!(s.mode(), Mode::Traditional, "回拼音只针对 Symbol 模式");
}
