// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

use engine_core::candidates::{CandidateKind, DEFAULT_TOP_N};
use engine_core::composer::Mode;
use engine_core::symbols::{Block, BlockId, SymbolEngine, SymbolEntry};
use engine_core::{Engine, InMemoryDictionary};

/// 符号表用**空表**：符号内容来自 scripts 生成的数据文件，本文件的断言只关心
/// 拼音候选与 Engine 门面，不该被数据内容牵连。
fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

/// 符号面板/emoji 合并测试专用的小内联符号表（不依赖数据文件）。
fn panel_symbols() -> SymbolEngine {
    SymbolEngine::new(
        vec![
            Block { id: BlockId(1), start: 0x2600, end: 0x26FF, name: "杂项符号".into(), common: true },
            Block { id: BlockId(2), start: 0x1F600, end: 0x1F64F, name: "表情符号".into(), common: true },
        ],
        vec![
            SymbolEntry {
                text: "♥".into(),
                name: "心形".into(),
                keywords: vec!["heart".into(), "xin".into()],
                block: BlockId(1),
                emoji: false,
            },
            SymbolEntry {
                text: "😄".into(),
                name: "微笑".into(),
                keywords: vec!["xiao".into(), "smile".into()],
                block: BlockId(2),
                emoji: true,
            },
        ],
    )
}

fn test_engine(learner_enabled: bool) -> Engine {
    let mut d = InMemoryDictionary::new();
    d.insert("hao", "好", 5000);
    d.insert("hao", "号", 1200);
    d.insert("hao", "豪", 800);
    d.insert("xiao", "笑", 3000);
    d.insert("xiao", "小", 2000);
    d.insert("xiao", "校", 1000);
    Engine::new(Box::new(d), no_symbols(), learner_enabled)
}

#[test]
fn candidates_sorted_by_freq() {
    let mut e = test_engine(false);
    e.input_key('h');
    e.input_key('a');
    e.input_key('o');
    let got = e.candidates(DEFAULT_TOP_N);
    assert_eq!(got[0].text, "好");
    assert_eq!(got[1].text, "号");
    assert_eq!(got[2].text, "豪");
}

#[test]
fn space_commits_top_candidate() {
    let mut e = test_engine(false);
    e.input_key('h');
    e.input_key('a');
    e.input_key('o');
    assert_eq!(e.input_key(' '), "好");
    assert_eq!(e.candidates(DEFAULT_TOP_N).len(), 0);
}

#[test]
fn no_candidate_space_commits_raw_buffer() {
    let mut e = test_engine(false);
    e.input_key('x');
    e.input_key('y');
    e.input_key('z');
    assert_eq!(e.input_key(' '), "xyz");
}

#[test]
fn select_records_learning() {
    let mut e = test_engine(true);
    e.input_key('x');
    e.input_key('i');
    e.input_key('a');
    e.input_key('o');
    let c = e.candidates(DEFAULT_TOP_N);
    let idx = c.iter().position(|c| c.text == "小").unwrap();
    assert_eq!(e.select(idx), "小");
    let exported = e.export_user_words();
    assert!(exported.contains("小"));
}

#[test]
fn learner_boost_reorders_after_repeats() {
    let mut e = test_engine(true);
    e.input_key('h');
    e.input_key('a');
    e.input_key('o');
    for _ in 0..3 {
        // 每次循环重算：选中后排序已变，冻结的 idx 会选到别的词。
        let idx = e.candidates(DEFAULT_TOP_N).iter().position(|c| c.text == "豪").unwrap();
        e.select(idx);
        e.input_key('h');
        e.input_key('a');
        e.input_key('o');
    }
    let got = e.candidates(DEFAULT_TOP_N);
    assert_eq!(got[0].text, "豪");
}

#[test]
fn emoji_mixed_into_candidates() {
    let d = {
        let mut d = InMemoryDictionary::new();
        d.insert("xiao", "笑", 3000);
        d
    };
    let mut e = Engine::new(Box::new(d), panel_symbols(), false);
    for ch in "xiao".chars() {
        e.input_key(ch);
    }
    let got = e.candidates(DEFAULT_TOP_N);
    let emoji = got.iter().find(|c| c.kind == CandidateKind::Emoji && c.text == "😄");
    assert!(emoji.is_some());
    assert_eq!(got[0].text, "笑", "emoji 不得顶掉精确匹配的拼音候选");
}

#[test]
fn english_mode_commits_shifted() {
    let mut e = test_engine(false);
    e.switch_mode(Mode::English);
    e.set_shift(true);
    e.input_key('H');
    // shift 是粘滞态（Task 5 语义），打完大写后手动释放
    e.set_shift(false);
    e.input_key('i');
    assert_eq!(e.input_key(' '), "Hi");
}

#[test]
fn number_mode_commits_digits() {
    let mut e = test_engine(false);
    e.switch_mode(Mode::Number);
    for ch in ['2', '0', '2', '6'] {
        e.input_key(ch);
    }
    assert_eq!(e.input_key(' '), "2026");
}

#[test]
fn disabled_learner_exports_empty() {
    let mut e = test_engine(false);
    e.input_key('x');
    e.input_key('i');
    e.input_key('a');
    e.input_key('o');
    let c = e.candidates(DEFAULT_TOP_N);
    let idx = c.iter().position(|c| c.text == "笑").unwrap();
    e.select(idx);
    assert_eq!(e.export_user_words(), r#"{"version":1,"words":[]}"#);
}

/// 学习持久化的引擎门面：导出 → 新引擎导入（模拟进程被杀后重启）→ 学过的词立刻回到首位。
/// 这正是 README 缘起 #3「你最常用的那个词永远排在最后」的修复路径。
#[test]
fn export_then_import_round_trips_user_words() {
    let mut e = test_engine(true);
    for ch in "hao".chars() {
        e.input_key(ch);
    }
    let idx = e.candidates(DEFAULT_TOP_N).iter().position(|c| c.text == "豪").unwrap();
    assert_eq!(e.select(idx), "豪");

    let json = e.export_user_words();
    let mut restarted = test_engine(true);
    assert_eq!(restarted.import_user_words(&json).unwrap(), 1);
    for ch in "hao".chars() {
        restarted.input_key(ch);
    }
    assert_eq!(
        restarted.candidates(DEFAULT_TOP_N)[0].text,
        "豪",
        "重启后学过的词必须仍排首位（否则学习等于没做）"
    );
}

#[test]
fn import_user_words_rejects_garbage_atomically() {
    let mut e = test_engine(true);
    assert!(e.import_user_words("{").is_err());
    assert!(e.import_user_words(r#"{"version":2,"words":[]}"#).is_err());
    assert_eq!(e.export_user_words(), r#"{"version":1,"words":[]}"#, "失败的导入不得留痕");
    assert_eq!(
        e.import_user_words(r#"{"version":1,"words":[{"text":"好","freq":9}]}"#).unwrap(),
        1
    );
    assert_eq!(e.export_user_words(), r#"{"version":1,"words":[{"text":"好","freq":9}]}"#);
}

#[test]
fn symbol_panel_queries() {
    let mut d = InMemoryDictionary::new();
    d.insert("hao", "好", 5000);
    let e = Engine::new(Box::new(d), panel_symbols(), false);
    let blocks = e.symbol_blocks();
    assert!(blocks.iter().any(|b| b.name == "杂项符号"));
    assert_eq!(e.search_symbols("heart")[0].text, "♥");
    assert!(e.search_symbols("zzz").is_empty());
    let in_block = e.symbols_in_block(BlockId(1));
    assert!(in_block.iter().any(|s| s.text == "♥"));
}

#[test]
fn backspace_clears_buffer() {
    let mut e = test_engine(false);
    e.input_key('h');
    e.input_key('a');
    e.backspace();
    // 缓冲 "h" 仍是音节前缀，还有候选；退两次后缓冲为空。
    assert!(!e.candidates(DEFAULT_TOP_N).is_empty());
    e.backspace();
    assert_eq!(e.candidates(DEFAULT_TOP_N).len(), 0);
}
