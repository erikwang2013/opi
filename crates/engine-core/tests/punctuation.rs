// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 引擎层标点表 + 全角/半角开关（用户裁决 2026-09-27：「引擎层加标点表，四端走同一条路」）。
//!
//! 契约：
//! - 中文模式（拼音/繁体）里 ASCII 标点出**中文标点**（`,` → `，`、`.` → `。`、`\` → `、`），
//!   表里没有的 ASCII 标点机械全角兜底（`/` → `／`）；
//! - 英文/数字模式默认**半角直通**（`input_punct` 返回 `None`）；全角开关打开后出
//!   **机械全角**（`.` → `．` U+FF0E，不是中文句号 `。`）；
//! - 同键成对的引号交替出左右引号，**切模式**时复位；
//! - 撇号在拼音/繁体里是**音节分隔符**（`xi'an`），只有缓冲为空时才当引号；
//! - 标点落在待提交的拼音**后面**（先走与空格同一条收尾）；
//! - `input_key` **不吞可见字符**：没进缓冲也没映射的，原样交回调用方
//!   —— Android 数字面板（旧路径是 UI 直接 commit，绕开引擎）靠这条活在引擎里。
//!
//! 本文件走**公开 API**（`Engine` + `KeyRouter`）；两轨的镜像（14 条同名同序，彼此逐行同构）
//! 在 crates/fcitx5-opi/src/input_method_punct_tests.rs 与
//! crates/tsf-opi/src/logic_punct_tests.rs；与本文件共享 12 条，另有 4 条只属于这一层
//! （两条路由用例，加两条不经路由的：数据门与「无客户端可交」的直调用例）。

use engine_core::dictionary::InMemoryDictionary;
use engine_core::keys::{KEY_PAGE_DOWN, KEY_STATE_RELEASED};
use engine_core::router::{KeyAction, KeyRouter};
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, Mode};

/// 词库只放 `ni → 你`：够钉「标点排在待提交的拼音后面」，别的都不掺。
fn engine() -> Engine {
    let mut d = InMemoryDictionary::new();
    d.insert("ni", "你", 100);
    Engine::new(Box::new(d), SymbolEngine::builtin(), true)
}

fn typed(e: &mut Engine, s: &str) {
    for c in s.chars() {
        e.input_key(c);
    }
}

// ---------- 中文模式：老问的那张表，一行一键 ----------

/// 中文模式（拼音/繁体）逐键映射。前半是**中文标点表**，后半是**机械全角兜底**
/// （这些键没有中文专用字形，但不是「直通」——中文模式就是全角模式）。
#[test]
fn chinese_modes_map_every_punctuation_key() {
    const CASES: &[(char, &str)] = &[
        (',', "，"),
        ('.', "。"),
        ('?', "？"),
        ('!', "！"),
        (':', "："),
        (';', "；"),
        ('\\', "、"),
        ('(', "（"),
        (')', "）"),
        ('[', "【"),
        (']', "】"),
        ('<', "《"),
        ('>', "》"),
        ('~', "～"),
        // 机械全角兜底：0x21..=0x7E 平移 +0xFEE0（Unicode 全角区的定义）
        ('/', "／"),
        ('@', "＠"),
        ('#', "＃"),
        ('$', "＄"),
        ('%', "％"),
        ('^', "＾"),
        ('&', "＆"),
        ('*', "＊"),
        ('-', "－"),
        ('_', "＿"),
        ('+', "＋"),
        ('=', "＝"),
        ('|', "｜"),
        ('{', "｛"),
        ('}', "｝"),
        ('`', "｀"),
    ];
    for mode in [Mode::Pinyin, Mode::Traditional] {
        for &(input, want) in CASES {
            let mut e = engine();
            e.switch_mode(mode);
            assert_eq!(
                e.input_punct(input).as_deref(),
                Some(want),
                "{mode:?} 下 {input:?} 的映射"
            );
        }
    }
}

/// 成对引号交替。`"` 与 `'` 各有一份独立状态（共用一份会出 `”‘` 这种半对）。
#[test]
fn paired_quotes_alternate() {
    let mut e = engine();
    assert_eq!(e.input_punct('"').as_deref(), Some("“"));
    assert_eq!(e.input_punct('"').as_deref(), Some("”"));
    assert_eq!(e.input_punct('"').as_deref(), Some("“"));
    assert_eq!(e.input_punct('\'').as_deref(), Some("‘"), "单引号另有状态");
    assert_eq!(e.input_punct('\'').as_deref(), Some("’"));
}

/// **复位时机 = 切模式**（用户要的「复位时机」，这是裁决）：
/// 不随提交/清缓冲复位 —— 引号交替是**排版状态**，跟着正在写的这段文本走：
/// 提交一个词再打右引号是常态，而缓冲在每次选词后都会清空，跟着缓冲复位等于
/// 右引号永远打不出来。全角开关同样不复位（它只管宽度，不管打过什么）。
#[test]
fn quote_alternation_resets_on_mode_switch_only() {
    let mut e = engine();
    assert_eq!(e.input_punct('"').as_deref(), Some("“"));
    // 选词提交（连缓冲一起清）后：状态保持 → 下一对是右引号
    typed(&mut e, "ni");
    assert_eq!(e.input_key(' '), "你");
    assert_eq!(e.input_punct('"').as_deref(), Some("”"), "提交不复位");
    // 全角开关往返回一次：还是右引号
    e.toggle_fullwidth();
    e.toggle_fullwidth();
    assert_eq!(
        e.input_punct('"').as_deref(),
        Some("“"),
        "半个来回后是下一对"
    );
    // 切模式复位：新的一段文本从头开引号
    let mut e = engine();
    assert_eq!(e.input_punct('"').as_deref(), Some("“"));
    e.switch_mode(Mode::Traditional);
    assert_eq!(e.input_punct('"').as_deref(), Some("“"), "切模式复位");
}

/// 撇号在拼音/繁体是**音节分隔符**：缓冲非空时不进标点表（`xi'an` 打得出来）。
/// 缓冲空时它没有分隔语义，这时才当引号 —— 两个需求都不丢。
#[test]
fn apostrophe_is_separator_while_composing() {
    let mut e = engine();
    typed(&mut e, "xi");
    assert_eq!(
        e.input_punct('\''),
        None,
        "缓冲非空：撇号是分隔符，不归标点表管"
    );
    assert_eq!(e.input_key('\''), "", "入缓冲，无提交文本");
    assert_eq!(e.buffer(), "xi'");
    e.clear();
    assert_eq!(e.input_punct('\'').as_deref(), Some("‘"), "缓冲空：当引号");
}

/// 标点必须落在待提交的拼音**后面**：先走与空格同一条收尾（有候选提首候选、
/// 无候选提原文），否则 `ni,` 上屏成 `，你` —— 顺序反了。
#[test]
fn punctuation_flushes_pending_buffer_first() {
    let mut e = engine();
    typed(&mut e, "ni");
    assert_eq!(e.input_punct(',').as_deref(), Some("你，"));
    assert_eq!(e.buffer(), "");
    // 乱码缓冲无候选：与空格同一条逃生口，原文上屏
    // （别拿 "zzz" 当乱码：它是 😴 的关键字）
    let mut e = engine();
    typed(&mut e, "qqq");
    assert_eq!(e.input_punct(',').as_deref(), Some("qqq，"));
}

/// flush 是**路由**的页码事件：`reset_page_if_buffer_changed` 是「缓冲变了就回第一页」
/// 的唯一实现处，标点这条新路径必须走它 —— 否则翻到第 3 页打个逗号，下一页停在第 3 页
/// 而候选早已换了一批（与空格提交后翻页同一个坑）。
/// 单独钉**那一行**：只把 `self.reset_page_if_buffer_changed()` 从守卫里删掉，本文件其余
/// 用例与 `router_invariants` 的性质测试都还是绿的（实测），只有这一条会红。
#[test]
fn punctuation_flush_resets_page_to_first() {
    let mut d = InMemoryDictionary::new();
    for i in 0..20 {
        d.insert("hao", &format!("词{i:02}"), (5000 - i) as u32);
    }
    let mut r = KeyRouter::new(Engine::new(Box::new(d), SymbolEngine::builtin(), true));
    r.switch_mode(Mode::Pinyin);
    for c in ['h', 'a', 'o'] {
        r.key_event(c as u32, 0);
    }
    r.key_event(KEY_PAGE_DOWN, 0);
    assert_eq!(r.page(), 1, "前置：20 条候选有 3 页");
    assert_eq!(
        r.key_event(',' as u32, 0),
        KeyAction::Input("词00，".into())
    );
    assert_eq!(r.page(), 0, "缓冲被 flush 上屏 → 页码归零");
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

/// 中文模式关掉全角 → 一个都不映射（交调用方直通，由客户端插入半角字符）。
#[test]
fn chinese_halfwidth_passes_everything_through() {
    let mut e = engine();
    e.toggle_fullwidth();
    assert!(!e.fullwidth());
    for c in [',', '.', '\\', '[', '"', '\''] {
        assert_eq!(e.input_punct(c), None, "半角态 {c:?} 不该映射");
    }
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
    // 字母/数字不进全角：全角字母表是另一件事，本表只管标点
    assert_eq!(e.input_punct('a'), None);
    assert_eq!(e.input_punct('1'), None);
}

/// 符号模式：不映射（`,` `.` 在关键字里根本不存在，见下一条），全角开关也不生效。
#[test]
fn symbol_mode_has_no_punctuation_mapping() {
    let mut e = engine();
    e.switch_mode(Mode::Symbol);
    assert_eq!(e.input_punct(','), None);
    e.toggle_fullwidth();
    assert_eq!(e.input_punct(','), None, "全角开关在符号模式不生效");
}

/// 符号关键字**全是** `^[a-z0-9]+$`（生产表 583 条实测）—— 标点永远搜不出候选，
/// 所以它在符号模式里既不是搜索输入、也不是关键字：只能原样交回去。
/// 这是「符号模式标点直通」这条裁决的数据依据；表一旦出现别的字符，这条先红。
#[test]
fn every_symbol_keyword_is_alnum() {
    let sym = SymbolEngine::builtin();
    for entry in sym.search("") {
        for kw in &entry.keywords {
            assert!(
                kw.chars().all(|c| c.is_ascii_alphanumeric()),
                "关键字 {kw:?}（{:?}）含非 [a-z0-9] 字符 —— 标点直通的裁决要重新审",
                entry.text
            );
        }
    }
}

// ---------- `input_key` 不吞可见字符 ----------

/// 不变式：**ASCII 标点**进 `input_key` 后不会无声消失 —— 要么被标点表映射，要么原样交回。
/// 这一条把「死键」整类消灭：Android 数字面板的 `,` `.` 与其它端「直通给客户端」
/// 是同一件事的两种落法（Android 没有客户端可交，IME 自己就是输入源）。
///
/// 边界画在标点上而不是全部可见 ASCII：字母/数字各有去处（进缓冲或这个模式不收），
/// 且**不能靠「交回」来救** —— 缓冲满时 composer 也返回 `Ignored`，见 `Engine::input_key`。
#[test]
fn ascii_punctuation_never_vanishes() {
    let mut e = engine();
    for mode in [
        Mode::Pinyin,
        Mode::Traditional,
        Mode::English,
        Mode::Number,
        Mode::Symbol,
    ] {
        e.switch_mode(mode);
        for c in (0x21u8..=0x7e)
            .map(char::from)
            .filter(char::is_ascii_punctuation)
        {
            e.clear();
            if c == '\'' {
                continue; // 撇号在中文模式是分隔符：进缓冲，另一条测试覆盖
            }
            let out = e.input_key(c);
            assert!(
                !(out.is_empty() && e.buffer().is_empty()),
                "{mode:?} 下 {c:?} 被吞了（既没进缓冲也没交回）"
            );
        }
    }
}

/// 满缓冲不得被误当成「这个字符没处放」：打到 16 上限后再打字母，
/// 缓冲**原样留着**，绝不 flush（回归：交回分支曾用 `is_ascii_graphic`，
/// 于是第 17 个字母把用户打到一半的拼音整段提交上屏）。
#[test]
fn full_buffer_is_not_flushed_by_a_letter() {
    let mut e = engine();
    for c in "ni".repeat(8).chars() {
        e.input_key(c);
    }
    assert_eq!(e.buffer().chars().count(), 16, "前置：缓冲已到上限");
    let before = e.buffer().to_string();
    assert_eq!(e.input_key('a'), "", "满缓冲下字母键不提交、也不进缓冲");
    assert_eq!(e.buffer(), before, "满缓冲必须原样留着");
}

/// Android 数字面板那条路：Number 模式半角，`input_key` 原样交回 `,` `.`
/// （旧路径是 UI 直接 commit、绕开引擎；现在引擎是唯一出口，返回空串就等于吞键）。
#[test]
fn number_mode_returns_raw_punctuation_to_the_caller() {
    let mut e = engine();
    e.switch_mode(Mode::Number);
    assert_eq!(e.input_key(','), ",");
    assert_eq!(e.input_key('.'), ".");
    assert_eq!(e.input_key('1'), "", "数字进缓冲，不是提交文本");
    assert_eq!(e.buffer(), "1");
    assert_eq!(e.input_key(','), "1,", "待提交的缓冲先上屏，标点排在其后");
    e.clear();
    e.toggle_fullwidth();
    assert_eq!(e.input_key(','), "，", "面板键同样吃全角开关");
}

// ---------- 路由层：`,` 一路走到底 ----------

/// 胶水不是闸门（`opi_fcitx5.cpp` 无键过滤器，实测），闸门在路由的可打印分支：
/// 这里钉「按下 → 上屏」，以及抬起按按下的结论回复。
#[test]
fn router_commits_chinese_punctuation() {
    let mut r = KeyRouter::new(engine());
    assert_eq!(r.key_event(',' as u32, 0), KeyAction::Input("，".into()));
    assert_eq!(
        r.key_event(',' as u32, KEY_STATE_RELEASED),
        KeyAction::EngineHandled,
        "按下出文本、抬起不重复提交"
    );
    assert_eq!(r.key_event('.' as u32, 0), KeyAction::Input("。".into()));
    // 半角（用户按了切换键）：直通，交客户端自己插
    r.engine_mut().toggle_fullwidth();
    assert_eq!(r.key_event('.' as u32, 0), KeyAction::PassThrough);
    // 英文模式不受影响：直传
    r.switch_mode(Mode::English);
    assert_eq!(r.key_event('.' as u32, 0), KeyAction::Input(".".into()));
}

/// 符号模式下标点原样插（不进关键字缓冲）—— 与 Android 同一条路。
#[test]
fn router_symbol_mode_inserts_punctuation() {
    let mut r = KeyRouter::new(engine());
    r.switch_mode(Mode::Symbol);
    assert_eq!(r.key_event(',' as u32, 0), KeyAction::Input(",".into()));
    assert_eq!(r.buffer(), "", "标点不是关键字，不进缓冲");
}
