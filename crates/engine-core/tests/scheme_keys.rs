// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 输入方案（9 键 / 14 键 / 笔画，2026-10-09）的端到端行为门禁。
//!
//! 判据与全键盘各层同源：**按键类前缀匹配**的结果集 + 「精确加成 ≥ 任何前缀
//! 扩展」那条既有不变式在方案路径下同样成立（打分规则逐字复用）。
use engine_core::symbols::SymbolEngine;
use engine_core::{Dictionary, Engine, InMemoryDictionary, Scheme};

fn dict(entries: &[(&str, &str, u32)]) -> Box<dyn Dictionary> {
    let mut d = InMemoryDictionary::new();
    for (py, w, f) in entries {
        d.insert(py, w, *f);
    }
    Box::new(d)
}

/// 符号表用空表：本文件测的是按键类匹配，符号数据不该参与断言。
fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

fn engine() -> Engine {
    Engine::new(
        dict(&[
            ("ni", "你", 5000),
            ("ni", "尼", 900),
            ("nihao", "你好", 6000),
            ("li", "里", 4000),
            ("hao", "好", 7000),
            ("wang", "王", 3000),
        ]),
        no_symbols(),
        false,
    )
}

fn type_str(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

fn texts(e: &Engine) -> Vec<String> {
    e.candidates(8).into_iter().map(|c| c.text).collect()
}

#[test]
fn nine_key_expands_key_classes() {
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64"); // 6∈{m,n,o} 4∈{g,h,i} ⇒ ni 命中
    let got = texts(&e);
    assert!(
        got.contains(&"你".to_string()),
        "64 应命中 ni⇒你，实际 {got:?}"
    );
    assert!(
        !got.contains(&"里".to_string()),
        "li 的第一个字母 l 不在 6 的类里，不该命中：{got:?}"
    );
}

#[test]
fn nine_key_prefix_matches_longer_keys() {
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64426"); // nihao
    assert_eq!(texts(&e)[0], "你好", "64426 应命中 nihao⇒你好");
    // 前缀语义：更短的输入也应命中更长的键
    let mut e2 = engine();
    e2.set_scheme(Scheme::Nine);
    type_str(&mut e2, "64");
    assert!(
        texts(&e2).contains(&"你好".to_string()),
        "64 前缀应命中 nihao"
    );
}

#[test]
fn nine_key_digits_enter_buffer_full_scheme_ignores_them() {
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64");
    assert_eq!(e.buffer(), "64", "9 键方案下数字进缓冲");
    // 0/1 不是 9 键的输入键
    e.input_key('0');
    assert_eq!(e.buffer(), "64", "0 不进缓冲");
    // 对照组：全键盘方案下数字照旧**不进**缓冲（行为一字不动）
    let mut full = engine();
    full.input_key('6');
    assert_eq!(full.buffer(), "", "全键盘下数字不该进拼音缓冲");
}

#[test]
fn fourteen_key_expands_letter_pairs() {
    let mut e = engine();
    e.set_scheme(Scheme::Fourteen);
    type_str(&mut e, "bu"); // b∈{b,n} u∈{u,i} ⇒ ni 命中
    let got = texts(&e);
    assert!(
        got.contains(&"你".to_string()),
        "14 键 bu 应命中 ni⇒你，实际 {got:?}"
    );
    // q∈{q,w}：q 应命中 wang（自己的类里的字母）
    let mut e2 = engine();
    e2.set_scheme(Scheme::Fourteen);
    type_str(&mut e2, "q");
    assert!(
        texts(&e2).contains(&"王".to_string()),
        "14 键 q 应命中 wang⇒王（q 键类含 w）"
    );
}

#[test]
fn scheme_exact_match_outranks_prefix_even_with_lower_freq() {
    // 与全键盘同一条不变式：精确（键长相等）> 任何前缀扩展。
    // ni（5000，精确）应压过 nihao（6000，前缀）。
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64");
    assert_eq!(texts(&e)[0], "你", "精确的 ni 应排在更长的 nihao 前");
}

#[test]
fn set_scheme_clears_buffer() {
    let mut e = engine();
    type_str(&mut e, "ni");
    assert_eq!(e.buffer(), "ni");
    e.set_scheme(Scheme::Nine);
    assert_eq!(
        e.buffer(),
        "",
        "换方案必须清缓冲（旧方案的串在新方案下无意义）"
    );
}

#[test]
fn stroke_scheme_queries_stroke_dict() {
    let mut e = engine();
    // 笔画词典：键是笔画串（h 横 s 竖 p 撇 n 捺 z 折），词是单字。
    // 主词典里**故意**没有这些键，用来分辨「换了词典」与「还是主词典」。
    e.set_stroke_dict(Some(dict(&[("hs", "十", 5000), ("hsz", "土", 4000)])));
    e.set_scheme(Scheme::Stroke);
    type_str(&mut e, "hs");
    assert_eq!(
        texts(&e)[0],
        "十",
        "笔画方案应查 stroke 词典（主词典里没有 hs）"
    );
    // 前缀语义：hs 也命中 hsz
    assert!(texts(&e).contains(&"土".to_string()));
}

#[test]
fn stroke_scheme_without_dict_is_empty_not_error() {
    let mut e = engine();
    e.set_scheme(Scheme::Stroke);
    type_str(&mut e, "hs");
    assert!(
        texts(&e).is_empty(),
        "没装笔画词典 = 候选取自普通词典（此例为空），不该报错"
    );
}

#[test]
fn full_scheme_behavior_unchanged() {
    // 回归守：默认方案下一切照旧（普通前缀查询 + 数字不入缓冲）。
    let mut e = engine();
    type_str(&mut e, "ni");
    assert_eq!(texts(&e)[0], "你");
    assert_eq!(e.scheme(), Scheme::Full);
}

#[test]
fn nine_scheme_digits_enter_buffer_through_router() {
    // 路由器层（Android/Apple 走的就是它）：9 键方案下数字必须进缓冲 ——
    // **有候选时也不许**被 digit_select 当页内选词吃掉（那正是首版的门没开在
    // 选词判定之前的形态：缓冲一有候选，9 键就再也打不进第二个数字）。
    let mut r = engine_core::KeyRouter::new(engine());
    r.engine_mut().set_scheme(Scheme::Nine);
    for c in ['6', '4'] {
        r.key_event(c as u32, 0);
    }
    assert_eq!(r.buffer(), "64", "9 键方案：数字经路由器进缓冲");
    assert!(
        r.candidates().contains(&"你".to_string()),
        "并且真的出候选（此时缓冲非空、有候选可被选）"
    );
    r.key_event('7' as u32, 0);
    assert_eq!(r.buffer(), "647", "有候选时数字仍是输入键（没被选词吃掉）");
    // 0/1 不是 9 键的输入键：直通（不进缓冲、也不选词）
    r.key_event('1' as u32, 0);
    assert_eq!(r.buffer(), "647", "0/1 既不进缓冲也不选词");
}

#[test]
fn full_scheme_digits_still_select_through_router() {
    // 对照组：默认方案下数字仍是选词键（行为一字不动）。
    let mut r = engine_core::KeyRouter::new(engine());
    for c in ['n', 'i'] {
        r.key_event(c as u32, 0);
    }
    assert_eq!(r.buffer(), "ni");
    let action = r.key_event('1' as u32, 0);
    assert!(
        matches!(action, engine_core::KeyAction::Input(_)),
        "全键盘：数字选词应产出提交，实际 {action:?}"
    );
    assert_eq!(r.buffer(), "", "选词后缓冲清空");
}

// ---- 显示用缓冲（2026-10-09：9 键左侧要显示「拼音读数」）----

/// 9 键的引擎缓冲是数字串；**显示层**要的是首条候选的拼音（搜狗 9 键候选栏左侧
/// 那格「输入时显示的拼音」）。FFI 的 buffer 出口读 `display_buffer`。
#[test]
fn nine_key_display_buffer_is_the_top_candidate_pinyin() {
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64"); // ni
    let _ = e.candidates(8); // 取候选顺带刷新读数（宿主每键都这么走）
    assert_eq!(e.buffer(), "64", "引擎缓冲本体仍是数字串（路由/退格靠它）");
    assert_eq!(e.display_buffer(), "ni", "读数 = 首条候选「你」的拼音");

    type_str(&mut e, "426"); // nihao
    let _ = e.candidates(8);
    assert_eq!(e.buffer(), "64426");
    // 分数上 你好(6000+精确) 压过 你(5000+?)：读数跟着首条走
    assert_eq!(
        e.display_buffer(),
        "ni'hao",
        "读数带分词（搜狗「显示分词」同款）"
    );

    // 没有候选（乱按）：读数为空 —— 不把数字串端上去（用户裁决的本意）
    let mut e2 = engine();
    e2.set_scheme(Scheme::Nine);
    type_str(&mut e2, "999");
    let _ = e2.candidates(8);
    assert_eq!(e2.display_buffer(), "");
}

/// 换方案把读数一起清掉（否则换方案后的一次读拿到上一条输入的拼音）。
#[test]
fn set_scheme_clears_the_display_reading() {
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64");
    let _ = e.candidates(8);
    assert_eq!(e.display_buffer(), "ni");
    e.set_scheme(Scheme::Nine); // 同值重设也清（等于「重新开始」）
    assert_eq!(e.display_buffer(), "", "换方案后读数必须为空");
}

/// 非 9 键方案：显示缓冲 = 实际缓冲，逐字节相同（26/14 键的字母串本来就是拼音，
/// 别的端（fcitx5/TSF）读到的必须与改动前一致）。
#[test]
fn display_buffer_is_raw_outside_nine() {
    let mut e = engine();
    type_str(&mut e, "ni");
    let _ = e.candidates(8);
    assert_eq!(e.display_buffer(), "ni");
    assert_eq!(e.buffer(), "ni");

    let mut e14 = engine();
    e14.set_scheme(Scheme::Fourteen);
    type_str(&mut e14, "n"); // 14 键发的是字母对首字母，缓冲仍是字母串
    let _ = e14.candidates(8);
    assert_eq!(e14.display_buffer(), e14.buffer());
}

// ---- 分词键（`'`，2026-10-09 搜狗式「就在这里断」）----

/// 分词把候选**约束到音节边界**上：`64'26` = ni|an ⇒ 命中 nian、不命中 nihao；
/// 不带分词时两者都能前缀匹配；断点不对齐（`644'26`，断在 nih 后）谁都不匹配。
#[test]
fn segment_key_forces_the_syllable_boundary() {
    let mut e = Engine::new(
        dict(&[
            ("nian", "年", 6000),
            ("nihao", "你好", 7000),
            ("ni", "你", 5000),
        ]),
        no_symbols(),
        false,
    );
    e.set_scheme(Scheme::Nine);

    // 基线（数字串别写错）：6426 = nian、64426 = nihao，各命中各的
    type_str(&mut e, "6426");
    let nian_only = texts(&e);
    assert!(
        nian_only.contains(&"年".to_string()),
        "6426 应能命中 nian：{nian_only:?}"
    );
    assert!(
        !nian_only.contains(&"你好".to_string()),
        "6426 不该命中 nihao（那是 64426）：{nian_only:?}"
    );

    // 分词：64'26 = ni|an —— an 与 hao 不同键位，nihao 出局
    let mut e2 = Engine::new(
        dict(&[
            ("nian", "年", 6000),
            ("nihao", "你好", 7000),
            ("ni", "你", 5000),
        ]),
        no_symbols(),
        false,
    );
    e2.set_scheme(Scheme::Nine);
    type_str(&mut e2, "64'26");
    let seg = texts(&e2);
    assert!(
        seg.contains(&"年".to_string()),
        "64'26 应命中 nian（ni|an）：{seg:?}"
    );
    assert!(
        !seg.contains(&"你好".to_string()),
        "64'26 不得命中 nihao：{seg:?}"
    );

    // 96(=wang) 不掺和；加课：64'426 = ni|hao → 命中 nihao
    let mut e4 = Engine::new(
        dict(&[("nian", "年", 6000), ("nihao", "你好", 7000)]),
        no_symbols(),
        false,
    );
    e4.set_scheme(Scheme::Nine);
    type_str(&mut e4, "64'426");
    assert!(
        texts(&e4).contains(&"你好".to_string()),
        "64'426 = ni|hao 应命中 nihao：{:?}",
        texts(&e4)
    );

    // 断点不对齐（nih 不是音节边界）：无人匹配
    let mut e3 = Engine::new(
        dict(&[("nian", "年", 6000), ("nihao", "你好", 7000)]),
        no_symbols(),
        false,
    );
    e3.set_scheme(Scheme::Nine);
    type_str(&mut e3, "644'26");
    assert!(
        texts(&e3).is_empty(),
        "断在 nih 后的分词不该匹配任何词：{:?}",
        texts(&e3)
    );

    // `'` 确实进了缓冲（显示/退格都基于它），且不是标点引号
    assert!(
        e2.buffer().contains('\''),
        "分词符应在缓冲里：{:?}",
        e2.buffer()
    );
}

/// 单段（只有分词符在末尾/开头）与连打等价性：`64'` 应当与 `64` 一样出候选
/// —— 末段可为前缀，分词符在末尾只表示「边界在这里（后面还没打）」。
#[test]
fn trailing_segment_marker_keeps_prefix_semantics() {
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64'");
    assert!(
        texts(&e).contains(&"你".to_string()),
        "64' 应与 64 同义（末段可为空续打）：{:?}",
        texts(&e)
    );
}

// ---- 缓冲到顶：静默但可被看见（2026-10-09）----

/// 到顶（MAX_BUFFER）后：缓冲**原样留着**、后续键不提交 —— 这是实测过的裁决
/// （自动提交曾把用户打到一半的拼音整段冲上屏，回归在 tests/punctuation.rs）。
/// 2026-10-09 把上限 16 提到 32（「不停输入」摸得到 16）；「看得见」由 UI 负责
/// （9 键左列显示缓冲原样）。本用例按符号化写法钉住「满 = 静默」，改上限不用动。
#[test]
fn buffer_cap_stays_silent_and_holds() {
    let mut e = Engine::new(dict(&[("ni", "你", 5000)]), no_symbols(), false);
    for _ in 0..engine_core::composer::MAX_BUFFER {
        e.input_key('n');
    }
    let full = e.buffer().to_string();
    assert_eq!(full.chars().count(), engine_core::composer::MAX_BUFFER);
    assert_eq!(e.input_key('b'), "", "到顶不提交");
    assert_eq!(e.buffer(), full, "到顶缓冲原样留着（绝不 flush）");
}
