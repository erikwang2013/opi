// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 引擎层标点表在本轨**路由层**的落点（`#[path]` 引入 logic_input_method.rs，保持各文件 <500 行）。
//!
//! 表本身（哪个键映射到什么）钉在 engine-core 的 `tests/punctuation.rs`；本文件钉
//! 路由这一层多出来的两件事：
//! 1. 可打印分支**先问标点表**、再分模式 —— 表命中就直接提交，不进任何模式的
//!    关键字/缓冲语义（`Mode::Symbol` 那一臂最容易被漏掉：它自己也收可见 ASCII）；
//! 2. 命中时缓冲**先 flush 上屏**（标点排在待提交的拼音后面），页码跟着归零
//!    —— 与空格同一条收尾，故必须走 `input_punct` 而不是直接问映射。
//!
//! 两轨逐条同构：本文件 ↔ `crates/fcitx5-opi/src/input_method_punct_tests.rs`（14 条同名同序，逐行对照过）。
//! `crates/engine-core/tests/punctuation.rs` 共享其中 12 条；它另有 4 条只属于那一层：
//! 数据门 `every_symbol_keyword_is_alnum`（不经路由）、直接调 `Engine` 的
//! `number_mode_returns_raw_punctuation_to_the_caller`（本轨没有「无客户端可交」的处境，
//! 路由是 Unhandled）、以及路由层的 `router_commits_chinese_punctuation` 与
//! `router_symbol_mode_inserts_punctuation`。本轨这 2 条不跨到 engine：
//! `pinyin_ascii_punctuation_maps_to_chinese` 与 `pinyin_apostrophe_is_separator_only_while_composing`
//! —— engine 层没有「VK 已给全角键值」这个处境（非 ASCII 键值不归表管那半段）。

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
        last_printable: None,
    };
    s.switch_mode(Mode::Pinyin);
    s
}

fn typed(s: &mut TsfLogic, word: &str) {
    for c in word.chars() {
        s.input_key(c as u32, 0);
    }
}

// ---------- 改写自 logic_tests.rs 的两条旧用例 ----------
// 旧名分别是 `pinyin_symbol_unhandled`（钉「拼音态标点放行」）与
// `pinyin_apostrophe_goes_to_buffer`（只钉了撇号的后半段）。标点表落地后前者已不成立，
// 两条都挪进本文件随标点一起改（原文件同时顶到了 500 行上限）。

/// 非 ASCII 的 `，`（VK 层已给全角键值）不归表管、照旧放行：标点表只管 ASCII 码位，
/// 全角字形由键盘布局给，别映射第二遍；ASCII 的 `,` 才出中文标点。
#[test]
fn pinyin_ascii_punctuation_maps_to_chinese() {
    let mut s = punct_state();
    assert_eq!(s.input_key('，' as u32, 0), KeyOutcome::Unhandled);
    assert_eq!(s.input_key(',' as u32, 0), KeyOutcome::Commit("，".into()));
    assert_eq!(s.buffer(), "");
}

/// 撇号：缓冲空时是引号（标点表），缓冲非空时是**音节分隔符**（`xi'an`）。
#[test]
fn pinyin_apostrophe_is_separator_only_while_composing() {
    let mut s = punct_state();
    assert_eq!(s.input_key('\'' as u32, 0), KeyOutcome::Commit("‘".into()));
    assert_eq!(s.buffer(), "", "缓冲空：撇号没有分隔语义，当引号");
    for c in ['x', 'i'] {
        s.input_key(c as u32, 0);
    }
    assert_eq!(s.input_key('\'' as u32, 0), KeyOutcome::CompositionChanged);
    assert_eq!(s.buffer(), "xi'", "缓冲非空：音节分隔符，入缓冲");
}

// ---------- 中文模式：老问的那张表，一行一键 ----------

/// 中文模式（拼音/繁体）逐键映射，走**路由**。前半是中文标点表，后半是机械全角兜底
/// （这些键没有中文专用字形，但不是「直通」——中文模式就是全角模式）。
/// 非 ASCII 键值（如 VK 层已给的 `，`）不归表管、照旧放行：别映射第二遍。
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
            let mut s = punct_state();
            s.switch_mode(mode);
            assert_eq!(
                s.input_key(input as u32, 0),
                KeyOutcome::Commit(want.to_string()),
                "{mode:?} 下 {input:?} 的映射"
            );
            assert_eq!(s.buffer(), "", "{mode:?} 下 {input:?} 不该进缓冲");
        }
        // 非 ASCII 键值不归表管：全角字形由键盘布局给，本表只管 ASCII 码位
        let mut s = punct_state();
        s.switch_mode(mode);
        assert_eq!(
            s.input_key('，' as u32, 0),
            KeyOutcome::Unhandled,
            "{mode:?} 下非 ASCII 标点应放行"
        );
    }
}

/// 成对引号交替。`"` 与 `'` 各有一份独立状态（共用一份会出 `”‘` 这种半对）。
#[test]
fn paired_quotes_alternate() {
    let mut s = punct_state();
    assert_eq!(s.input_key('"' as u32, 0), KeyOutcome::Commit("“".into()));
    assert_eq!(s.input_key('"' as u32, 0), KeyOutcome::Commit("”".into()));
    assert_eq!(s.input_key('"' as u32, 0), KeyOutcome::Commit("“".into()));
    assert_eq!(
        s.input_key('\'' as u32, 0),
        KeyOutcome::Commit("‘".into()),
        "单引号另有状态"
    );
    assert_eq!(s.input_key('\'' as u32, 0), KeyOutcome::Commit("’".into()));
}

/// **复位时机 = 切模式**（用户要的「复位时机」，这是裁决）：不随提交/清缓冲复位 ——
/// 引号交替是排版状态，跟着正在写的这段文本走：提交一个词再打右引号是常态，而缓冲
/// 在每次选词后都会清空，跟着缓冲复位等于右引号永远打不出来。全角开关同样不复位。
#[test]
fn quote_alternation_resets_on_mode_switch_only() {
    let mut s = punct_state();
    assert_eq!(s.input_key('"' as u32, 0), KeyOutcome::Commit("“".into()));
    // 选词提交（连缓冲一起清）后：状态保持 → 下一对是右引号
    typed(&mut s, "ni");
    assert_eq!(s.input_key(KEY_SPACE, 0), KeyOutcome::Commit("你".into()));
    assert_eq!(
        s.input_key('"' as u32, 0),
        KeyOutcome::Commit("”".into()),
        "提交不复位"
    );
    // 全角开关往返回一次：还是右引号
    s.toggle_fullwidth();
    s.toggle_fullwidth();
    assert_eq!(
        s.input_key('"' as u32, 0),
        KeyOutcome::Commit("“".into()),
        "半个来回后是下一对"
    );
    // 切模式复位：新的一段文本从头开引号
    let mut s = punct_state();
    assert_eq!(s.input_key('"' as u32, 0), KeyOutcome::Commit("“".into()));
    s.switch_mode(Mode::Traditional);
    assert_eq!(
        s.input_key('"' as u32, 0),
        KeyOutcome::Commit("“".into()),
        "切模式复位"
    );
}

/// 撇号在拼音/繁体是**音节分隔符**：缓冲非空时不进标点表（`xi'an` 打得出来）。
/// 缓冲空时它没有分隔语义，这时才当引号 —— 两个需求都不丢。
#[test]
fn apostrophe_is_separator_while_composing() {
    let mut s = punct_state();
    assert_eq!(s.input_key('\'' as u32, 0), KeyOutcome::Commit("‘".into()));
    assert_eq!(s.buffer(), "", "缓冲空：撇号没有分隔语义，当引号");
    typed(&mut s, "xi");
    assert_eq!(s.input_key('\'' as u32, 0), KeyOutcome::CompositionChanged);
    assert_eq!(s.buffer(), "xi'", "缓冲非空：音节分隔符，入缓冲");
}

/// 标点必须落在待提交的拼音**后面**：先走与空格同一条收尾（有候选提首候选、
/// 无候选提原文），否则 `ni,` 上屏成 `，你` —— 顺序反了。
#[test]
fn punctuation_flushes_pending_buffer_first() {
    let mut s = punct_state();
    typed(&mut s, "ni");
    assert_eq!(
        s.input_key(',' as u32, 0),
        KeyOutcome::Commit("你，".into())
    );
    assert_eq!(s.buffer(), "");
    // 乱码缓冲无候选：与空格同一条逃生口，原文上屏
    // （别拿 "zzz" 当乱码：它是 😴 的关键字）
    let mut s = punct_state();
    typed(&mut s, "qqq");
    assert_eq!(
        s.input_key(',' as u32, 0),
        KeyOutcome::Commit("qqq，".into())
    );
}

/// flush 是**本轨**的页码事件：`reset_page_if_buffer_changed` 是「缓冲变了就回第一页」
/// 的唯一实现处，标点这条新路径必须走它 —— 否则翻到第 3 页打个逗号，下一页停在第 3 页
/// 而候选早已换了一批（与空格提交后翻页同一个坑）。
#[test]
fn punctuation_flush_resets_page_to_first() {
    let mut d = InMemoryDictionary::new();
    for i in 0..20 {
        d.insert("hao", &format!("词{i:02}"), (5000 - i) as u32);
    }
    let mut s = TsfLogic {
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
    typed(&mut s, "hao");
    s.next_page();
    assert_eq!(s.page(), 1, "前置：20 条候选有 3 页");
    assert_eq!(
        s.input_key(',' as u32, 0),
        KeyOutcome::Commit("词00，".into())
    );
    assert_eq!(s.page(), 0, "缓冲被 flush 上屏 → 页码归零");
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

/// 中文模式关掉全角 → 标点全部放行（交应用自己插半角字符），
/// 但字母照旧进缓冲：全角开关只管标点。
/// 撇号不在这一列：它在拼音里是音节分隔符，半角态照样进缓冲（见上一条）——
/// 「半角」关掉的是标点**映射**，不是分隔符语义。
#[test]
fn chinese_halfwidth_passes_everything_through() {
    let mut s = punct_state();
    s.toggle_fullwidth();
    for c in [',', '.', '\\', '[', '"'] {
        assert_eq!(s.input_key(c as u32, 0), KeyOutcome::Unhandled, "{c:?}");
    }
    typed(&mut s, "ni");
    assert_eq!(s.buffer(), "ni", "半角开关不影响字母入缓冲");
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
    // 字母/数字不进全角：全角字母表是另一件事，本表只管标点
    assert_eq!(s.input_key('a' as u32, 0), KeyOutcome::Commit("a".into()));
}

/// 符号模式：不映射 —— 标点在关键字表里根本不存在（生产表 583 条全是 `[a-z0-9]`，
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

// ---------- 标点永不无声消失 ----------

/// 不变式：**ASCII 标点**进路由后不会无声消失 —— 要么被标点表映射（Commit），
/// 要么放行给应用（Unhandled）。这一条把「死键」整类消灭：标点在两种半角/全角
/// 状态下都有归宿，任何模式下都不该出现「既不上屏、也没放行」（Consumed 却无文本）。
/// 边界画在标点上而不是全部可见 ASCII：字母/数字各有去处（进缓冲 / 这个模式不收），
/// 且缓冲满时它们连缓冲都进不去（见 engine-core 的 full_buffer_is_not_flushed_by_a_letter）。
#[test]
fn ascii_punctuation_never_vanishes() {
    let mut s = punct_state();
    for mode in [
        Mode::Pinyin,
        Mode::Traditional,
        Mode::English,
        Mode::Number,
        Mode::Symbol,
    ] {
        s.switch_mode(mode);
        for c in (0x21u8..=0x7e)
            .map(char::from)
            .filter(char::is_ascii_punctuation)
        {
            s.clear();
            if c == '\'' {
                continue; // 撇号在中文模式是分隔符：进缓冲，另一条测试覆盖
            }
            match s.input_key(c as u32, 0) {
                KeyOutcome::Commit(_) => {}
                KeyOutcome::Unhandled => {}
                other => panic!("{mode:?} 下 {c:?} 被吞成了 {other:?}"),
            }
        }
    }
}

/// 满缓冲不得被误当成「这个字符没处放」：打到 16 上限后再打字母，缓冲**原样留着**，
/// 绝不 flush。回归：交回分支曾用 `is_ascii_graphic`，于是第 17 个字母把用户打到
/// 一半的拼音整段提交上屏。
#[test]
fn full_buffer_is_not_flushed_by_a_letter() {
    let mut s = punct_state();
    typed(&mut s, &"ni".repeat(8));
    assert_eq!(s.buffer().chars().count(), 16, "前置：缓冲已到上限");
    let before = s.buffer();
    assert_eq!(s.input_key('a' as u32, 0), KeyOutcome::CompositionChanged);
    assert_eq!(s.buffer(), before, "满缓冲必须原样留着");
}
