// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 引擎层标点表 + 全角/半角开关（用户裁决 2026-09-27：「引擎层加标点表，四端走同一条路」）。
//!
//! 契约：
//! - 中文模式（拼音/繁体）里 ASCII 标点出**中文标点**（`,` → `，`、`.` → `。`、`\` → `、`），
//!   表里没有的 ASCII 标点在全角开关开着时机械全角兜底（`/` → `／`）；
//! - **两个开关相互独立**（用户裁决 2026-09-28）：`chinese_punct` 管中文标点表那一档
//!   （含引号交替）、`fullwidth` 管机械全角那一档；只关全角**不再**把表一起关掉，
//!   真值表逐格见 `chinese_punct_and_fullwidth_are_independent`；
//! - 英文/数字模式默认**半角直通**（`input_punct` 返回 `None`）；全角开关打开后出
//!   **机械全角**（`.` → `．` U+FF0E，不是中文句号 `。`）；`chinese_punct` 在那两个
//!   模式不参与判定（没有中文标点表这一档）；
//! - 同键成对的引号交替出左右引号，**切模式**时复位；
//! - 撇号在拼音/繁体里是**音节分隔符**（`xi'an`），只有缓冲为空时才当引号；
//! - 标点落在待提交的拼音**后面**（先走与空格同一条收尾）；
//! - `input_key` **不吞可见字符**：没进缓冲也没映射的，原样交回调用方
//!   —— Android 数字面板（旧路径是 UI 直接 commit，绕开引擎）靠这条活在引擎里。
//!
//! 本文件走**公开 API**（`Engine` + `KeyRouter`）；两轨的镜像（同名同序，彼此逐行同构）
//! 在 crates/fcitx5-opi/src/input_method_punct_tests.rs 与
//! crates/tsf-opi/src/logic_punct_tests.rs；多数用例共享，少数只属于这一层
//! （路由用例，以及不经路由的：数据门与「无客户端可交」的直调用例）。
//!
//! 拆开关（2026-09-28）新增的 4 条与两个开关的行为锁在 `punctuation_switches.rs`
//! —— 本文件撞了 500 行门禁，按仓里惯例拆出去的。
//!
//! **镜像同步状态**：那两轨尚未跟上拆开关 —— 它们按「只关全角 ⇒ 直通」写的 3+3 条
//! 用例会红（`chinese_halfwidth_passes_everything_through` 与 `*_release_tests.rs`
//! 里以 `.` 直通为背景的两条），等下一波接线时一并改。别把那份红当成引擎回归。

use engine_core::composer::MAX_BUFFER;
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
        // 斜杠 → 顿号（2026-10-15 用户裁决「斜杠问号键在中文下，点击默认中文顿号」）：
        // 它此前落在**机械全角兜底**那一档（出 `／`）；进表后归中文标点档 ——
        // 这正是「关掉中文标点后它直通」要能被看见的那一格。
        ('/', "、"),
        ('(', "（"),
        (')', "）"),
        ('[', "【"),
        (']', "】"),
        ('<', "《"),
        ('>', "》"),
        ('~', "～"),
        // 机械全角兜底：0x21..=0x7E 平移 +0xFEE0（Unicode 全角区的定义）
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
    // 分隔符语义与两个标点开关**无关**（它排在两道闸之前）：表关掉、全角开着也一样
    let mut e = engine();
    e.set_chinese_punct(false);
    typed(&mut e, "xi");
    assert_eq!(e.input_punct('\''), None, "表关掉：撇号仍是音节分隔符");
    assert_eq!(e.input_key('\''), "", "入缓冲，无提交文本");
    assert_eq!(e.buffer(), "xi'", "照样进缓冲");
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
/// 的唯一实现处。提交的是**当前页**首条（2026-10-24 用户报障「翻页后直接确认始终是
/// 第一页的 1」），提交后页码归零。
#[test]
fn punctuation_flush_uses_current_page_first_then_resets() {
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
        KeyAction::Input("词08，".into())
    );
    assert_eq!(r.page(), 0, "缓冲被 flush 上屏 → 页码归零");
}

/// 符号模式：不映射（`,` `.` 在关键字里根本不存在，见下一条）——**两个开关都不生效**
/// （模式分派在两道闸之前，见 `Engine::punct_text` 的 `Mode::Symbol` 那一支）。
#[test]
fn symbol_mode_has_no_punctuation_mapping() {
    let mut e = engine();
    e.switch_mode(Mode::Symbol);
    assert_eq!(e.input_punct(','), None);
    e.toggle_fullwidth();
    assert_eq!(e.input_punct(','), None, "全角开关在符号模式不生效");
    e.set_chinese_punct(false);
    assert_eq!(e.input_punct(','), None, "中文标点开关在符号模式不生效");
    e.set_chinese_punct(true);
    e.toggle_fullwidth();
    assert_eq!(e.input_punct(','), None, "两档都开也一样");
}

/// 符号关键字**全是** `^[a-z0-9]+$`（本测试自己遍历全表实测；条数随表增长，故不写死）—— 标点永远搜不出候选，
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
    // 前置按 MAX_BUFFER 符号化（别写死 16/32）：改上限不用再动这里。
    for _ in 0..MAX_BUFFER {
        e.input_key('n');
    }
    assert_eq!(e.buffer().chars().count(), MAX_BUFFER, "前置：缓冲已到上限");
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
    // 关掉全角：中文标点表还在 → `.` 仍是 `。`（两个开关独立）
    r.engine_mut().toggle_fullwidth();
    assert_eq!(r.key_event('.' as u32, 0), KeyAction::Input("。".into()));
    // 再关掉中文标点：两道闸都关 → 直通，交客户端自己插
    r.engine_mut().set_chinese_punct(false);
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

// ---------- 数字串后的句号（2026-10-17 用户裁决，桌面端报障） ----------

/// 「键盘输入数字的时候……数字后面点击 . 键默认只能是 .」：数字**紧邻**的句号键出
/// ASCII `.`（不被中文标点表、也不被全角兜底改字形）；不紧邻时照旧走标点表。
///
/// 判据放引擎层：每端路由的可打印分支都先问 [`Engine::input_punct`]（fcitx5/TSF/
/// 平台中立路由同序）——桌面端数字直通后，引擎只剩这一个观测点，四端同一条路。
#[test]
fn period_after_digits_stays_ascii() {
    let mut e = engine();
    // 数字走同一条可打印门（router 的顺序：先 punct、再模式分派）。
    assert_eq!(e.input_punct('1'), None, "数字不进标点表");
    assert_eq!(e.input_punct('2'), None);
    assert_eq!(
        e.input_punct('.').as_deref(),
        Some("."),
        "数字紧邻的句号必须是 ASCII `.`（不是 。、也不是全角 ．）"
    );
    // 句号之后已不在「数字后面」：回到中文句号。
    assert_eq!(e.input_punct('.').as_deref(), Some("。"));
}

/// 紧邻关系由**最近一次可打印输入**决定：字母、空格都会打断它。
#[test]
fn period_after_non_digit_stays_chinese() {
    let mut e = engine();
    assert_eq!(
        e.input_punct('.').as_deref(),
        Some("。"),
        "无前文：中文句号"
    );

    let mut e = engine();
    assert_eq!(e.input_punct('1'), None);
    assert_eq!(e.input_punct('a'), None, "字母不进标点表");
    assert_eq!(
        e.input_punct('.').as_deref(),
        Some("。"),
        "字母打断数字上下文"
    );

    let mut e = engine();
    assert_eq!(e.input_punct('1'), None);
    e.input_space();
    assert_eq!(
        e.input_punct('.').as_deref(),
        Some("。"),
        "空格收尾后不再是数字上下文"
    );
}

/// 9 键：数字是**输入码**（缓冲非空）—— 不是「数字输入」，句号仍出中文。
#[test]
fn period_with_nine_key_code_buffer_stays_chinese() {
    let mut e = engine();
    e.set_scheme(engine_core::scheme::Scheme::Nine);
    typed(&mut e, "2"); // 9 键码进缓冲
    assert_eq!(
        e.input_punct('.').as_deref(),
        Some("。"),
        "缓冲非空（输入码）时数字上下文规则不生效"
    );
}
