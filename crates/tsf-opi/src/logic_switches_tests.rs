// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **两个标点开关**（`chinese_punct` / `fullwidth`）在本轨**路由层**的落点 ——
//! 从 `logic_punct_tests.rs` 拆出来（2026-09-28 两轨**对称**拆：fcitx5 轨的
//! 原文件被 `cargo fmt --all` 从 498 行推到 502 行撞了 500 行门禁；只拆一侧会让
//! 「两轨用例文件一一配对」在文件层面断掉，故两侧同拆）。**拆的是主题、不是行数** ——
//! engine-core 早一步已按同一条把开关那一批拆去了
//! `crates/engine-core/tests/punctuation_switches.rs`，本文件与它同主题：开关
//! （中文标点表档 / 机械全角档）与标点映射（哪个键出什么字）是两件事，前者的复位
//! 时机还随模式走（见 `Engine::set_chinese_punct` 的注释）。
//!
//! 两轨逐条同构：本文件 ↔ `crates/fcitx5-opi/src/input_method_switches_tests.rs`（同名同序）。
//! 标点映射那一批（引号交替、撇号分隔、flush 顺序、永不无声消失）仍在
//! `logic_punct_tests.rs`；两边都留了余量，别再往单边堆。

use super::*;
use engine_core::dictionary::InMemoryDictionary;

/// 词库只放 `ni → 你`：够钉「标点排在待提交的拼音后面」，别的都不掺。
fn punct_state() -> TsfLogic {
    let mut d = InMemoryDictionary::new();
    d.insert("ni", "你", 100);
    let mut s = TsfLogic {
        engine: engine_core::Engine::new(
            Box::new(d),
            engine_core::symbols::SymbolEngine::builtin(),
            true,
        ),
        page: 0,
        buffer_snapshot: String::new(),
        shift_state: ShiftState::Off,
        english_pro: true,
        last_printable: None,
        shift_toggle: true,
        fetch_cache: Default::default(),
    };
    s.switch_mode(Mode::Pinyin);
    s
}

fn typed(s: &mut TsfLogic, word: &str) {
    for c in word.chars() {
        s.input_key(c as u32, 0);
    }
}

// ---------- 全角开关 ----------

/// 默认值**跟着模式走**，且切模式重置 —— 与「切模式清 shift」同一条理由：
/// 跨模式残留的粘滞态会让用户「切回来发现打字是另一个样子」。
#[test]
fn fullwidth_default_follows_mode_and_resets_on_switch() {
    let mut s = punct_state();
    assert_eq!(s.input_key(',' as u32, 0), KeyOutcome::Commit("，".into()));
    s.switch_mode(Mode::English);
    assert_eq!(
        s.input_key(',' as u32, 0),
        KeyOutcome::Commit(",".into()),
        "英文：半角直传"
    );
    assert!(s.toggle_fullwidth(), "开关返回新状态，供状态栏显示");
    assert_eq!(s.input_key(',' as u32, 0), KeyOutcome::Commit("，".into()));
    s.switch_mode(Mode::Number);
    assert_eq!(
        s.input_key(',' as u32, 0),
        KeyOutcome::Unhandled,
        "切模式重置为模式默认，不记住手动值"
    );
    s.switch_mode(Mode::Traditional);
    assert_eq!(s.input_key(',' as u32, 0), KeyOutcome::Commit("，".into()));
}

/// **纯半角直通**（真值表第 4 行）：两个标点开关**都关**，标点才全部放行
/// （交应用自己插半角字符），字母照旧进缓冲。只关全角**不再是**这一档 ——
/// 中文标点表由 `engine.chinese_punct` 单独管（见下一条），那时 `,` 照出 `，`。
/// 撇号不在这一列：它在拼音里是音节分隔符，全关态照样进缓冲（见上一条）——
/// 关掉的是标点**映射**，不是分隔符语义。
#[test]
fn chinese_halfwidth_passes_everything_through() {
    let mut s = punct_state();
    s.engine.set_chinese_punct(false);
    s.toggle_fullwidth();
    for c in [',', '.', '\\', '[', '"'] {
        assert_eq!(s.input_key(c as u32, 0), KeyOutcome::Unhandled, "{c:?}");
    }
    typed(&mut s, "ni");
    assert_eq!(s.buffer(), "ni", "标点开关不影响字母入缓冲");
}

/// 真值表**逐格**（拼音，四组合 × 4 键）。用户裁决 2026-09-28：`chinese_punct` 管
/// 中文标点表那一档、`fullwidth` 管机械全角那一档，两个开关互不牵连 ——
/// 关全角不再把整张中文标点表一起关掉（那是拆开关要修的 bug）。
/// 引擎层同表见 `crates/engine-core/tests/punctuation_switches.rs` 的
/// `chinese_punct_and_fullwidth_are_independent`（本轨多一层：None 落成 Unhandled）。
#[test]
fn chinese_punct_and_fullwidth_are_independent() {
    // (中文标点, 全角, `,` `.` `\` `^` 的期望；None = 放行)
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
                Some(t) => KeyOutcome::Commit(t.into()),
                None => KeyOutcome::Unhandled,
            };
            assert_eq!(
                s.input_key(k as u32, 0),
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
    assert_eq!(s.input_key('.' as u32, 0), KeyOutcome::Commit(".".into()));
    s.toggle_fullwidth();
    assert_eq!(s.input_key('.' as u32, 0), KeyOutcome::Commit("．".into()));
    assert_eq!(s.input_key('"' as u32, 0), KeyOutcome::Commit("＂".into()));
    assert_eq!(
        s.input_key('"' as u32, 0),
        KeyOutcome::Commit("＂".into()),
        "西文引号不交替"
    );
    // 字母/数字**也**进全角（用户裁决 2026-10-12「需要，修复所有问题」）——
    // 与「标点档只管标点」的分档是两回事：判据是**这个字符在这模式下是不是待上屏文本**
    // （英文/数字模式是；拼音模式是输入码，绝不宽 —— 见 engine-core 的
    // punctuation::ascii_fullwidth 头注）。映射与开关唯一在引擎 `Engine::fullwidthize`。
    // 字母自 2026-10-18 起进缓冲（单词联想），全角在**提交点**生效（空格/选词）。
    assert_eq!(s.input_key('a' as u32, 0), KeyOutcome::CompositionChanged);
    assert_eq!(s.buffer(), "a");
    s.engine.clear();
    assert_eq!(s.input_key('1' as u32, 0), KeyOutcome::Commit("１".into()));
    // 空格**不宽化**（有意，别顺手补）：全角在拼音模式默认开，宽空格会把「中文模式按
    // 一下空格」换成 U+3000 —— 见 `Engine::fullwidthize` 头注。
    s.engine.clear();
    assert_eq!(
        s.input_key(' ' as u32, 0),
        KeyOutcome::Commit(" ".into()),
        "空缓冲的空格照旧半角（全角只吃 ASCII 可打印、除空格）"
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
    assert_eq!(s.input_key(',' as u32, 0), KeyOutcome::Commit(",".into()));
    assert_eq!(s.buffer(), "", "标点不是关键字，不进缓冲");
    s.toggle_fullwidth();
    assert_eq!(
        s.input_key(',' as u32, 0),
        KeyOutcome::Commit(",".into()),
        "全角开关在符号模式不生效"
    );
}

// ---------- 模式切换 × ⇧ ----------

/// 拼音里按 ⇧（SINGLE/LOCK）再切 English：引擎 shift 刚被 `switch_mode` 清掉，
/// 前端三态必须重推 —— 否则高亮着却打出小写（四端同一条语义）。
#[test]
fn switch_to_english_reapplies_shift_state() {
    let mut s = punct_state();
    s.shift_tap(); // SINGLE
    s.switch_mode(Mode::English);
    assert_eq!(s.input_key('a' as u32, 0), KeyOutcome::CompositionChanged);
    assert_eq!(s.buffer(), "A", "SINGLE 重推：首个字母大写");
    assert_eq!(s.shift_state(), ShiftState::Off, "single 用掉即复位");

    let mut s = punct_state();
    s.shift_long_press(); // LOCK
    s.switch_mode(Mode::English);
    s.input_key('a' as u32, 0);
    s.input_key('b' as u32, 0);
    assert_eq!(s.buffer(), "AB", "LOCK 重推且持续");
    assert_eq!(s.shift_state(), ShiftState::Lock);
}

// ---------- toggle_language（Shift 轻按 / 状态栏 lang 共用的语义本体） ----------

/// 物理 Shift 的「轻按」判定在胶水层（Linux=C++、TSF=tsf.rs，见 crate::shift_tap）；
/// 这里只钉语义本体两件事：两个方向都切、进英文时待处理拼音按 W1 上原文。
#[test]
fn toggle_language_switches_both_ways_and_commits_raw_pinyin() {
    use crate::logic::TsfLogic;

    let mut s = TsfLogic::load(None).expect("内置回退词库");
    s.switch_mode(Mode::Pinyin);
    assert_eq!(s.toggle_language(), "", "空缓冲切英文：没有要上屏的");
    assert_eq!(s.mode(), Mode::English);
    assert_eq!(s.toggle_language(), "", "英文切回拼音：无提交");
    assert_eq!(s.mode(), Mode::Pinyin);

    // 有缓冲：切英文前把拼音原文交出去（不是首候选、更不是静默丢弃）
    for c in "nihao".chars() {
        s.input_key(c as u32, 0);
    }
    assert_eq!(s.toggle_language(), "nihao");
    assert_eq!(s.mode(), Mode::English);
    assert_eq!(s.buffer(), "");
    assert_eq!(s.shift_state(), ShiftState::Off, "轻按不该把大写带进英文档");
}
