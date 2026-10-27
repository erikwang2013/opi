// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 输入方案（9 键 / 14 键 / 笔画，2026-10-09）的端到端行为门禁。
//!
//! 判据与全键盘各层同源：**按键类前缀匹配**的结果集 + 「精确加成 ≥ 任何前缀
//! 扩展」那条既有不变式在方案路径下同样成立（打分规则逐字复用）。
use engine_core::symbols::SymbolEngine;
use engine_core::{Dictionary, Engine, InMemoryDictionary, Mode, Scheme};

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

/// 2026-10-27：整串是「≥3 个完整音节 + 更长词」的前缀时，**首字回退不许顶掉它**
/// （26 键 `segment_fallback` 判据 3b 的镜像）。用户报障：`shengrikuai` 在 26 键给
/// 「生日快乐」，9/14 键只剩「生」。
#[test]
fn nine_and_fourteen_keep_a_longer_prefix_word() {
    let entries = [("sheng", "生", 900), ("shengrikuaile", "生日快乐", 950)];
    let mut e = Engine::new(dict(&entries), no_symbols(), false);
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "74364745824"); // shengrikuai
    assert_eq!(texts(&e)[0], "生日快乐", "9 键：长词前缀不该被首字回退替换");

    let mut f = Engine::new(dict(&entries), no_symbols(), false);
    f.set_scheme(Scheme::Fourteen);
    type_str(&mut f, "agebgeujuau"); // s h e n g r i k u a i
    assert_eq!(
        texts(&f)[0],
        "生日快乐",
        "14 键同一条：长词前缀不该被首字回退替换"
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

// ---- 分词键（`'`，2026-10-09 主流式「就在这里断」）----

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

/// 左列复位（2026-10-09 用户裁决「不输入、回退、确定后选词、删除完后，左侧恢复
/// 标点一列」）：**只要缓冲回到空，读数就必须为空** —— 界面据此把左列换回标点。
/// （读数由 `display_buffer` 给，界面直接拿它当「显示什么」的判据。）
#[test]
fn reading_resets_to_empty_when_the_buffer_empties() {
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64"); // ni
    let _ = e.candidates(8);
    assert_eq!(e.display_buffer(), "ni", "前置：读得到拼音");

    // 确定后选词（选中第一条 = 引擎的提交收尾）
    let _ = e.select_from(&e.candidates(8), 0);
    assert_eq!(e.buffer(), "", "前置：选词后缓冲已清");
    let _ = e.candidates(8); // 界面每次操作后都会重取候选（refresh 那条路）
    assert_eq!(e.display_buffer(), "", "选词后读数必须归零（左列回标点）");

    // 回退删完
    type_str(&mut e, "64");
    let _ = e.candidates(8);
    assert_eq!(e.display_buffer(), "ni");
    e.backspace();
    e.backspace();
    let _ = e.candidates(8);
    assert_eq!(e.buffer(), "");
    assert_eq!(e.display_buffer(), "", "退空后读数必须归零");
}

/// 语言切换**保留缓冲**（2026-10-09 用户裁决「中文输入的时候切换英文，拼音保留
/// 可继续使用」）：中→英→中 往返后，打到一半的拼音（含 9 键的数字串）还在、
/// 候选按同一缓冲重新算出、读数也回来。**对照组**：方案切换（用户可见的「输入模式」
/// 面板那条 `set_scheme`）照旧清缓冲 —— 两条路判据不同，别顺手统一。
#[test]
fn switch_language_preserves_the_pending_buffer() {
    let mut e = engine();
    e.set_scheme(Scheme::Nine);
    type_str(&mut e, "64"); // ni
    let before = e.candidates(8);
    assert!(!before.is_empty(), "前置：64 有候选");

    // 中→英（语言切换：英文模式 + 全键盘方案 —— 与 Android 中/英键同一对参数）
    e.switch_language(Mode::English, Scheme::Full);
    assert_eq!(e.mode(), Mode::English);
    assert_eq!(e.buffer(), "64", "切英文后缓冲必须保留");

    // 英→中（换回：拼音 + 9 键方案）
    e.switch_language(Mode::Pinyin, Scheme::Nine);
    assert_eq!(e.buffer(), "64", "切回中文后缓冲仍在（可继续使用）");
    assert_eq!(
        e.candidates(8).len(),
        before.len(),
        "切回后候选按同一缓冲重新算出"
    );
    assert_eq!(
        e.display_buffer(),
        "ni",
        "读数也回来了（缓存随切换作废、取候选时重算）"
    );

    // 对照组：方案切换仍清缓冲（「输入模式」面板走这条，切换前调用方已 commit）
    e.set_scheme(Scheme::Full);
    assert_eq!(e.buffer(), "", "set_scheme 照旧清缓冲（两条路判据不同）");
}
