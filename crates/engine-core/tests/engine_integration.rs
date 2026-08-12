use engine_core::candidates::{CandidateKind, DEFAULT_TOP_N};
use engine_core::composer::Mode;
use engine_core::{Engine, InMemoryDictionary};

fn test_engine(learner_enabled: bool) -> Engine {
    let mut d = InMemoryDictionary::new();
    d.insert("hao", "好", 5000);
    d.insert("hao", "号", 1200);
    d.insert("hao", "豪", 800);
    d.insert("xiao", "笑", 3000);
    d.insert("xiao", "小", 2000);
    d.insert("xiao", "校", 1000);
    Engine::new(Box::new(d), engine_core::symbols::SymbolEngine::builtin(), learner_enabled)
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
    let mut e = test_engine(false);
    for ch in "xiao".chars() {
        e.input_key(ch);
    }
    let got = e.candidates(DEFAULT_TOP_N);
    let emoji = got.iter().find(|c| c.kind == CandidateKind::Emoji && c.text == "😄");
    assert!(emoji.is_some());
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

#[test]
fn symbol_panel_queries() {
    let e = test_engine(false);
    let blocks = e.symbol_blocks();
    assert!(blocks.iter().any(|b| b.name == "杂项符号"));
    let hearts = e.search_symbols("heart");
    assert_eq!(hearts[0].text, "♥");
    let in_block = e.symbols_in_block(engine_core::symbols::BlockId(3));
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
