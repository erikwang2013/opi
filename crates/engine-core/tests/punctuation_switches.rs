// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **两个标点开关**（`chinese_punct` / `fullwidth`）的行为锁 —— 从 `punctuation.rs`
//! 拆出来（本仓 500 行门禁，与 `input_method_tests.rs` 拆出 `*_punct_tests.rs` 同一条）。
//!
//! 契约（用户裁决 2026-09-28「全角/半角」与「中文标点」拆成两个独立开关）：
//! - `chinese_punct` 管中文标点表那一档（含成对引号交替），**不随模式重置**（用户偏好）；
//! - `fullwidth` 管机械全角那一档，**随模式重置**（[`Mode::default_fullwidth`]）；
//! - 只关全角**不再**把中文标点表一起关掉 —— 那是拆开关要修的 bug，真值表逐格见
//!   `chinese_punct_and_fullwidth_are_independent`；
//! - 符号模式两个开关都不生效（它是搜索模式，不是文本模式）。
//!
//! 两轨镜像（同名同序）：标点在 crates/fcitx5-opi/src/input_method_punct_tests.rs 与
//! crates/tsf-opi/src/logic_punct_tests.rs，**开关**这一批两轨各多拆了一层
//! （crates/{fcitx5-opi,tsf-opi}/src/{input_method,logic}_switches_tests.rs）。
//!
//! **已同步**（2026-09-28 订正）：原文写「这两条镜像本轮未同步，按『只关全角 ⇒ 直通』写的
//! 用例会红，等下一波接线时一并改」—— 实测不成立：`cargo test -p fcitx5_opi -p tsf_opi`
//! 全绿（0 failed），两侧的旧用例都已改完。**别把「未同步」写回来**：读到那句话的人会去找
//! 两个**根本不存在**的红灯（本波已有一个分身据此以为套件是红的）。

use engine_core::dictionary::InMemoryDictionary;
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Mode};

/// 词库只放 `ni → 你`：够钉「标点排在待提交的拼音后面」，别的都不掺。
fn engine() -> Engine {
    let mut d = InMemoryDictionary::new();
    d.insert("ni", "你", 100);
    Engine::new(Box::new(d), SymbolEngine::builtin(), true)
}

// ---------- 全角开关 ----------

/// 默认值**跟着模式走**，且切模式重置 —— 与「切模式清 shift」同一条理由：
/// 跨模式残留的粘滞态会让用户「切回来发现打字是另一个样子」。
#[test]
fn fullwidth_default_follows_mode_and_resets_on_switch() {
    let mut e = engine();
    assert!(e.fullwidth(), "拼音：自动全角");
    e.switch_mode(Mode::English);
    assert!(!e.fullwidth(), "英文：半角");
    assert!(e.toggle_fullwidth(), "开关返回新状态，供状态栏显示");
    assert!(e.fullwidth());
    e.switch_mode(Mode::Number);
    assert!(!e.fullwidth(), "切模式重置为模式默认，不记住手动值");
    e.switch_mode(Mode::Symbol);
    assert!(!e.fullwidth());
    e.switch_mode(Mode::Traditional);
    assert!(e.fullwidth(), "繁体与拼音同档");
}

/// **纯半角直通**（真值表第 4 行）：两个开关**都关**，标点一个都不映射
/// （交调用方直通，由客户端插入半角字符）。
///
/// 只关全角**不再是**这一档：2026-09-28 拆开关之前「关全角」= 连整张中文标点表一起关，
/// 本用例当时只 toggle 一次全角就够；现在表由 `chinese_punct` 单独管，只关全角时
/// `,` 照出 `，`（第 2 行，见 `chinese_punct_and_fullwidth_are_independent`）。
#[test]
fn chinese_halfwidth_passes_everything_through() {
    let mut e = engine();
    e.set_chinese_punct(false);
    e.toggle_fullwidth();
    assert!(!e.fullwidth() && !e.chinese_punct());
    // `'/'` 2026-10-15 进表（用户裁决「斜杠问号键在中文下，点击默认中文顿号」）——
    // 两位开关都关时它同样必须直通，别只在「表关」时漏了它。
    for c in [',', '.', '\\', '/', '[', '"', '\''] {
        assert_eq!(e.input_punct(c), None, "半角态 {c:?} 不该映射");
    }
}

/// **斜杠在中文标点档出顿号**（2026-10-15 用户裁决）。同时钉住「问号键」那一半：
/// `?` 一直就是 `？`（表里原有），两条都是移动端数字/标点层上的键。
#[test]
fn slash_maps_to_dunhao_and_question_mark_to_fullwidth() {
    let mut e = engine();
    assert!(e.chinese_punct(), "拼音默认开中文标点");
    assert_eq!(e.input_punct('/').as_deref(), Some("、"));
    assert_eq!(e.input_punct('?').as_deref(), Some("？"));
    // 关掉中文标点：两条都直通（全角那档不管它们 —— 表里没有 `/`/`?` 的全角兜底对照见
    // `chinese_punct_and_fullwidth_are_independent` 的四组合真值表）。
    e.set_chinese_punct(false);
    e.toggle_fullwidth();
    assert!(!e.fullwidth());
    assert_eq!(e.input_punct('/'), None);
    assert_eq!(e.input_punct('?'), None);
}

// ---------- 中文标点 / 全角：两个独立开关（用户裁决 2026-09-28） ----------

/// 把两个开关设到指定状态。`fullwidth` 只有触发键（[`Engine::toggle_fullwidth`]）、
/// 没有 setter —— 用它的返回值确认到位。
fn switches(e: &mut Engine, chinese_punct: bool, fullwidth: bool) {
    e.set_chinese_punct(chinese_punct);
    if e.fullwidth() != fullwidth {
        assert_eq!(e.toggle_fullwidth(), fullwidth, "toggle 返回切换后的状态");
    }
}

/// 真值表**逐格**实测（拼音模式：四组合 × 4 键，共 16 格）。
/// `chinese_punct` 管中文标点表那一档，`fullwidth` 管机械全角那一档，互不牵连。
///
/// 钉住的正是拆开关这件事本身：
/// - **第 2 行**（表开、全角关）：`，` `。` `、` 照出中文标点。拆开关前这一格是 `None`
///   （旧 `punct_text` 第一行 `if !self.fullwidth { return None; }`）—— 「关全角把整张
///   中文标点表一起关掉」正是要修的 bug，表外键 `^` 这时才是半角直通；
/// - **第 3 行**（表关、全角开）：`．` `＼` 是**机械全角**而不是中文标点 `。` `、`；
///   而 `, → ，` 与第 1 行**同字**（`，` 就是 U+FF0C 全角逗号）—— 结果同、路径不同，
///   所以这一格不能用 `,` 单独验，必须连 `.` `\` 一起。
#[test]
fn chinese_punct_and_fullwidth_are_independent() {
    const CASES: &[(bool, bool, [Option<&str>; 4])] = &[
        // (中文标点, 全角, 键 `,` `.` `\` `^` 的期望)
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
            let mut e = engine();
            switches(&mut e, cp, fw);
            assert_eq!(
                e.input_punct(k).as_deref(),
                want[i],
                "chinese_punct={cp} fullwidth={fw} 键 {k:?}"
            );
        }
    }
}

/// 西文（英文/数字）模式**没有中文标点表这一档**：`.` 在任何组合下都不是 `。`，
/// `chinese_punct` 在这里不参与判定，只有 `fullwidth` 的机械全角那一档。
/// 三模式各测一遍（拼音那两模式见上一条）。
#[test]
fn chinese_punct_has_no_effect_in_non_chinese_modes() {
    for mode in [Mode::English, Mode::Number] {
        for cp in [false, true] {
            for fw in [false, true] {
                let mut e = engine();
                e.switch_mode(mode);
                switches(&mut e, cp, fw);
                let why = format!("{mode:?} chinese_punct={cp} fullwidth={fw}");
                assert_eq!(
                    e.input_punct('.').as_deref(),
                    fw.then_some("．"),
                    "{why} 的 `.`"
                );
                assert_eq!(
                    e.input_punct(',').as_deref(),
                    fw.then_some("，"),
                    "{why} 的 `,`"
                );
                // 表里的中文专用字形在两种状态下都出不来：`\` 走机械全角是 `＼`
                assert_eq!(
                    e.input_punct('\\').as_deref(),
                    fw.then_some("＼"),
                    "{why} 的 `\\`"
                );
            }
        }
    }
}

/// `chinese_punct` 是**全局偏好、不随模式重置**（与 `fullwidth` 有意相反）：
/// 只有拼音/繁体读它，对它而言「关掉中文标点」不是某个模式的默认值 ——
/// 切到英文再切回来，用户关掉的表不该自己打开。默认 `true` = 保持既有行为。
#[test]
fn chinese_punct_survives_mode_switch_but_fullwidth_does_not() {
    let mut e = engine();
    assert!(e.chinese_punct(), "默认开");
    e.set_chinese_punct(false);
    for mode in [
        Mode::English,
        Mode::Number,
        Mode::Symbol,
        Mode::Traditional,
        Mode::Pinyin,
    ] {
        e.switch_mode(mode);
        assert!(!e.chinese_punct(), "{mode:?} 切模式不该重置这档偏好");
    }
    // 对照：同一次切换里全角被重置回模式默认 —— 两档的复位时机**有意不同**
    e.switch_mode(Mode::Pinyin);
    assert!(e.fullwidth(), "全角仍按模式默认重置");
    e.set_chinese_punct(true);
    assert!(e.chinese_punct(), "set 的读侧一致");
}

/// 触发键那一侧（[`Engine::toggle_chinese_punct`]，与 `set_chinese_punct` 同字段）：
/// 返回**切换后的新状态**、两个入口互不覆盖、且**经由触发键关掉的表也不随模式重置**。
///
/// 上一条只走了设置项那一侧 —— 触发键是**另一个入口**，同一个属性得单独钉一次
/// （入口多一个，能绕过它的路径就多一条）。
#[test]
fn toggle_chinese_punct_flips_returns_and_survives_mode_switch() {
    let mut e = engine();
    assert!(e.chinese_punct(), "默认开");
    assert!(!e.toggle_chinese_punct(), "toggle 返回切换后的状态");
    assert!(!e.chinese_punct(), "读侧与返回值一致");
    assert!(e.toggle_chinese_punct(), "再翻一次回来");
    assert!(e.chinese_punct());

    // 两个入口是同一个字段：谁后设谁生效，另一侧不残留
    e.set_chinese_punct(false);
    assert!(!e.chinese_punct());
    assert!(e.toggle_chinese_punct(), "从设置项设下的值继续翻转");
    assert!(e.chinese_punct());

    // 触发键关掉之后同样不随模式重置（与 `fullwidth` 的触发键**有意相反**）
    assert!(!e.toggle_chinese_punct());
    for mode in [Mode::English, Mode::Number, Mode::Symbol, Mode::Pinyin] {
        e.switch_mode(mode);
        assert!(!e.chinese_punct(), "{mode:?} 不该重置触发键设下的值");
    }

    // 对照：**同一次切模式**里，触发键那一侧的全角确实被重置了 ——
    // 没有这条，上面那串断言可能只是因为「切模式什么都没做」而恒真
    e.switch_mode(Mode::Pinyin);
    assert!(e.fullwidth(), "拼音默认全角");
    assert!(!e.toggle_fullwidth(), "触发键关掉全角");
    e.switch_mode(Mode::Pinyin);
    assert!(
        e.fullwidth(),
        "fullwidth 被重置回默认 —— 与 chinese_punct 的对照成立"
    );
}

/// 成对引号**属于中文标点表那一档**（`"` `'` 就在 `CHINESE_PUNCT` 里）：表关掉后
/// `"` 落到机械全角 `＂`、不交替；表开着交替照旧。
/// 表关期间**不碰交替状态**（那条路根本没走到 [`Quotes`]）—— 再打开时仍从头出左引号。
#[test]
fn quote_alternation_is_gated_by_chinese_punct() {
    let mut e = engine();
    e.set_chinese_punct(false);
    assert_eq!(e.input_punct('"').as_deref(), Some("＂"));
    assert_eq!(e.input_punct('"').as_deref(), Some("＂"), "机械全角不交替");
    e.set_chinese_punct(true);
    assert_eq!(
        e.input_punct('"').as_deref(),
        Some("“"),
        "表回来：交替状态没被机械全角那条路动过"
    );
    assert_eq!(e.input_punct('"').as_deref(), Some("”"));
}

/// 英文/数字模式的全角是**机械全角**：`.` 得 `．`(U+FF0E)，不是中文句号 `。`
/// —— 西文文本里冒出一个中文句号是错的。
#[test]
fn non_chinese_fullwidth_is_mechanical_not_cjk() {
    let mut e = engine();
    e.switch_mode(Mode::English);
    assert_eq!(e.input_punct('.'), None, "英文默认半角：不映射");
    e.toggle_fullwidth();
    assert_eq!(e.input_punct('.').as_deref(), Some("．"));
    assert_eq!(e.input_punct('"').as_deref(), Some("＂"), "西文引号不交替");
    assert_eq!(e.input_punct('"').as_deref(), Some("＂"), "第二次还是它");
    // **标点档**不管字母/数字（`ascii_fullwidth` 的范围钉）：那些字符走模式分派 ——
    // 英文/数字模式在宿主**直传点**宽化（`fullwidthize`，见下一批用例），
    // 拼音模式进缓冲当输入码、绝不宽。
    assert_eq!(e.input_punct('a'), None);
    assert_eq!(e.input_punct('1'), None);
}

// ---------- 生效值（**显示面**；2026-10-15 用户裁决「切换中英文的时候，标点符号也联动切换」） ----------
//
// `effective_chinese_punct` = 「此刻按标点键会不会出中文标点」。**行为本来就对**
// （`punct_text` 一按模式分派），这一族是给**状态栏那一格**用的读侧：原先宿主读的是
// 配置值 `ChinesePunct`（不随模式变），切英文后仍报 on、按钮还画「。，」。

/// 中文类模式（拼音/繁体）：就是设置值本身 —— 设置项与触发键两个入口都跟。
#[test]
fn effective_chinese_punct_follows_setting_in_chinese_modes() {
    for mode in [Mode::Pinyin, Mode::Traditional] {
        let mut e = engine();
        e.switch_mode(mode);
        assert!(e.effective_chinese_punct(), "{mode:?} 默认开");
        e.set_chinese_punct(false);
        assert!(!e.effective_chinese_punct(), "{mode:?} 跟设置项");
        e.toggle_chinese_punct();
        assert!(e.effective_chinese_punct(), "{mode:?} 跟触发键（同一字段）");
    }
}

/// **非中文模式恒 false**（英文/数字/符号）：那里没有中文标点表这一档，配置值再开
/// 也不出中文标点 —— 状态栏因此切英文就翻成「.,」。也是「配置值不能当显示值」的实证。
#[test]
fn effective_chinese_punct_is_false_in_non_chinese_modes() {
    for mode in [Mode::English, Mode::Number, Mode::Symbol] {
        for cp in [false, true] {
            let mut e = engine();
            e.switch_mode(mode);
            e.set_chinese_punct(cp);
            assert!(
                !e.effective_chinese_punct(),
                "{mode:?} chinese_punct={cp} —— 非中文模式恒 false"
            );
        }
    }
}

/// **与 `punct_text` 的模式分派逐字一致**（机械核对，不是复述那个布尔值）：
/// `\` 只有中文标点表给 `、`（机械全角给的是 `＼`），故「`\` 出不出中文顿号」就是
/// 「中文标点表这一档在不在」——逐模式 × 逐设置 × 逐全角档全跑一遍，两处判据必须同值
/// （将来加了新模式，这格会红而不是静默漂）。
#[test]
fn effective_chinese_punct_matches_punct_text_dispatch() {
    let modes = [
        Mode::Pinyin,
        Mode::Traditional,
        Mode::English,
        Mode::Number,
        Mode::Symbol,
    ];
    for mode in modes {
        for cp in [false, true] {
            for fw in [false, true] {
                let mut e = engine();
                e.switch_mode(mode);
                switches(&mut e, cp, fw);
                let table_gives_cjk = e.input_punct('\\').as_deref() == Some("、");
                assert_eq!(
                    e.effective_chinese_punct(),
                    table_gives_cjk,
                    "{mode:?} chinese_punct={cp} fullwidth={fw} 的生效值与 punct_text 分派不一致"
                );
            }
        }
    }
}

// ---------- 全角宽档（字母/数字/空格；用户裁决 2026-10-12「字母、数字也要」） ----------

/// `fullwidthize` —— 宿主**直传点**的唯一入口（英文/数字模式下字母数字是待上屏文本，
/// 但各端都是直传、不进引擎，所以规则放这里、由宿主在直传点调用）：
/// ASCII 可打印平移 +0xFEE0；**空格不宽化**（有意，见该函数头注）；开关关/非 ASCII 恒等。
#[test]
fn fullwidthize_widens_text_for_direct_passthrough_points() {
    let mut e = engine();
    e.switch_mode(Mode::English);
    assert_eq!(e.fullwidthize("aA1. "), "aA1. ", "开关关 ⇒ 恒等");
    e.toggle_fullwidth();
    assert_eq!(e.fullwidthize("aA1. "), "ａＡ１． ", "空格保持半角");
    assert_eq!(
        e.fullwidthize("你好"),
        "你好",
        "非 ASCII 恒等（汉字天然双宽）"
    );
}

/// **拼音模式绝不宽字母/数字**（回归钉）：它们是**输入码**不是文本 —— 宽了会把
/// 「打 a」变成「上屏 ａ」，拼音打字整条断（`punctuation::ascii_fullwidth` 头注）。
#[test]
fn pinyin_composition_letters_are_never_widened() {
    let mut e = engine();
    assert!(e.fullwidth(), "前置：拼音默认全角");
    assert_eq!(e.input_key('n'), "", "字母进缓冲，不上屏");
    assert_eq!(e.buffer(), "n", "缓冲里是半角输入码");
    assert_eq!(e.input_key('i'), "");
    assert_eq!(e.buffer(), "ni");
}

/// 英文模式的**引擎内**提交路径也宽化：空格提交（无候选时的原始缓冲）走
/// `fullwidthize` —— 与两轨直传点同一条规则（`input_space` 的英文分支）。
#[test]
fn english_mode_buffer_commit_widens_too() {
    let mut e = engine();
    e.switch_mode(Mode::English);
    e.toggle_fullwidth();
    // 无前缀词的缓冲：走「英文打原文」的逃生口（有候选时提首候选，另有用例）。
    for c in ['h', 'i', 'q'] {
        e.input_key(c);
    }
    assert_eq!(e.input_space(), "ｈｉｑ", "空格提交原始缓冲：全角");
    assert_eq!(e.buffer(), "", "提交后缓冲清空");
}
