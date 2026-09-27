// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **两个标点开关**（`chinese_punct` / `fullwidth`）在本轨**路由层**的落点 ——
//! 从 `input_method_punct_tests.rs` 拆出来（2026-09-28：`cargo fmt --all` 把原文件从
//! 498 行推到 502 行，撞 500 行门禁）。**拆的是主题、不是行数** —— engine-core 早一步
//! 已按同一条把开关那一批拆去了 `crates/engine-core/tests/punctuation_switches.rs`，
//! 本文件与它同主题：开关（中文标点表档 / 机械全角档）与标点映射（哪个键出什么字）
//! 是两件事，前者的复位时机还随模式走（见 `Engine::set_chinese_punct` 的注释）。
//!
//! 两轨逐条同构：本文件 ↔ `crates/tsf-opi/src/logic_switches_tests.rs`（同名同序）。
//! 标点映射那一批（引号交替、撇号分隔、flush 顺序、永不无声消失）仍在
//! `input_method_punct_tests.rs`；两边都留了余量，别再往单边堆。

use super::*;
use crate::candidate::CandidateState;
use engine_core::dictionary::InMemoryDictionary;

/// 词库只放 `ni → 你`：够钉「标点排在待提交的拼音后面」，别的都不掺。
fn punct_state() -> CandidateState {
    let mut d = InMemoryDictionary::new();
    d.insert("ni", "你", 100);
    let mut s = CandidateState {
        engine: engine_core::Engine::new(
            Box::new(d),
            engine_core::symbols::SymbolEngine::builtin(),
            true,
        ),
        page: 0,
        buffer_snapshot: String::new(),
        shift_state: ShiftState::Off,
        last_printable: None,
    };
    s.switch_mode(Mode::Pinyin);
    s
}

fn typed(s: &mut CandidateState, word: &str) {
    for c in word.chars() {
        handle_key(s, c as u32, 0);
    }
}

// ---------- 全角开关 ----------

/// 默认值**跟着模式走**，且切模式重置 —— 与「切模式清 shift」同一条理由：
/// 跨模式残留的粘滞态会让用户「切回来发现打字是另一个样子」。
#[test]
fn fullwidth_default_follows_mode_and_resets_on_switch() {
    let mut s = punct_state();
    assert_eq!(
        handle_key(&mut s, ',' as u32, 0),
        KeyAction::Input("，".into())
    );
    s.switch_mode(Mode::English);
    assert_eq!(
        handle_key(&mut s, ',' as u32, 0),
        KeyAction::Input(",".into()),
        "英文：半角直传"
    );
    assert!(s.toggle_fullwidth(), "开关返回新状态，供状态栏显示");
    assert_eq!(
        handle_key(&mut s, ',' as u32, 0),
        KeyAction::Input("，".into())
    );
    s.switch_mode(Mode::Number);
    assert_eq!(
        handle_key(&mut s, ',' as u32, 0),
        KeyAction::PassThrough,
        "切模式重置为模式默认，不记住手动值"
    );
    s.switch_mode(Mode::Traditional);
    assert_eq!(
        handle_key(&mut s, ',' as u32, 0),
        KeyAction::Input("，".into())
    );
}

/// **纯半角直通**（真值表第 4 行）：两个标点开关**都关**，标点才全部直通
/// （交客户端自己插半角字符），字母照旧进缓冲。只关全角**不再是**这一档 ——
/// 中文标点表由 `engine.chinese_punct` 单独管（见下一条），那时 `,` 照出 `，`。
/// 撇号不在这一列：它在拼音里是音节分隔符，全关态照样进缓冲（见上一条）——
/// 关掉的是标点**映射**，不是分隔符语义。
#[test]
fn chinese_halfwidth_passes_everything_through() {
    let mut s = punct_state();
    s.engine.set_chinese_punct(false);
    s.toggle_fullwidth();
    for c in [',', '.', '\\', '[', '"'] {
        assert_eq!(
            handle_key(&mut s, c as u32, 0),
            KeyAction::PassThrough,
            "{c:?}"
        );
    }
    typed(&mut s, "ni");
    assert_eq!(s.buffer(), "ni", "标点开关不影响字母入缓冲");
}

/// 真值表**逐格**（拼音，四组合 × 4 键）。用户裁决 2026-09-28：`chinese_punct` 管
/// 中文标点表那一档、`fullwidth` 管机械全角那一档，两个开关互不牵连 ——
/// 关全角不再把整张中文标点表一起关掉（那是拆开关要修的 bug）。
/// 引擎层同表见 `crates/engine-core/tests/punctuation_switches.rs` 的
/// `chinese_punct_and_fullwidth_are_independent`（本轨多一层：None 落成 PassThrough）。
#[test]
fn chinese_punct_and_fullwidth_are_independent() {
    // (中文标点, 全角, `,` `.` `\` `^` 的期望；None = 直通)
    const CASES: &[(bool, bool, [Option<&str>; 4])] = &[
        (true, true, [Some("，"), Some("。"), Some("、"), Some("＾")]),
        (true, false, [Some("，"), Some("。"), Some("、"), None]),
        (
            false,
            true,
            [Some("，"), Some("．"), Some("＼"), Some("＾")],
        ),
        (false, false, [None, None, None, None]),
    ];
    const KEYS: [char; 4] = [',', '.', '\\', '^'];
    for &(cp, fw, want) in CASES {
        for (i, &k) in KEYS.iter().enumerate() {
            let mut s = punct_state();
            s.engine.set_chinese_punct(cp);
            if !fw {
                s.toggle_fullwidth(); // 拼音默认全角（switch_mode 重置），只在要半角时切
            }
            let want = match want[i] {
                Some(t) => KeyAction::Input(t.into()),
                None => KeyAction::PassThrough,
            };
            assert_eq!(
                handle_key(&mut s, k as u32, 0),
                want,
                "chinese_punct={cp} fullwidth={fw} 键 {k:?}"
            );
        }
    }
}

/// `chinese_punct` 是**用户偏好、不随模式重置**（与 `fullwidth` 有意相反）：
/// 表只被 Pinyin / Traditional 读，对它而言这不是某个模式的默认值 ——
/// 切到英文再切回来，用户关掉的表不该自己打开。
#[test]
fn chinese_punct_survives_mode_switch() {
    let mut s = punct_state();
    s.engine.set_chinese_punct(false);
    for mode in [
        Mode::English,
        Mode::Number,
        Mode::Symbol,
        Mode::Traditional,
        Mode::Pinyin,
    ] {
        s.switch_mode(mode);
        assert!(!s.engine.chinese_punct(), "{mode:?} 切模式不该重置这档偏好");
    }
}

/// 英文/数字模式的全角是**机械全角**：`.` 得 `．`(U+FF0E)，不是中文句号 `。`
/// —— 西文文本里冒出一个中文句号是错的。
#[test]
fn non_chinese_fullwidth_is_mechanical_not_cjk() {
    let mut s = punct_state();
    s.switch_mode(Mode::English);
    assert_eq!(
        handle_key(&mut s, '.' as u32, 0),
        KeyAction::Input(".".into())
    );
    s.toggle_fullwidth();
    assert_eq!(
        handle_key(&mut s, '.' as u32, 0),
        KeyAction::Input("．".into())
    );
    assert_eq!(
        handle_key(&mut s, '"' as u32, 0),
        KeyAction::Input("＂".into())
    );
    assert_eq!(
        handle_key(&mut s, '"' as u32, 0),
        KeyAction::Input("＂".into()),
        "西文引号不交替"
    );
    // 字母/数字不进全角：全角字母表是另一件事，本表只管标点
    assert_eq!(
        handle_key(&mut s, 'a' as u32, 0),
        KeyAction::Input("a".into())
    );
}

/// 符号模式：不映射 —— 标点在关键字表里根本不存在（生产表关键字全是 `[a-z0-9]`，
/// 见 engine-core 的 every_symbol_keyword_is_alnum）。落到路由上就是**原样插入**：
/// `,` 出 `,` 不是 `，`，也不进关键字缓冲（进去就是搜不出候选的死缓冲 ——
/// 符号模式的可见 ASCII 臂同样收字符，所以表那一臂必须在模式分派**之前**）。
#[test]
fn symbol_mode_has_no_punctuation_mapping() {
    let mut s = punct_state();
    s.switch_mode(Mode::Symbol);
    assert_eq!(
        handle_key(&mut s, ',' as u32, 0),
        KeyAction::Input(",".into())
    );
    assert_eq!(s.buffer(), "", "标点不是关键字，不进缓冲");
    s.toggle_fullwidth();
    assert_eq!(
        handle_key(&mut s, ',' as u32, 0),
        KeyAction::Input(",".into()),
        "全角开关在符号模式不生效"
    );
}
