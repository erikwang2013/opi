//! 简繁双词库路由（spec 2026-08-15）：Traditional 模式查 trad 词典，Pinyin 模式不受影响。
use engine_core::composer::Mode;
use engine_core::symbols::SymbolEngine;
use engine_core::{Engine, InMemoryDictionary};

fn dict(entries: &[(&str, &str, u32)]) -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    for (py, w, f) in entries {
        d.insert(py, w, *f);
    }
    d
}

/// 符号表用空表：本文件测的是词典路由，符号数据来自生成的文件，不该参与这里的断言。
fn no_symbols() -> SymbolEngine {
    SymbolEngine::new(Vec::new(), Vec::new())
}

fn two_dict_engine() -> Engine {
    let simp = dict(&[("hao", "好", 5000), ("hao", "号", 1200)]);
    let trad = dict(&[("hao", "發", 4000), ("hao", "髮", 3500)]);
    Engine::with_dictionaries(Box::new(simp), Some(Box::new(trad)), no_symbols(), false)
}

#[test]
fn traditional_mode_queries_trad_dict() {
    let mut e = two_dict_engine();
    e.switch_mode(Mode::Traditional);
    e.input_key('h');
    e.input_key('a');
    e.input_key('o');
    assert_eq!(e.mode(), Mode::Traditional);
    let got = e.candidates(8);
    assert_eq!(got[0].text, "發");
    assert_eq!(got[1].text, "髮");
}

#[test]
fn pinyin_mode_ignores_trad_dict() {
    let mut e = two_dict_engine();
    e.input_key('h');
    e.input_key('a');
    e.input_key('o');
    let got = e.candidates(8);
    assert_eq!(got[0].text, "好");
}

#[test]
fn traditional_without_trad_dict_falls_back_to_simplified() {
    let simp = dict(&[("hao", "好", 5000)]);
    let mut e = Engine::with_dictionaries(Box::new(simp), None, no_symbols(), false);
    e.switch_mode(Mode::Traditional);
    e.input_key('h');
    e.input_key('a');
    e.input_key('o');
    assert_eq!(e.candidates(8)[0].text, "好");
}

#[test]
fn traditional_space_selects_top_candidate() {
    let mut e = two_dict_engine();
    e.switch_mode(Mode::Traditional);
    for ch in "hao".chars() {
        e.input_key(ch);
    }
    assert_eq!(e.input_key(' '), "發");
}

#[test]
fn switch_mode_clears_buffer_and_traditional_lowercases() {
    let mut e = two_dict_engine();
    e.input_key('n');
    e.switch_mode(Mode::Traditional);
    assert_eq!(e.buffer(), "");
    e.set_shift(true);
    e.input_key('A');
    assert_eq!(e.buffer(), "a"); // Traditional 同 Pinyin：字母转小写、shift 无效
}
